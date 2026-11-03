//! Exact recorded-history restoration through public core operations.
#[allow(dead_code)]
#[path = "../examples/support/snapshot_fixture.rs"]
mod fixture;

use fixture::*;
use mdc_core::contract::evaluation::{ConformanceResult as R, LifecycleStatus as S};
use mdc_core::contract::registry::Registry;
use mdc_core::identity::IdentityProtocol;
use mdc_core::model::lifecycle::LifecycleError;
use mdc_core::pipeline::*;
use mdc_core::provenance::ObjectRef;

fn restore(session: &SnapshotSession) -> SnapshotSession {
    SnapshotSession::restore(
        &session.export(SnapshotLimits::default()).unwrap(),
        SnapshotLimits::default(),
    )
    .unwrap()
}

#[test]
fn empty_registry_and_history_roundtrip_without_synthesizing_state() {
    for protocol in [
        IdentityProtocol::LegacyRustHashFnv64,
        IdentityProtocol::PortableV1,
    ] {
        let original = SnapshotSession::new(Registry::new(), protocol);
        let bytes = original.export(SnapshotLimits::default()).unwrap();
        let restored = SnapshotSession::restore(&bytes, SnapshotLimits::default()).unwrap();
        assert_eq!(restored.pipeline().identity_protocol(), protocol);
        assert!(restored.inventory().is_empty());
        assert_eq!(restored.operation_count(), 0);
        assert_eq!(restored.pipeline().lifecycle_revision(), 0);
        assert!(restored.pipeline().rejections().is_empty());
        assert!(restored.pipeline().lifecycle_rejections().is_empty());
        assert_eq!(
            original.state_manifest(SnapshotLimits::default()).unwrap(),
            restored.state_manifest(SnapshotLimits::default()).unwrap()
        );
        assert_eq!(restored.export(SnapshotLimits::default()).unwrap(), bytes);
    }
}

#[test]
fn full_typed_state_and_exact_archive_roundtrip_in_both_profiles() {
    for protocol in [
        IdentityProtocol::LegacyRustHashFnv64,
        IdentityProtocol::PortableV1,
    ] {
        let mut g = graph(protocol);
        enrich(&mut g);
        let before = g.session.state_manifest(SnapshotLimits::default()).unwrap();
        let bytes = g.session.export(SnapshotLimits::default()).unwrap();
        assert_eq!(
            before,
            g.session.state_manifest(SnapshotLimits::default()).unwrap(),
            "export mutated its source"
        );
        let restored = SnapshotSession::restore(&bytes, SnapshotLimits::default()).unwrap();
        assert_equivalent(&g.session, &restored);
        assert_eq!(
            restored.state_manifest(SnapshotLimits::default()).unwrap(),
            before
        );
        assert_eq!(restored.export(SnapshotLimits::default()).unwrap(), bytes);
    }
}

#[test]
fn unused_exact_version_definitions_are_restored_and_support_new_work() {
    let mut g = graph(IdentityProtocol::PortableV1);
    let mut restored = restore(&g.session);
    assert_registry_equal(
        g.session.pipeline().registry(),
        restored.pipeline().registry(),
    );
    let request = ObservationRequest {
        source: version("unused/source", 2),
        timestamp: Some(75),
        payload: 42_i64.to_le_bytes().to_vec(),
        acquisition_context: "unused/source/75".into(),
    };
    let left = g.session.admit(request.clone()).unwrap();
    let right = restored.admit(request).unwrap();
    assert_eq!(left, right);
    let request = MeasurementRequest {
        operator: version("unused/read", 2),
        inputs: vec![left],
    };
    assert_eq!(
        g.session.measure(request.clone()),
        restored.measure(request)
    );
    let request = MeasurementRequest {
        operator: version("unused/difference", 2),
        inputs: vec![g.roots[1].clone(), g.roots[1].clone()],
    };
    let a = g.session.measure(request.clone()).unwrap();
    let b = restored.measure(request).unwrap();
    assert_eq!(a, b);
    assert_eq!(
        restored.pipeline().resolve_measurement(&b).unwrap().value(),
        0
    );
    let missing_version = MeasurementRequest {
        operator: id("unused/read"),
        inputs: vec![right],
    };
    assert!(g.session.measure(missing_version.clone()).is_err());
    assert!(restored.measure(missing_version).is_err());
    assert_equivalent(&g.session, &restored);
}

