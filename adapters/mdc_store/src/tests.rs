use super::*;
use mdc_core::{
    contract::registry::{Registry, VersionedId},
    pipeline::ObservationRequest,
};
use std::sync::atomic::{AtomicU64, Ordering};
#[allow(dead_code)]
#[path = "../../../examples/support/snapshot_fixture.rs"]
mod fixture;
static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "mdc-012-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
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
fn create(t: &Temp) -> Store {
    Store::create(
        t.db(),
        Registry::new(),
        IdentityProtocol::PortableV1,
        Limits::default(),
    )
    .unwrap()
}
fn reopened(t: &Temp) -> Store {
    Store::open(t.db(), Limits::default()).unwrap()
}
fn fail_at<T>(point: &'static str, f: impl FnOnce() -> T) -> T {
    FAULT.with(|p| p.set(Some(point)));
    let r = f();
    FAULT.with(|p| p.set(None));
    r
}
#[test]
fn strict_recovery_preserves_semantic_failure_and_historical_receipt() {
    let t = Temp::new();
    let mut s = create(&t);
    let a = s.submit("one", request(1), Mode::Durable).unwrap();
    assert!(matches!(a.outcome, SnapshotOutcome::Object(Err(_))));
    let manifest = s.session().unwrap().state_manifest(s.limits.core).unwrap();
    drop(s);
    let s = reopened(&t);
    assert_eq!(s.lookup("one").unwrap().unwrap(), a);
    assert_eq!(
        s.session().unwrap().state_manifest(s.limits.core).unwrap(),
        manifest
    );
    assert_eq!(s.durable_seq(), 1);
}
#[test]
fn buffered_tail_is_discarded_even_when_complete_and_retry_is_safe() {
    let t = Temp::new();
    let mut s = create(&t);
    s.submit("one", request(1), Mode::Durable).unwrap();
    assert_eq!(
        s.submit("two", request(2), Mode::Buffered)
            .unwrap()
            .durability,
        Durability::AcceptedVolatile
    );
    let committed = s.head.offset;
    drop(s);
    let mut s = reopened(&t);
    assert_eq!((s.live_seq(), s.durable_seq()), (1, 1));
    assert!(s.lookup("two").unwrap().is_none());
    assert_eq!(s.wal.metadata().unwrap().len(), committed);
    s.submit("two", request(2), Mode::Durable).unwrap();
    assert_eq!(s.live_seq(), 2);
}
#[test]
fn barrier_commits_all_inputs_and_buffered_failure_receipts() {
    let t = Temp::new();
    let mut s = create(&t);
    for i in 1..=4 {
        s.submit(&i.to_string(), request(i), Mode::Buffered)
            .unwrap();
    }
    assert_eq!(s.durable_seq(), 0);
    assert_eq!(s.barrier().unwrap(), 4);
    drop(s);
    let s = reopened(&t);
    for i in 1..=4 {
        assert_eq!(
            s.lookup(&i.to_string()).unwrap().unwrap().durability,
            Durability::DurableThrough(4)
        );
    }
    assert_eq!(s.session().unwrap().pipeline().rejections().len(), 4);
}
#[test]
fn request_id_retry_deduplicates_and_conflict_does_not_mutate() {
    let t = Temp::new();
    let mut s = create(&t);
    let first = s.submit("same", request(1), Mode::Buffered).unwrap();
    assert_eq!(s.submit("same", request(1), Mode::Buffered).unwrap(), first);
    assert!(matches!(
        s.submit("same", request(2), Mode::Durable),
        Err(Error::RequestIdConflict)
    ));
    assert_eq!(s.live_seq(), 1);
    let r = s.submit("same", request(1), Mode::Durable).unwrap();
    assert_eq!(r.durability, Durability::DurableThrough(1));
    drop(s);
    let mut s = reopened(&t);
    assert_eq!(s.submit("same", request(1), Mode::Durable).unwrap(), r);
    assert_eq!(s.live_seq(), 1);
}
#[test]
fn exclusive_lock_and_explicit_create_open() {
    let t = Temp::new();
    assert!(Store::open(t.db(), Limits::default()).is_err());
    let s = create(&t);
    assert!(matches!(
        Store::open(t.db(), Limits::default()),
        Err(Error::LockBusy)
    ));
    assert!(Store::create(
        t.db(),
        Registry::new(),
        IdentityProtocol::PortableV1,
        Limits::default()
    )
    .is_err());
    drop(s);
    let _s = reopened(&t);
}
#[test]
fn budget_invalid_request_and_bad_encoding_leave_wal_head_and_session_unchanged() {
    let t = Temp::new();
    let mut s = create(&t);
    let before = fs::read(t.db().join("HEAD")).unwrap();
    let offset = s.offset;
    s.limits.max_wal_bytes = s.offset as usize + 10;
    assert!(matches!(
        s.submit("budget", request(1), Mode::Durable),
        Err(Error::Limit(_))
    ));
    for id in ["", "bad\0id"] {
        assert!(s.submit(id, request(1), Mode::Buffered).is_err());
    }
    assert!(s.submit_encoded("bad", &[255], Mode::Buffered).is_err());
    assert_eq!(s.live_seq(), 0);
    assert_eq!(s.offset, offset);
    assert_eq!(s.wal.metadata().unwrap().len(), offset);
    assert_eq!(fs::read(t.db().join("HEAD")).unwrap(), before);
    assert!(!s.recovery_required());
}
#[test]
fn corrupted_head_or_committed_wal_never_resets_or_falls_back() {
    for head in [true, false] {
        let t = Temp::new();
        let mut s = create(&t);
        s.submit("one", request(1), Mode::Durable).unwrap();
        let p = if head {
            t.db().join("HEAD")
        } else {
            path(&t.db(), 'W', s.head.wal)
        };
        drop(s);
        let mut b = fs::read(&p).unwrap();
        let i = b.len() - 1;
        b[i] ^= 1;
        fs::write(&p, b).unwrap();
        assert!(matches!(
            Store::open(t.db(), Limits::default()),
            Err(Error::Integrity(_))
        ));
        assert!(t.db().join("HEAD").exists());
    }
}
#[test]
fn committed_wal_short_read_and_missing_snapshot_fail_closed() {
    for missing in [true, false] {
        let t = Temp::new();
        let mut s = create(&t);
        s.submit("one", request(1), Mode::Durable).unwrap();
        let p = if missing {
            path(&t.db(), 'S', s.head.snapshot)
        } else {
            path(&t.db(), 'W', s.head.wal)
        };
        drop(s);
        if missing {
            fs::remove_file(p).unwrap();
        } else {
            let f = OpenOptions::new().write(true).open(p).unwrap();
            f.set_len(5).unwrap();
        }
        assert!(Store::open(t.db(), Limits::default()).is_err());
    }
}
#[test]
fn checksum_valid_forged_outcome_is_rejected_by_real_replay() {
    let t = Temp::new();
    let mut s = create(&t);
    s.submit("one", request(1), Mode::Durable).unwrap();
    let mut head = s.head.clone();
    drop(s);
    let p = path(&t.db(), 'W', head.wal);
    let b = fs::read(&p).unwrap();
    let mut pos = 0;
    let (_, tip) = parse(&b, &mut pos, Kind::Header, Limits::default()).unwrap();
    let prefix = b[..pos].to_vec();
    let (body, _) = parse(&b, &mut pos, Kind::Record, Limits::default()).unwrap();
    let mut r = R::new(body);
    r.hash().unwrap();
    r.u64().unwrap();
    let mut call = Call::read(&mut r, Limits::default()).unwrap();
    let state = r.hash().unwrap();
    call.outcome.push(255);
    let record = wal_record(&call, &head, tip, state, Limits::default()).unwrap();
    let mut forged = prefix;
    forged.extend_from_slice(&record);
    head.offset = forged.len() as u64;
    head.tip = record[record.len() - 32..].try_into().unwrap();
    fs::write(p, forged).unwrap();
    fs::write(t.db().join("HEAD"), head.encode(Limits::default()).unwrap()).unwrap();
    assert!(matches!(
        Store::open(t.db(), Limits::default()),
        Err(Error::Core(SnapshotError::OutcomeMismatch { .. }))
    ));
}
#[test]
fn injected_io_errors_fence_live_view_and_recover_only_old_or_new_committed_prefix() {
    for point in [
        "wal-half",
        "wal-written",
        "wal-synced",
        "head-half",
        "head-written",
        "head-synced",
        "head-renamed",
        "head-dir-synced",
    ] {
        let t = Temp::new();
        let mut s = create(&t);
        s.submit("one", request(1), Mode::Durable).unwrap();
        let e = fail_at(point, || s.submit("two", request(2), Mode::Durable));
        assert!(matches!(e, Err(Error::Indeterminate(_))), "{point}: {e:?}");
        assert!(s.recovery_required());
        assert!(matches!(s.session(), Err(Error::RecoveryRequired)));
        assert!(matches!(s.barrier(), Err(Error::RecoveryRequired)));
        assert!(matches!(s.lookup("one"), Err(Error::RecoveryRequired)));
        drop(s);
        let mut s = reopened(&t);
        assert_eq!(
            s.durable_seq(),
            if matches!(point, "head-renamed" | "head-dir-synced") {
                2
            } else {
                1
            },
            "{point}"
        );
        s.submit("two", request(2), Mode::Durable).unwrap();
        assert_eq!(s.live_seq(), 2);
    }
}
#[test]
fn checkpoint_flushes_volatile_state_rotates_and_keeps_host_ids() {
    let t = Temp::new();
    let mut s = create(&t);
    s.submit("one", request(1), Mode::Buffered).unwrap();
    s.checkpoint().unwrap();
    assert_eq!((s.head.base, s.head.seq), (1, 1));
    assert_eq!(s.wal.metadata().unwrap().len(), s.head.offset);
    let manifest = s.session().unwrap().state_manifest(s.limits.core).unwrap();
    drop(s);
    let mut s = reopened(&t);
    assert_eq!(
        s.session().unwrap().state_manifest(s.limits.core).unwrap(),
        manifest
    );
    s.submit("one", request(1), Mode::Durable).unwrap();
    s.submit("two", request(2), Mode::Durable).unwrap();
    assert_eq!(s.live_seq(), 2);
    assert_eq!(fs::read_dir(t.db()).unwrap().count(), 4);
}
#[test]
fn checkpoint_failures_never_destroy_the_selected_pair() {
    for point in [
        "generation-half",
        "generation-written",
        "generation-synced",
        "pair-dir-synced",
        "head-half",
        "head-written",
        "head-synced",
        "head-renamed",
        "head-dir-synced",
        "cleanup-unlinked",
    ] {
        let t = Temp::new();
        let mut s = create(&t);
        s.submit("one", request(1), Mode::Durable).unwrap();
        let e = fail_at(point, || s.checkpoint());
        assert!(matches!(e, Err(Error::Indeterminate(_))), "{point}: {e:?}");
        drop(s);
        let mut s = reopened(&t);
        assert_eq!(s.durable_seq(), 1);
        s.submit("one", request(1), Mode::Durable).unwrap();
        assert_eq!(s.live_seq(), 1);
    }
}
#[test]
fn checkpoint_budget_error_does_not_commit_volatile_suffix() {
    let t = Temp::new();
    let mut s = create(&t);
    s.submit("one", request(1), Mode::Buffered).unwrap();
    s.limits.max_frame_body = 1;
    assert!(matches!(s.checkpoint(), Err(Error::Limit(_))));
    assert_eq!(s.durable_seq(), 0);
    assert!(!s.recovery_required());
    drop(s);
    let s = reopened(&t);
    assert_eq!(s.live_seq(), 0);
}
#[test]
fn full_scalar_lifecycle_graph_roundtrips_from_wal_then_rotated_checkpoint() {
    let t = Temp::new();
    let mut graph = fixture::graph(IdentityProtocol::PortableV1);
    fixture::enrich(&mut graph);
    let mut s = Store::create(
        t.db(),
        fixture::registry(),
        IdentityProtocol::PortableV1,
        Limits::default(),
    )
    .unwrap();
    for i in 0..graph.session.operation_count() {
        let (req, outcome) = graph
            .session
            .recorded_call(i, SnapshotLimits::default())
            .unwrap();
        let receipt = s
            .submit_encoded(&format!("call/{i}"), &req, Mode::Buffered)
            .unwrap();
        assert_eq!(
            s.session()
                .unwrap()
                .recorded_call(i, SnapshotLimits::default())
                .unwrap()
                .1,
            outcome
        );
        assert_eq!(receipt.seq, i as u64 + 1);
    }
    s.barrier().unwrap();
    let expected = graph.session.export(SnapshotLimits::default()).unwrap();
    drop(s);
    let mut s = reopened(&t);
    assert_eq!(
        s.session()
            .unwrap()
            .export(SnapshotLimits::default())
            .unwrap(),
        expected
    );
    fixture::assert_equivalent(&graph.session, s.session().unwrap());
    s.checkpoint().unwrap();
    drop(s);
    let s = reopened(&t);
    assert_eq!(
        s.session()
            .unwrap()
            .export(SnapshotLimits::default())
            .unwrap(),
        expected
    );
}
#[test]
fn legacy_profile_is_preserved_without_native_to_portable_rebinding() {
    let t = Temp::new();
    let mut s = Store::create(
        t.db(),
        Registry::new(),
        IdentityProtocol::LegacyRustHashFnv64,
        Limits::default(),
    )
    .unwrap();
    s.submit("legacy", request(9), Mode::Durable).unwrap();
    let expected = s.session().unwrap().export(s.limits.core).unwrap();
    drop(s);
    let s = reopened(&t);
    assert_eq!(
        s.session().unwrap().pipeline().identity_protocol(),
        IdentityProtocol::LegacyRustHashFnv64
    );
    assert_eq!(
        s.session().unwrap().export(s.limits.core).unwrap(),
        expected
    );
}
#[test]
fn common_framing_matches_all_eight_independent_frozen_vectors() {
    let json = include_str!("../../../docs/evidence/wal-framing-v1.json");
    let expected: Vec<_> = json
        .split("\"frame_hex\": \"")
        .skip(1)
        .map(|s| s.split('"').next().unwrap())
        .collect();
    assert_eq!(expected.len(), 8);
    let mut i = 0;
    for kind in [Kind::Snapshot, Kind::Header, Kind::Record, Kind::Head] {
        for body in [vec![], vec![0, 1, 2, 127, 128, 254, 255]] {
            let b = frame(kind, body.clone(), Limits::default()).unwrap();
            let mut hex = String::new();
            for v in &b {
                use std::fmt::Write as _;
                write!(&mut hex, "{v:02x}").unwrap();
            }
            assert_eq!(hex, expected[i]);
            let mut p = 0;
            let (decoded, _) = parse(&b, &mut p, kind, Limits::default()).unwrap();
            assert_eq!(decoded, body);
            assert_eq!(p, b.len());
            for j in 0..b.len() {
                let mut bad = b.clone();
                bad[j] ^= 1;
                assert!(parse(&bad, &mut 0, kind, Limits::default()).is_err());
            }
            i += 1;
        }
    }
}
#[test]
fn decoder_rejects_lengths_versions_tags_truncation_and_head_invariants() {
    let mut b = frame(Kind::Head, vec![0; 4], Limits::default()).unwrap();
    b[18..26].copy_from_slice(&u64::MAX.to_le_bytes());
    assert!(matches!(
        parse(&b, &mut 0, Kind::Head, Limits::default()),
        Err(Error::Limit(_))
    ));
    let t = Temp::new();
    let s = create(&t);
    let mut h = s.head.clone();
    h.generation = 0;
    assert!(Head::decode(&h.encode(s.limits).unwrap(), s.limits).is_err());
    let bytes = s.head.encode(s.limits).unwrap();
    for n in 0..bytes.len() {
        assert!(Head::decode(&bytes[..n], s.limits).is_err());
    }
    let mut extra = bytes;
    extra.push(0);
    assert!(Head::decode(&extra, s.limits).is_err());
}

