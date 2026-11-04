//! Core journal bridge used by the separately compiled host adapter.
#[allow(dead_code)]
#[path = "../examples/support/snapshot_fixture.rs"]
mod fixture;
use mdc_core::{identity::IdentityProtocol, pipeline::*};
#[test]
fn prepared_call_is_isolated_and_outcome_matches_production_api() {
    let limits = SnapshotLimits::default();
    let mut original = SnapshotSession::new(fixture::registry(), IdentityProtocol::PortableV1);
    let before = original.export(limits).unwrap();
    let request = fixture::observation(10, 5);
    let bytes =
        SnapshotSession::encode_request(&SnapshotRequest::Admit(request.clone()), limits).unwrap();
    let (candidate, out, typed) = original.prepare_call(&bytes, None, limits).unwrap();
    assert_eq!(original.export(limits).unwrap(), before);
    let expected = original.admit(request);
    assert_eq!(typed, SnapshotOutcome::Object(expected));
    assert_eq!(candidate.recorded_call(0, limits).unwrap(), (bytes, out));
    assert_eq!(
        candidate.export(limits).unwrap(),
        original.export(limits).unwrap()
    );
}
#[test]
fn wrong_expected_outcome_bad_request_and_budget_failure_preserve_source() {
    let l = SnapshotLimits::default();
    let original = SnapshotSession::new(fixture::registry(), IdentityProtocol::PortableV1);
    let before = original.export(l).unwrap();
    let request =
        SnapshotSession::encode_request(&SnapshotRequest::Admit(fixture::observation(10, 5)), l)
            .unwrap();
    assert!(matches!(
        original.prepare_call(&request, Some(&[]), l),
        Err(SnapshotError::OutcomeMismatch { operation: 0 })
    ));
    assert!(original.prepare_call(&[255], None, l).is_err());
    let limited = SnapshotLimits {
        max_operations: 0,
        ..l
    };
    assert!(matches!(
        original.prepare_call(&request, None, limited),
        Err(SnapshotError::LimitExceeded(_))
    ));
    assert_eq!(original.export(l).unwrap(), before);
    assert!(original.recorded_call(0, l).is_err());
    assert_eq!(original.recorded_outcome(0), None);
}
#[test]
fn all_nine_recorded_operations_replay_with_exact_bytes_and_semantic_failures() {
    let l = SnapshotLimits::default();
    let mut graph = fixture::graph(IdentityProtocol::PortableV1);
    fixture::enrich(&mut graph);
    let mut replay = SnapshotSession::new(fixture::registry(), IdentityProtocol::PortableV1);
    let mut tags = std::collections::BTreeSet::new();
    for i in 0..graph.session.operation_count() {
        let (req, out) = graph.session.recorded_call(i, l).unwrap();
        tags.insert(req[0]);
        let decoded = SnapshotSession::decode_request(&req, l).unwrap();
        assert_eq!(SnapshotSession::encode_request(&decoded, l).unwrap(), req);
        let (next, actual, typed) = replay.prepare_call(&req, Some(&out), l).unwrap();
        assert_eq!(actual, out);
        assert_eq!(Some(typed), graph.session.recorded_outcome(i));
        replay = next;
    }
    assert_eq!(tags, (0..9).collect());
    assert_eq!(replay.export(l).unwrap(), graph.session.export(l).unwrap());
}
#[test]
fn request_decoder_requires_eof_and_does_not_repair_raw_handles() {
    let l = SnapshotLimits::default();
    let mut g = fixture::graph(IdentityProtocol::PortableV1);
    fixture::enrich(&mut g);
    for i in 0..g.session.operation_count() {
        let (req, _) = g.session.recorded_call(i, l).unwrap();
        let mut trailing = req.clone();
        trailing.push(255);
        assert!(SnapshotSession::decode_request(&trailing, l).is_err());
        for n in [0, req.len() - 1] {
            assert!(SnapshotSession::decode_request(&req[..n], l).is_err());
        }
    }
}
