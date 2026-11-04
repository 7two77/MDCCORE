//! Verification driver; filesystem setup/replay uses the production adapter.
#[allow(dead_code)]
#[path = "../../../../examples/support/snapshot_fixture.rs"]
mod fixture;
use mdc_core::{
    contract::registry::{Registry, VersionedId},
    identity::IdentityProtocol,
    pipeline::{ObservationRequest, SnapshotLimits, SnapshotRequest},
};
use mdc_store::{BatchPolicy, Limits, Mode, ScheduledStore, Store};
fn simple(n: u64) -> SnapshotRequest {
    SnapshotRequest::Admit(ObservationRequest {
        source: VersionedId::new("unresolved", 1).unwrap(),
        timestamp: Some(n),
        payload: n.to_le_bytes().to_vec(),
        acquisition_context: format!("request/{n}"),
    })
}
fn main() {
    let a: Vec<_> = std::env::args().collect();
    let mode = a.get(1).expect("produce|consume|worker|seed|volatile");
    let db = a.get(2).expect("store directory");
    match mode.as_str() {
        "seed" => {
            let mut s = Store::create(
                db,
                Registry::new(),
                IdentityProtocol::PortableV1,
                Limits::default(),
            )
            .unwrap();
            s.submit("one", simple(1), Mode::Durable).unwrap();
        }
        "worker" => {
            let mut s = Store::open(db, Limits::default()).unwrap();
            if a.get(3).map(String::as_str) == Some("checkpoint") {
                s.checkpoint().unwrap();
            } else {
                s.submit("two", simple(2), Mode::Durable).unwrap();
            }
            panic!("worker passed requested fault point");
        }
        "volatile" => {
            let mut s = Store::open(db, Limits::default()).unwrap();
            s.submit("two", simple(2), Mode::Buffered).unwrap();
            println!(
                "MDC_VOLATILE=ACCEPTED live={} durable={}",
                s.live_seq(),
                s.durable_seq()
            );
        }
        "produce" => {
            let mut graph = fixture::graph(IdentityProtocol::PortableV1);
            fixture::enrich(&mut graph);
            let mut s = Store::create(
                db,
                fixture::registry(),
                IdentityProtocol::PortableV1,
                Limits::default(),
            )
            .unwrap();
            for i in 0..graph.session.operation_count() {
                let (req, _) = graph
                    .session
                    .recorded_call(i, SnapshotLimits::default())
                    .unwrap();
                s.submit_encoded(&format!("call/{i}"), &req, Mode::Buffered)
                    .unwrap();
            }
            s.barrier().unwrap();
            if a.get(4).map(String::as_str) == Some("checkpoint") {
                s.checkpoint().unwrap();
            }
            assert_eq!(
                s.session()
                    .unwrap()
                    .export(SnapshotLimits::default())
                    .unwrap(),
                graph.session.export(SnapshotLimits::default()).unwrap()
            );
            std::fs::write(
                a.get(3).expect("oracle path"),
                graph.session.export(SnapshotLimits::default()).unwrap(),
            )
            .unwrap();
            report(&s);
        }
        "scheduled-produce" => {
            let mut graph = fixture::graph(IdentityProtocol::PortableV1);
            fixture::enrich(&mut graph);
            let owner = ScheduledStore::create(
                db,
                fixture::registry(),
                IdentityProtocol::PortableV1,
                Limits::default(),
                BatchPolicy {
                    max_pending: 7,
                    max_delay: std::time::Duration::from_secs(3600),
                    queue_capacity: 4,
                },
            )
            .unwrap();
            let client = owner.client();
            for i in 0..graph.session.operation_count() {
                let (request, _) = graph
                    .session
                    .recorded_call(i, SnapshotLimits::default())
                    .unwrap();
                let receipt = client
                    .submit(
                        &format!("call/{i}"),
                        mdc_core::pipeline::SnapshotSession::decode_request(
                            &request,
                            SnapshotLimits::default(),
                        )
                        .unwrap(),
                        Mode::Buffered,
                    )
                    .unwrap();
                assert_eq!(Some(receipt.outcome), graph.session.recorded_outcome(i));
            }
            let view = client.snapshot().unwrap();
            assert_eq!((view.status.live_seq, view.status.durable_seq), (38, 35));
            assert_eq!(
                view.bytes,
                graph.session.export(SnapshotLimits::default()).unwrap()
            );
            std::fs::write(a.get(3).expect("oracle path"), &view.bytes).unwrap();
            let status = owner.shutdown().unwrap();
            assert!(status.healthy);
            assert_eq!((status.live_seq, status.durable_seq), (38, 38));
            let restored = mdc_core::pipeline::SnapshotSession::restore(
                &view.bytes,
                SnapshotLimits::default(),
            )
            .unwrap();
            println!(
                "MDC_RECOVERY_VERIFY=PASS operations={} durable={} objects={} revision={}",
                status.live_seq,
                status.durable_seq,
                restored.inventory().len(),
                restored.pipeline().lifecycle_revision()
            );
        }
        "consume" => {
            let s = Store::open(db, Limits::default()).unwrap();
            assert_eq!(
                s.session()
                    .unwrap()
                    .export(SnapshotLimits::default())
                    .unwrap(),
                std::fs::read(a.get(3).expect("oracle path")).unwrap()
            );
            report(&s);
        }
        _ => panic!("unknown driver mode"),
    }
}
fn report(s: &Store) {
    println!(
        "MDC_RECOVERY_VERIFY=PASS operations={} durable={} objects={} revision={}",
        s.live_seq(),
        s.durable_seq(),
        s.session().unwrap().inventory().len(),
        s.session().unwrap().pipeline().lifecycle_revision()
    );
}