#[test]
fn all_evaluation_results_and_reference_policies_survive_restoration() {
    let mut g = graph(IdentityProtocol::PortableV1);
    let request = evaluation_request(g.session.pipeline(), "O", &g.roots[0]);
    let key_first = g.session.evaluate_keyed(request.clone()).unwrap();
    let ObjectRef::EvaluationKey(key) = &key_first else {
        panic!("expected a canonical handle")
    };
    let digest = key.digest();
    assert!(g
        .session
        .pipeline()
        .resolve_evaluation(&ObjectRef::Evaluation(digest))
        .is_none());
    let mut restored = restore(&g.session);
    assert_eq!(g.session.evaluate(request.clone()).unwrap(), key_first);
    assert_eq!(restored.evaluate(request).unwrap(), key_first);
    assert!(restored
        .pipeline()
        .resolve_evaluation(&ObjectRef::Evaluation(digest))
        .is_none());
    assert!(matches!(g.evaluation, ObjectRef::Evaluation(_)));
    let full = restored
        .pipeline()
        .canonical_evaluation_reference(&g.evaluation)
        .unwrap();
    assert_eq!(
        restored.pipeline().resolve_evaluation(&full),
        restored.pipeline().resolve_evaluation(&g.evaluation)
    );
    assert_equivalent(&g.session, &restored);

    let history = enrich(&mut g);
    let restored = restore(&g.session);
    for (reference, result) in [
        (&g.evaluation, Some(R::Pass)),
        (&history.unresolved_evaluation, Some(R::Unresolved)),
        (&history.failed_evaluation, Some(R::Fail)),
        (&history.unknown_evaluation, None),
    ] {
        assert_eq!(
            restored
                .pipeline()
                .resolve_evaluation(reference)
                .unwrap()
                .result(),
            result
        );
    }
    assert_equivalent(&g.session, &restored);
}

#[test]
fn invalid_types_audit_multiplicity_and_old_missing_snapshot_are_preserved() {
    let mut g = graph(IdentityProtocol::PortableV1);
    let history = enrich(&mut g);
    let restored = restore(&g.session);
    let invalids: Vec<_> = restored
        .inventory()
        .into_iter()
        .filter(|r| restored.pipeline().resolve_invalid(r).is_some())
        .collect();
    for expected in [
        "Observation",
        "Measurement",
        "Relation",
        "Composition",
        "State",
        "Transition",
    ] {
        assert!(invalids.iter().any(|r| format!(
            "{:?}",
            restored.pipeline().resolve_invalid(r).unwrap().kind()
        ) == expected));
    }
    assert!(restored
        .pipeline()
        .resolve_observation(&history.late_input)
        .is_some());
    assert!(restored
        .pipeline()
        .lineage_report(&history.old_missing)
        .unresolved
        .contains(&history.late_input));
    assert!(restored.pipeline().rejections().len() > restored.pipeline().invalid_count());
    assert_equivalent(&g.session, &restored);
}

#[test]
fn lifecycle_epochs_rejections_and_request_idempotence_are_restored_exactly() {
    let mut g = graph(IdentityProtocol::PortableV1);
    let history = enrich(&mut g);
    let mut restored = restore(&g.session);
    assert_eq!(restored.pipeline().lifecycle_revision(), 2);
    assert_eq!(
        restored.pipeline().lifecycle_view_at(&history.born_late, 0),
        Err(LifecycleError::NotYetAdmitted(history.born_late.clone()))
    );
    assert_eq!(
        restored
            .pipeline()
            .lifecycle_view(&history.born_late)
            .unwrap()
            .admitted_revision,
        2
    );
    assert!(restored
        .pipeline()
        .lifecycle_view_at(&history.born_late, 3)
        .is_err());
    assert_eq!(
        restored
            .pipeline()
            .resolve_lifecycle_event(&history.first_event),
        g.session
            .pipeline()
            .resolve_lifecycle_event(&history.first_event)
    );
    assert_eq!(
        restored
            .pipeline()
            .resolve_lifecycle_event(&history.second_event),
        g.session
            .pipeline()
            .resolve_lifecycle_event(&history.second_event)
    );
    assert_eq!(
        g.session.invalidate(history.first_request.clone()).unwrap(),
        history.first_event
    );
    assert_eq!(
        restored.invalidate(history.first_request).unwrap(),
        history.first_event
    );
    assert_eq!(restored.pipeline().lifecycle_revision(), 2);
    assert_eq!(restored.pipeline().lifecycle_rejections().len(), 2);
    assert_equivalent(&g.session, &restored);
}