#[test]
fn arbitrary_uncommitted_suffix_is_ignored_without_decoding_and_truncated() {
    let t = Temp::new();
    let mut s = create(&t);
    s.submit("one", request(1), Mode::Durable).unwrap();
    let h = s.head.clone();
    drop(s);
    let p = path(&t.db(), 'W', h.wal);
    OpenOptions::new()
        .append(true)
        .open(&p)
        .unwrap()
        .write_all(&vec![255; 65536])
        .unwrap();
    let s = reopened(&t);
    assert_eq!(s.durable_seq(), 1);
    assert_eq!(fs::metadata(p).unwrap().len(), h.offset);
}
#[test]
fn checksum_valid_snapshot_receipt_cannot_substitute_a_different_core_request() {
    let t = Temp::new();
    let mut s = create(&t);
    s.submit("one", request(1), Mode::Durable).unwrap();
    s.checkpoint().unwrap();
    let mut h = s.head.clone();
    drop(s);
    let p = path(&t.db(), 'S', h.snapshot);
    let bytes = fs::read(&p).unwrap();
    let (body, _) = parse(&bytes, &mut 0, Kind::Snapshot, Limits::default()).unwrap();
    let mut r = R::new(body);
    r.engine().unwrap();
    r.hash().unwrap();
    r.u64().unwrap();
    r.u64().unwrap();
    let core = r.blob(Limits::default().core.max_snapshot_bytes).unwrap();
    r.u64().unwrap();
    let mut call = Call::read(&mut r, Limits::default()).unwrap();
    call.request.push(0);
    let mut w = W::new(Limits::default());
    w.u16(1).unwrap();
    w.raw(&h.session).unwrap();
    w.u64(h.snapshot).unwrap();
    w.u64(h.snapshot_seq).unwrap();
    w.blob(&core, Limits::default().core.max_snapshot_bytes)
        .unwrap();
    w.u64(1).unwrap();
    call.write(&mut w, Limits::default()).unwrap();
    let forged = frame(Kind::Snapshot, w.bytes, Limits::default()).unwrap();
    h.snapshot_hash = hash(&forged);
    fs::write(p, forged).unwrap();
    fs::write(t.db().join("HEAD"), h.encode(Limits::default()).unwrap()).unwrap();
    assert!(matches!(
        Store::open(t.db(), Limits::default()),
        Err(Error::Integrity("snapshot receipt vs journal"))
    ));
}
#[test]
fn foreign_wal_header_cannot_be_rebound_using_a_valid_frame_checksum() {
    let t = Temp::new();
    let s = create(&t);
    let h = s.head.clone();
    drop(s);
    let mut foreign = h.clone();
    foreign.session[0] ^= 1;
    let bytes = wal_header(&foreign, Limits::default()).unwrap();
    fs::write(path(&t.db(), 'W', h.wal), bytes).unwrap();
    assert!(matches!(
        Store::open(t.db(), Limits::default()),
        Err(Error::Integrity("WAL header binding"))
    ));
}
