use mdc_core::{
    contract::registry::{Registry, VersionedId},
    identity::IdentityProtocol,
    pipeline::{ObservationRequest, SnapshotLimits, SnapshotRequest, SnapshotSession},
};
use mdc_store::{BatchPolicy, Durability, Error, Limits, Mode, ScheduledStore, Store, StoreClient};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};
#[allow(dead_code)]
#[path = "../../../examples/support/snapshot_fixture.rs"]
mod fixture;
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "mdc-013-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn db(&self) -> PathBuf {
        self.0.join("db")
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn request(n: u64) -> SnapshotRequest {
    SnapshotRequest::Admit(ObservationRequest {
        source: VersionedId::new("unresolved", 1).unwrap(),
        timestamp: Some(n),
        payload: n.to_le_bytes().to_vec(),
        acquisition_context: format!("request/{n}"),
    })
}
fn policy(n: usize, delay: Duration) -> BatchPolicy {
    BatchPolicy {
        max_pending: n,
        max_delay: delay,
        queue_capacity: 2,
    }
}
fn create(t: &Temp, p: BatchPolicy) -> ScheduledStore {
    ScheduledStore::create(
        t.db(),
        Registry::new(),
        IdentityProtocol::PortableV1,
        Limits::default(),
        p,
    )
    .unwrap()
}
fn wait_for(c: &StoreClient, test: impl Fn(&mdc_store::SchedulerStatus) -> bool) {
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        let s = c.status().unwrap();
        if test(&s) {
            return;
        }
        assert!(Instant::now() < until, "soft flush test timed out: {s:?}");
        thread::sleep(Duration::from_millis(5));
    }
}
#[test]
fn invalid_policies_are_rejected_before_filesystem_creation() {
    let t = Temp::new();
    for p in [
        policy(0, Duration::from_secs(1)),
        policy(1, Duration::ZERO),
        policy(1, Duration::MAX),
        BatchPolicy {
            max_pending: 1,
            max_delay: Duration::from_secs(1),
            queue_capacity: 0,
        },
        BatchPolicy {
            max_pending: 1,
            max_delay: Duration::from_secs(1),
            queue_capacity: 1025,
        },
        policy(4097, Duration::from_secs(1)),
    ] {
        assert!(ScheduledStore::create(
            t.db(),
            Registry::new(),
            IdentityProtocol::PortableV1,
            Limits::default(),
            p
        )
        .is_err());
        assert!(!t.db().exists());
    }
}
#[test]
fn count_trigger_commits_whole_prefix_before_threshold_reply() {
    let t = Temp::new();
    let s = create(&t, policy(2, Duration::from_secs(3600)));
    let c = s.client();
    assert_eq!(
        c.submit("one", request(1), Mode::Buffered)
            .unwrap()
            .durability,
        Durability::AcceptedVolatile
    );
    assert_eq!(
        c.submit("two", request(2), Mode::Buffered)
            .unwrap()
            .durability,
        Durability::DurableThrough(2)
    );
    assert_eq!(c.status().unwrap().durable_seq, 2);
    s.shutdown().unwrap();
    let reopened = Store::open(t.db(), Limits::default()).unwrap();
    assert_eq!(reopened.durable_seq(), 2);
}
#[test]
fn idle_timer_flushes_without_another_submit_or_manual_barrier() {
    let t = Temp::new();
    let s = create(&t, policy(10, Duration::from_millis(40)));
    let c = s.client();
    let before = fs::read(t.db().join("HEAD")).unwrap();
    c.submit("one", request(1), Mode::Buffered).unwrap();
    let until = Instant::now() + Duration::from_secs(10);
    // Observe disk publication without sending a command which could itself
    // trigger a lazy flush. The worker must wake independently while idle.
    loop {
        if fs::read(t.db().join("HEAD")).unwrap() != before {
            break;
        }
        assert!(Instant::now() < until, "idle worker did not publish HEAD");
        thread::sleep(Duration::from_millis(5));
    }
    wait_for(&c, |v| v.durable_seq == 1);
    assert!(matches!(
        c.lookup("one").unwrap().unwrap().durability,
        Durability::DurableThrough(1)
    ));
    s.shutdown().unwrap();
}
#[test]
fn continuous_duplicate_traffic_does_not_reset_oldest_pending_deadline() {
    let t = Temp::new();
    let s = create(&t, policy(100, Duration::from_millis(60)));
    let c = s.client();
    c.submit("same", request(1), Mode::Buffered).unwrap();
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        let r = c.submit("same", request(1), Mode::Buffered).unwrap();
        if matches!(r.durability, Durability::DurableThrough(1)) {
            break;
        }
        assert!(Instant::now() < until, "traffic postponed pending deadline");
    }
    assert_eq!(c.status().unwrap().live_seq, 1);
    s.shutdown().unwrap();
}
#[test]
fn explicit_durable_submit_flushes_prior_buffered_inputs() {
    let t = Temp::new();
    let s = create(&t, policy(100, Duration::from_secs(3600)));
    let c = s.client();
    c.submit("input", request(1), Mode::Buffered).unwrap();
    let r = c.submit("critical", request(2), Mode::Durable).unwrap();
    assert_eq!(r.durability, Durability::DurableThrough(2));
    assert_eq!(
        c.lookup("input").unwrap().unwrap().durability,
        Durability::DurableThrough(2)
    );
    s.shutdown().unwrap();
}
#[test]
fn checkpoint_with_pending_calls_is_serialized_and_recovers() {
    let t = Temp::new();
    let s = create(&t, policy(100, Duration::from_secs(3600)));
    let c = s.client();
    c.submit("one", request(1), Mode::Buffered).unwrap();
    c.checkpoint().unwrap();
    let snap = c.snapshot().unwrap();
    assert_eq!(snap.status.live_seq, snap.status.durable_seq);
    s.shutdown().unwrap();
    let s = Store::open(t.db(), Limits::default()).unwrap();
    assert_eq!(
        s.session()
            .unwrap()
            .export(SnapshotLimits::default())
            .unwrap(),
        snap.bytes
    );
}
#[test]
fn count_flush_io_failure_is_reported_and_fences_until_reopen() {
    let t = Temp::new();
    let s = create(&t, policy(2, Duration::from_secs(3600)));
    let c = s.client();
    c.submit("durable", request(1), Mode::Durable).unwrap();
    c.submit("two", request(2), Mode::Buffered).unwrap();
    fs::write(t.db().join("HEAD.pending"), b"injected existing temp").unwrap();
    assert!(matches!(
        c.submit("three", request(3), Mode::Buffered),
        Err(Error::Indeterminate(_))
    ));
    let status = c.status().unwrap();
    assert!(!status.healthy);
    assert!(status.last_error.is_some());
    assert!(matches!(c.snapshot(), Err(Error::RecoveryRequired)));
    assert!(matches!(c.lookup("durable"), Err(Error::RecoveryRequired)));
    assert!(matches!(c.barrier(), Err(Error::RecoveryRequired)));
    assert!(s.shutdown().is_err());
    let s = Store::open(t.db(), Limits::default()).unwrap();
    assert_eq!(s.durable_seq(), 1);
}
#[test]
fn autonomous_timer_failure_remains_observable_without_new_writes() {
    let t = Temp::new();
    let s = create(&t, policy(100, Duration::from_millis(30)));
    let c = s.client();
    c.submit("durable", request(1), Mode::Durable).unwrap();
    fs::write(t.db().join("HEAD.pending"), b"injected temp").unwrap();
    c.submit("pending", request(2), Mode::Buffered).unwrap();
    wait_for(&c, |v| !v.healthy);
    assert!(c.status().unwrap().last_error.is_some());
    assert!(matches!(
        c.submit("new", request(3), Mode::Buffered),
        Err(Error::RecoveryRequired)
    ));
    assert!(s.shutdown().is_err());
    let s = Store::open(t.db(), Limits::default()).unwrap();
    assert_eq!(s.durable_seq(), 1);
}
#[test]
fn request_conflict_does_not_fence_or_duplicate_a_pending_call() {
    let t = Temp::new();
    let s = create(&t, policy(100, Duration::from_secs(3600)));
    let c = s.client();
    let first = c.submit("id", request(1), Mode::Buffered).unwrap();
    assert!(matches!(
        c.submit("id", request(2), Mode::Buffered),
        Err(Error::RequestIdConflict)
    ));
    assert!(c.status().unwrap().healthy);
    assert_eq!(c.submit("id", request(1), Mode::Buffered).unwrap(), first);
    assert_eq!(s.shutdown().unwrap().durable_seq, 1);
}
#[test]
fn cloned_clients_use_one_locked_worker_and_bounded_queue() {
    let t = Temp::new();
    let s = create(
        &t,
        BatchPolicy {
            max_pending: 3,
            max_delay: Duration::from_secs(3600),
            queue_capacity: 1,
        },
    );
    assert!(matches!(
        Store::open(t.db(), Limits::default()),
        Err(Error::LockBusy)
    ));
    let mut threads = Vec::new();
    for producer in 0..4 {
        let c = s.client();
        threads.push(thread::spawn(move || {
            for i in 0..8 {
                let n = producer * 8 + i;
                c.submit(&format!("{producer}/{i}"), request(n), Mode::Buffered)
                    .unwrap();
            }
        }));
    }
    for t in threads {
        t.join().unwrap();
    }
    let c = s.client();
    assert_eq!(c.barrier().unwrap(), 32);
    assert_eq!(c.status().unwrap().live_seq, 32);
    s.shutdown().unwrap();
    let s = Store::open(t.db(), Limits::default()).unwrap();
    assert_eq!(s.live_seq(), 32);
}
#[test]
fn shutdown_flushes_and_joins_before_releasing_lock_and_stops_clients() {
    let t = Temp::new();
    let s = create(&t, policy(100, Duration::from_secs(3600)));
    let c = s.client();
    c.submit("pending", request(1), Mode::Buffered).unwrap();
    let status = s.shutdown().unwrap();
    assert!(status.healthy);
    assert_eq!((status.live_seq, status.durable_seq), (1, 1));
    assert!(c.status().is_err());
    let s = Store::open(t.db(), Limits::default()).unwrap();
    assert_eq!(s.durable_seq(), 1);
}
#[test]
fn no_pending_data_does_not_rewrite_head_on_idle_timer_or_status_queries() {
    let t = Temp::new();
    let s = create(&t, policy(10, Duration::from_millis(10)));
    let c = s.client();
    let before = fs::read(t.db().join("HEAD")).unwrap();
    thread::sleep(Duration::from_millis(60));
    for _ in 0..5 {
        let status = c.status().unwrap();
        assert_eq!(status.durable_seq, 0);
    }
    assert_eq!(fs::read(t.db().join("HEAD")).unwrap(), before);
    s.shutdown().unwrap();
}
#[test]
fn rich_recorded_graph_survives_worker_shutdown_and_scheduler_reopen() {
    let t = Temp::new();
    let mut g = fixture::graph(IdentityProtocol::PortableV1);
    fixture::enrich(&mut g);
    let s = ScheduledStore::create(
        t.db(),
        fixture::registry(),
        IdentityProtocol::PortableV1,
        Limits::default(),
        policy(7, Duration::from_secs(3600)),
    )
    .unwrap();
    let c = s.client();
    for i in 0..g.session.operation_count() {
        let (req, _) = g
            .session
            .recorded_call(i, SnapshotLimits::default())
            .unwrap();
        let r = c
            .submit(
                &format!("call/{i}"),
                SnapshotSession::decode_request(&req, SnapshotLimits::default()).unwrap(),
                Mode::Buffered,
            )
            .unwrap();
        assert_eq!(Some(r.outcome), g.session.recorded_outcome(i));
    }
    let snap = c.snapshot().unwrap();
    assert_eq!(snap.status.live_seq, 38);
    assert_eq!(snap.status.durable_seq, 35);
    assert_eq!(
        snap.bytes,
        g.session.export(SnapshotLimits::default()).unwrap()
    );
    assert_eq!(s.shutdown().unwrap().durable_seq, 38);
    let s = ScheduledStore::open(
        t.db(),
        Limits::default(),
        policy(7, Duration::from_secs(3600)),
    )
    .unwrap();
    assert_eq!(s.client().snapshot().unwrap().bytes, snap.bytes);
    s.shutdown().unwrap();
}