#[test]
fn restored_usability_and_projection_boundaries_match_the_original() {
    let mut g = graph(IdentityProtocol::PortableV1);
    let history = enrich(&mut g);
    let mut restored = restore(&g.session);
    let request = MeasurementRequest {
        operator: id("read"),
        inputs: vec![g.roots[0].clone()],
    };
    let original_failure = g.session.measure(request.clone());
    let restored_failure = restored.measure(request);
    assert!(original_failure.is_err());
    assert_eq!(original_failure, restored_failure);
    let request = ProjectionRequest {
        profile: id("project/T"),
        target: g.roots[5].clone(),
        evaluation: g.evaluation.clone(),
    };
    assert_eq!(
        g.session.pipeline().project(request.clone()),
        restored.pipeline().project(request)
    );
    assert_eq!(
        restored
            .pipeline()
            .lifecycle_view(&g.evaluation)
            .unwrap()
            .status,
        S::Invalid
    );
    assert_eq!(
        restored
            .pipeline()
            .resolve_evaluation(&g.evaluation)
            .unwrap()
            .result(),
        Some(R::Pass)
    );
    let request = MeasurementRequest {
        operator: id("read"),
        inputs: vec![history.born_late],
    };
    let a = g.session.measure(request.clone()).unwrap();
    let b = restored.measure(request).unwrap();
    assert_eq!(a, b);
    assert_eq!(
        restored.pipeline().resolve_measurement(&b).unwrap().value(),
        99
    );
    assert_equivalent(&g.session, &restored);
}

#[test]
fn recorded_archive_keeps_idempotent_calls_and_failures_in_original_order() {
    let mut a = SnapshotSession::new(registry(), IdentityProtocol::PortableV1);
    let mut b = SnapshotSession::new(registry(), IdentityProtocol::PortableV1);
    let request = observation(70, 7);
    let first = a.admit(request.clone()).unwrap();
    assert_eq!(b.admit(request.clone()).unwrap(), first);
    assert_eq!(b.admit(request).unwrap(), first);
    assert_eq!(
        a.pipeline().observation_count(),
        b.pipeline().observation_count()
    );
    assert_eq!(basis(a.pipeline(), &first), basis(b.pipeline(), &first));
    assert_eq!(a.operation_count(), 1);
    assert_eq!(b.operation_count(), 2);
    assert_ne!(
        a.export(SnapshotLimits::default()).unwrap(),
        b.export(SnapshotLimits::default()).unwrap()
    );
    assert_equivalent(&b, &restore(&b));
    let mut g = graph(IdentityProtocol::PortableV1);
    enrich(&mut g);
    assert_eq!(
        g.session.operation_count(),
        38,
        "all successes, failures and idempotents must be recorded"
    );
}

#[test]
fn readonly_queries_and_external_projection_do_not_change_snapshot_state() {
    let g = graph(IdentityProtocol::PortableV1);
    let before = g.session.export(SnapshotLimits::default()).unwrap();
    let count = g.session.operation_count();
    g.session.pipeline().trace(&g.evaluation).unwrap();
    g.session
        .pipeline()
        .lifecycle_view_at(&g.evaluation, 0)
        .unwrap();
    let mut representation = g
        .session
        .pipeline()
        .project(ProjectionRequest {
            profile: id("project/T"),
            target: g.roots[5].clone(),
            evaluation: g.evaluation.clone(),
        })
        .unwrap();
    representation.label = "edited detached representation".into();
    assert!(g
        .session
        .pipeline()
        .apply_representation(&representation)
        .is_err());
    assert_eq!(count, g.session.operation_count());
    assert_eq!(before, g.session.export(SnapshotLimits::default()).unwrap());
}

#[test]
fn malformed_truncated_unknown_version_and_trailing_archives_are_rejected() {
    let empty = SnapshotSession::new(Registry::new(), IdentityProtocol::PortableV1);
    let bytes = empty.export(SnapshotLimits::default()).unwrap();
    assert_eq!(&bytes[..17], b"MDC-SNAPSHOT-001\0");
    for length in 0..bytes.len() {
        assert!(
            SnapshotSession::restore(&bytes[..length], SnapshotLimits::default()).is_err(),
            "accepted prefix {length}"
        );
    }
    for offset in [0, 17, 19, 21] {
        let mut bad = bytes.clone();
        bad[offset] = 0xff;
        assert!(
            SnapshotSession::restore(&bad, SnapshotLimits::default()).is_err(),
            "accepted corrupt header at {offset}"
        );
    }
    let mut tail = bytes.clone();
    tail.push(0);
    assert!(SnapshotSession::restore(&tail, SnapshotLimits::default()).is_err());
    let mut changed_manifest = bytes.clone();
    *changed_manifest.last_mut().unwrap() ^= 1;
    assert!(SnapshotSession::restore(&changed_manifest, SnapshotLimits::default()).is_err());
    assert_eq!(empty.export(SnapshotLimits::default()).unwrap(), bytes);
}

#[test]
fn every_configured_budget_rejects_work_without_changing_the_source_session() {
    let g = graph(IdentityProtocol::PortableV1);
    let bytes = g.session.export(SnapshotLimits::default()).unwrap();
    let before = g.session.state_manifest(SnapshotLimits::default()).unwrap();
    let setters: [fn(&mut SnapshotLimits); 7] = [
        |l| l.max_snapshot_bytes = 0,
        |l| l.max_field_bytes = 0,
        |l| l.max_collection_items = 0,
        |l| l.max_total_items = 0,
        |l| l.max_definitions = 0,
        |l| l.max_operations = 0,
        |l| l.max_manifest_bytes = 0,
    ];
    for (index, set) in setters.into_iter().enumerate() {
        let mut limit = SnapshotLimits::default();
        set(&mut limit);
        assert!(g.session.export(limit).is_err(), "export budget {index}");
        let mut limit = SnapshotLimits::default();
        set(&mut limit);
        assert!(
            SnapshotSession::restore(&bytes, limit).is_err(),
            "import budget {index}"
        );
    }
    assert_eq!(
        before,
        g.session.state_manifest(SnapshotLimits::default()).unwrap()
    );
    assert_eq!(g.session.export(SnapshotLimits::default()).unwrap(), bytes);
}

#[test]
fn exact_archive_byte_limit_succeeds_and_one_byte_less_is_rejected() {
    let g = graph(IdentityProtocol::PortableV1);
    let bytes = g.session.export(SnapshotLimits::default()).unwrap();
    let exact = SnapshotLimits {
        max_snapshot_bytes: bytes.len(),
        ..SnapshotLimits::default()
    };
    assert_eq!(g.session.export(exact).unwrap(), bytes);
    let exact = SnapshotLimits {
        max_snapshot_bytes: bytes.len(),
        ..SnapshotLimits::default()
    };
    let restored = SnapshotSession::restore(&bytes, exact).unwrap();
    assert_equivalent(&g.session, &restored);
    let short = SnapshotLimits {
        max_snapshot_bytes: bytes.len() - 1,
        ..SnapshotLimits::default()
    };
    assert!(g.session.export(short).is_err());
    let short = SnapshotLimits {
        max_snapshot_bytes: bytes.len() - 1,
        ..SnapshotLimits::default()
    };
    assert!(SnapshotSession::restore(&bytes, short).is_err());
}

#[test]
fn recorded_outcome_tampering_is_rejected_even_with_an_unchanged_manifest() {
    let mut session = SnapshotSession::new(registry(), IdentityProtocol::PortableV1);
    let reference = session.admit(observation(10, 12)).unwrap();
    let ObjectRef::Observation(id) = reference else {
        panic!("observation")
    };
    let mut bytes = session.export(SnapshotLimits::default()).unwrap();
    let mut needle = 10_u64.to_le_bytes().to_vec();
    needle.extend_from_slice(&[0, 1]);
    needle.extend_from_slice(&id.to_le_bytes());
    let position = bytes
        .windows(needle.len())
        .position(|w| w == needle)
        .unwrap();
    bytes[position + 8] = 255;
    assert!(matches!(
        SnapshotSession::restore(&bytes, SnapshotLimits::default()),
        Err(SnapshotError::OutcomeMismatch { operation: 0 })
    ));
}

#[test]
fn legacy_layout_mismatch_is_rejected_before_replay() {
    let session = SnapshotSession::new(Registry::new(), IdentityProtocol::LegacyRustHashFnv64);
    let bytes = session.export(SnapshotLimits::default()).unwrap();
    for position in [22, 23] {
        let mut altered = bytes.clone();
        altered[position] ^= 1;
        assert!(matches!(
            SnapshotSession::restore(&altered, SnapshotLimits::default()),
            Err(SnapshotError::LegacyEnvironmentMismatch)
        ));
    }
}

#[test]
fn invalid_raw_lookup_keys_are_preserved_in_recorded_requests() {
    use mdc_core::provenance::EvaluationId;
    let mut session = SnapshotSession::new(registry(), IdentityProtocol::PortableV1);
    let reference = ObjectRef::EvaluationKey(EvaluationId::from_canonical_basis(vec![1, 2, 3]));
    assert!(session
        .measure(MeasurementRequest {
            operator: id("read"),
            inputs: vec![reference]
        })
        .is_err());
    let restored = restore(&session);
    assert_equivalent(&session, &restored);
    assert_eq!(
        session.export(SnapshotLimits::default()).unwrap(),
        restored.export(SnapshotLimits::default()).unwrap()
    );
}
