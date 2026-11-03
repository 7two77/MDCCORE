#[allow(dead_code)]
#[path = "../examples/lifecycle.rs"]
mod fixture;
use fixture::*;
use mdc_core::contract::evaluation::{ConformanceResult as R, LifecycleStatus as S};
use mdc_core::contract::lifecycle::*;
use mdc_core::contract::registry::{RegistryError, VersionedId};
use mdc_core::identity::CanonicalSemanticBasis;
use mdc_core::model::evaluation::EvaluationDiagnostic;
use mdc_core::model::lifecycle::*;
use mdc_core::pipeline::*;
use mdc_core::provenance::ObjectRef;

const KINDS: [&str; 6] = ["O", "M", "R", "C", "S", "T"];

#[test]
fn all_six_roots_propagate_only_to_actual_descendants_and_keep_every_snapshot() {
    for (index, kind) in KINDS.iter().enumerate() {
        let mut g = graph();
        let before = capture(&g.p, &g.refs);
        let expected: Vec<_> = g
            .refs
            .iter()
            .filter(|r| g.p.trace(r).unwrap().contains(&g.roots[index]))
            .cloned()
            .collect();
        let old_evaluation = basis(&g.p, &g.evaluation);
        let req = request(&g.p, "revoke", kind, &g.roots[index], &g.witness);
        let id = g.p.invalidate(req).unwrap();
        assert_eq!(g.p.lifecycle_revision(), 1);
        let event = g.p.resolve_lifecycle_event(&id).unwrap();
        assert_eq!(event.affected().len(), 7 - index);
        for r in &g.refs {
            let view = g.p.lifecycle_view(r).unwrap();
            assert_eq!(
                view.status,
                if expected.contains(r) {
                    S::Invalid
                } else if r == &g.evaluation {
                    S::Conformant
                } else {
                    S::Candidate
                }
            );
            assert_eq!(
                g.p.lifecycle_view_at(r, 0).unwrap().status,
                if r == &g.evaluation {
                    S::Conformant
                } else {
                    S::Candidate
                }
            );
        }
        for snapshot in event.affected() {
            assert_eq!(snapshot.semantic_basis(), before[snapshot.reference()]);
        }
        assert!(event
            .captured_lineage()
            .contains(&ObjectRef::Source(id_source())));
        assert!(event.unresolved_references().is_empty());
        assert_eq!(before, capture(&g.p, &g.refs));
        assert_eq!(old_evaluation, basis(&g.p, &g.evaluation));
        assert_eq!(
            g.p.resolve_evaluation(&g.evaluation).unwrap().result(),
            Some(R::Pass)
        );
        assert_eq!(
            g.p.resolve_evaluation(&g.evaluation).unwrap().status(),
            S::Conformant
        );
        assert_eq!(g.p.trace(&g.evaluation).unwrap().len(), 17);
    }
}
fn id_source() -> VersionedId {
    id("source")
}

#[test]
fn reevaluation_of_all_six_invalidated_types_retains_epoch_and_causal_events() {
    for (index, kind) in KINDS.iter().enumerate() {
        let mut g = graph();
        let req = request(&g.p, "revoke", kind, &g.roots[index], &g.witness);
        let event = g.p.invalidate(req).unwrap();
        let new = evaluate(&mut g.p, kind, &g.roots[index]);
        let e = g.p.resolve_evaluation(&new).unwrap();
        assert_eq!(e.status(), S::Invalid);
        assert_eq!(e.result(), Some(R::Fail));
        assert!(e
            .diagnostics()
            .contains(&EvaluationDiagnostic::TargetInvalidated));
        assert_eq!(
            e.target_basis(),
            Some(basis(&g.p, &g.roots[index]).as_slice())
        );
        let ctx = e.lifecycle_context().unwrap();
        assert_eq!(ctx.revision, 1);
        assert_eq!(ctx.invalidated_by, vec![event]);
        assert_ne!(new, g.evaluation);
        assert_eq!(
            g.p.lifecycle_view_at(&new, 0),
            Err(LifecycleError::NotYetAdmitted(new.clone()))
        );
        assert_eq!(g.p.lifecycle_view(&new).unwrap().status, S::Invalid);
    }
}

fn reconstruct(g: &mut Graph, index: usize) -> Result<ObjectRef, PipelineError> {
    match index {
        0 => {
            let o = g.p.resolve_observation(&g.roots[0]).unwrap();
            g.p.admit(ObservationRequest {
                source: o.source_ref().clone(),
                timestamp: Some(o.timestamp()),
                payload: o.payload().to_vec(),
                acquisition_context: o.provenance().acquisition_context().into(),
            })
        }
        1 => g.p.measure(MeasurementRequest {
            operator: id("read"),
            inputs: vec![g.roots[0].clone()],
        }),
        2 => {
            let r = g.p.resolve_relation(&g.roots[2]).unwrap();
            g.p.relate(RelationRequest {
                contract: r.contract().clone(),
                left: r.left_ref().clone(),
                right: r.right_ref().clone(),
            })
        }
        3 => {
            let c = g.p.resolve_composition(&g.roots[3]).unwrap();
            g.p.compose(CompositionRequest {
                contract: c.contract().clone(),
                members: c.members().to_vec(),
                relations: c.relations().to_vec(),
            })
        }
        4 => g.p.derive_state(StateRequest {
            contract: id("state"),
            composition: g.roots[3].clone(),
            observation_horizon: Some(50),
        }),
        5 => g.p.transition(TransitionRequest {
            contract: id("transition"),
            source: g.roots[4].clone(),
            target: g.second_state.clone(),
            observation_horizon: Some(50),
        }),
        _ => unreachable!(),
    }
}

#[test]
fn repeated_construction_cannot_resurrect_any_of_the_six_revoked_identities() {
    for (i, kind) in KINDS.iter().enumerate() {
        let mut g = graph();
        let root = g.roots[i].clone();
        let req = request(&g.p, "revoke", kind, &root, &g.witness);
        let event = g.p.invalidate(req).unwrap();
        let before = capture(&g.p, &g.refs);
        let error = reconstruct(&mut g, i).unwrap_err();
        assert_eq!(
            error,
            PipelineError::InputInvalidated {
                reference: root,
                revision: 1,
                events: vec![event]
            }
        );
        assert_eq!(before, capture(&g.p, &g.refs));
        let invalid = g.p.rejections().last().unwrap().invalid_reference();
        assert_eq!(g.p.resolve_invalid(invalid).unwrap().error(), &error);
    }
}

#[test]
fn all_downstream_operations_reject_revoked_inputs_with_retained_diagnostics() {
    let mut g = graph();
    let req = request(&g.p, "revoke", "O", &g.roots[0], &g.witness);
    g.p.invalidate(req).unwrap();
    for i in 1..6 {
        assert!(matches!(
            reconstruct(&mut g, i),
            Err(PipelineError::InputInvalidated { revision: 1, .. })
        ));
    }
    assert_eq!(g.p.invalid_count(), 5);
    assert_eq!(g.p.rejections().len(), 5);
    for rejection in g.p.rejections() {
        assert!(g.p.resolve_invalid(rejection.invalid_reference()).is_some());
    }
}

#[test]
fn old_conformant_evaluation_and_external_representation_do_not_authorize_current_projection() {
    let mut g = graph();
    let input = ProjectionRequest {
        profile: id("project/T"),
        target: g.roots[5].clone(),
        evaluation: g.evaluation.clone(),
    };
    let representation = g.p.project(input.clone()).unwrap();
    let bytes = representation.canonical_bytes();
    let req = request(&g.p, "revoke", "O", &g.roots[0], &g.witness);
    g.p.invalidate(req).unwrap();
    assert_eq!(
        g.p.project(input),
        Err(ProjectionError::InvalidatedInput(g.roots[5].clone()))
    );
    assert_eq!(representation.canonical_bytes(), bytes);
    assert_eq!(
        g.p.apply_representation(&representation),
        Err(ProjectionError::BackflowForbidden)
    );
}

#[test]
fn active_unrelated_branch_can_still_derive_evaluate_and_project() {
    let mut g = graph();
    let req = request(&g.p, "revoke", "O", &g.roots[0], &g.witness);
    g.p.invalidate(req).unwrap();
    let target = g.second_state.clone();
    let evaluation = evaluate(&mut g.p, "S", &target);
    assert_eq!(
        g.p.resolve_evaluation(&evaluation).unwrap().result(),
        Some(R::Pass)
    );
    assert!(g
        .p
        .resolve_evaluation(&evaluation)
        .unwrap()
        .lifecycle_context()
        .is_none());
    assert!(g
        .p
        .project(ProjectionRequest {
            profile: id("project/S"),
            target,
            evaluation
        })
        .is_ok());
    let measurement =
        g.p.measure(MeasurementRequest {
            operator: id("read"),
            inputs: vec![g.witness],
        })
        .unwrap();
    assert_eq!(
        g.p.lifecycle_view(&measurement).unwrap().status,
        S::Candidate
    );
}

#[test]
fn duplicate_request_is_idempotent_and_conflicting_request_id_is_audited() {
    let mut g = graph();
    let req = request(&g.p, "same-id", "O", &g.roots[0], &g.witness);
    let first = g.p.invalidate(req.clone()).unwrap();
    assert_eq!(g.p.invalidate(req.clone()).unwrap(), first);
    assert_eq!(g.p.lifecycle_events().len(), 1);
    let mut conflict = req;
    conflict.reason = InvalidationReason::SourceContractViolation;
    assert_eq!(
        g.p.invalidate(conflict.clone()),
        Err(LifecycleError::RequestIdConflict)
    );
    assert_eq!(g.p.lifecycle_rejections().last().unwrap().request, conflict);
    assert_eq!(g.p.lifecycle_revision(), 1);
}

#[test]
fn independent_causes_are_retained_and_older_lifecycle_views_do_not_change() {
    let mut g = graph();
    let first = request(&g.p, "first", "O", &g.roots[0], &g.witness);
    let e1 = g.p.invalidate(first).unwrap();
    let frozen = g.p.lifecycle_view(&g.roots[5]).unwrap();
    let other = g
        .refs
        .iter()
        .find(|r| {
            g.p.resolve_observation(r)
                .is_some_and(|o| o.timestamp() == 20)
        })
        .unwrap()
        .clone();
    let second = request(&g.p, "second", "O", &other, &g.witness);
    let e2 = g.p.invalidate(second).unwrap();
    assert_eq!(g.p.lifecycle_view_at(&g.roots[5], 1).unwrap(), frozen);
    assert_eq!(
        g.p.lifecycle_view(&g.roots[5]).unwrap().invalidated_by,
        vec![e1, e2]
    );
    assert_eq!(
        g.p.lifecycle_view(&g.second_state).unwrap().status,
        S::Candidate
    );
}

#[test]
fn assessments_at_different_invalidation_epochs_keep_distinct_causal_contexts() {
    let mut g = graph();
    let first = request(&g.p, "first", "O", &g.roots[0], &g.witness);
    g.p.invalidate(first).unwrap();
    let e1 = evaluate(&mut g.p, "T", &g.roots[5]);
    let old = basis(&g.p, &e1);
    let other = g
        .refs
        .iter()
        .find(|r| {
            g.p.resolve_observation(r)
                .is_some_and(|o| o.timestamp() == 20)
        })
        .unwrap()
        .clone();
    let second = request(&g.p, "second", "O", &other, &g.witness);
    g.p.invalidate(second).unwrap();
    let e2 = evaluate(&mut g.p, "T", &g.roots[5]);
    assert_ne!(e1, e2);
    assert_eq!(basis(&g.p, &e1), old);
    assert_eq!(
        g.p.resolve_evaluation(&e1)
            .unwrap()
            .lifecycle_context()
            .unwrap()
            .invalidated_by
            .len(),
        1
    );
    assert_eq!(
        g.p.resolve_evaluation(&e2)
            .unwrap()
            .lifecycle_context()
            .unwrap()
            .invalidated_by
            .len(),
        2
    );
}

#[test]
fn stale_or_future_revision_is_explicit_and_has_no_semantic_effects() {
    let mut g = graph();
    let before = capture(&g.p, &g.refs);
    let mut req = request(&g.p, "stale", "O", &g.roots[0], &g.witness);
    req.expected_revision = 8;
    assert_eq!(
        g.p.invalidate(req),
        Err(LifecycleError::RevisionConflict {
            expected: 8,
            actual: 0
        })
    );
    assert_eq!(
        g.p.lifecycle_view_at(&g.roots[0], 1),
        Err(LifecycleError::FutureRevision)
    );
    assert_eq!(g.p.lifecycle_revision(), 0);
    assert_eq!(before, capture(&g.p, &g.refs));
}

#[test]
fn invalidation_requires_exact_contract_version_type_context_and_allowed_reason() {
    let mut g = graph();
    let base = request(&g.p, "bad", "O", &g.roots[0], &g.witness);
    let mut req = base.clone();
    req.contract = VersionedId::new("lifecycle/O", 2).unwrap();
    assert!(matches!(
        g.p.invalidate(req),
        Err(LifecycleError::ContractUnresolved(_))
    ));
    let mut req = base.clone();
    req.target = g.roots[1].clone();
    assert!(matches!(
        g.p.invalidate(req),
        Err(LifecycleError::UnsupportedTarget(_))
    ));
    let mut r = registry();
    let mut contract = r.lifecycle(&id("lifecycle/O")).unwrap().clone();
    contract.reference = id("restricted");
    contract.allowed_reasons = vec![InvalidationReason::SourceContractViolation];
    r.register_lifecycle(contract).unwrap();
    let mut wrong = r.lifecycle(&id("lifecycle/S")).unwrap().clone();
    wrong.reference = id("wrong-context");
    wrong.context = mdc_core::contract::dynamics::DynamicsContext::new(
        wrong.context.frame().clone(),
        id("other-calculus"),
        wrong.context.domain_version().clone(),
    );
    r.register_lifecycle(wrong).unwrap();
    let mut h = graph_with_registry(r);
    let mut req = request(&h.p, "restriction", "O", &h.roots[0], &h.witness);
    req.contract = id("restricted");
    assert_eq!(h.p.invalidate(req), Err(LifecycleError::ReasonNotAllowed));
    let mut req = request(&h.p, "restriction", "S", &h.roots[4], &h.witness);
    req.contract = id("wrong-context");
    assert_eq!(h.p.invalidate(req), Err(LifecycleError::ContextMismatch));
    assert_eq!(g.p.lifecycle_revision(), 0);
}

#[test]
fn missing_targets_and_unsupported_sources_or_evaluations_are_not_synthesized() {
    let mut g = graph();
    let mut req = request(&g.p, "absent", "O", &ObjectRef::Observation(0), &g.witness);
    assert!(matches!(
        g.p.invalidate(req.clone()),
        Err(LifecycleError::TargetUnresolved(_))
    ));
    req.target = ObjectRef::Source(id("source"));
    assert!(matches!(
        g.p.invalidate(req.clone()),
        Err(LifecycleError::UnsupportedTarget(_))
    ));
    req.target = g.evaluation.clone();
    assert!(matches!(
        g.p.invalidate(req),
        Err(LifecycleError::UnsupportedTarget(_))
    ));
    assert_eq!(g.p.observation_count(), 5);
    assert_eq!(g.p.lifecycle_events().len(), 0);
}

#[test]
fn malformed_or_unusable_witnesses_are_rejected_atomically() {
    let mut g = graph();
    let base = request(&g.p, "bad", "O", &g.roots[0], &g.witness);
    let mut req = base.clone();
    req.request_id = "  ".into();
    assert_eq!(g.p.invalidate(req), Err(LifecycleError::EmptyRequestId));
    let mut req = base.clone();
    req.evidence.clear();
    assert_eq!(g.p.invalidate(req), Err(LifecycleError::EmptyEvidence));
    let mut req = base.clone();
    req.evidence.push(g.witness.clone());
    assert_eq!(g.p.invalidate(req), Err(LifecycleError::DuplicateEvidence));
    let mut req = base.clone();
    req.evidence = vec![ObjectRef::Observation(0)];
    assert!(matches!(
        g.p.invalidate(req),
        Err(LifecycleError::EvidenceInvalid(_))
    ));
    assert_eq!(g.p.lifecycle_revision(), 0);
    let revoke = request(&g.p, "witness", "O", &g.witness, &g.roots[0]);
    g.p.invalidate(revoke).unwrap();
    let mut req = base;
    req.expected_revision = 1;
    assert_eq!(
        g.p.invalidate(req),
        Err(LifecycleError::EvidenceInvalid(g.witness.clone()))
    );
    assert_eq!(g.p.lifecycle_revision(), 1);
}

#[test]
fn witness_invalidation_revokes_confirmed_state_and_new_derivation_uses_only_active_facts() {
    let mut g = graph();
    let confirmed =
        g.p.derive_state(StateRequest {
            contract: id("pending"),
            composition: g.roots[3].clone(),
            observation_horizon: Some(100),
        })
        .unwrap();
    assert_eq!(
        g.p.resolve_state(&confirmed).unwrap().criteria_result(),
        R::Pass
    );
    let old = basis(&g.p, &confirmed);
    let evaluation = evaluate(&mut g.p, "S", &confirmed);
    let req = request(&g.p, "withdraw-confirmation", "O", &g.witness, &g.roots[0]);
    let event = g.p.invalidate(req).unwrap();
    let affected = g.p.resolve_lifecycle_event(&event).unwrap().affected();
    assert!(affected.iter().any(|s| s.reference() == &confirmed));
    assert!(affected.iter().any(|s| s.reference() == &evaluation));
    assert_eq!(
        g.p.lifecycle_view(&g.roots[3]).unwrap().status,
        S::Candidate
    );
    assert_eq!(
        g.p.lifecycle_view(&g.roots[4]).unwrap().status,
        S::Candidate
    );
    let pending =
        g.p.derive_state(StateRequest {
            contract: id("pending"),
            composition: g.roots[3].clone(),
            observation_horizon: Some(100),
        })
        .unwrap();
    assert_ne!(pending, confirmed);
    assert_eq!(
        g.p.resolve_state(&pending).unwrap().criteria_result(),
        R::Unresolved
    );
    assert_eq!(g.p.resolve_state(&pending).unwrap().status(), S::Candidate);
    assert_eq!(basis(&g.p, &confirmed), old);
}

#[test]
fn unrelated_new_facts_do_not_repair_revoked_identity_or_rewrite_event_capture() {
    let mut g = graph();
    let req = request(&g.p, "withdraw", "O", &g.roots[0], &g.witness);
    let id = g.p.invalidate(req).unwrap();
    let old = g.p.resolve_lifecycle_event(&id).unwrap().canonical_basis();
    let fresh =
        g.p.admit(ObservationRequest {
            source: fixture::id("source"),
            timestamp: Some(10),
            payload: 12_i64.to_le_bytes().to_vec(),
            acquisition_context: "corrected/acquisition/10".into(),
        })
        .unwrap();
    assert_ne!(fresh, g.roots[0]);
    assert_eq!(g.p.lifecycle_view(&fresh).unwrap().status, S::Candidate);
    assert_eq!(g.p.lifecycle_view(&g.roots[0]).unwrap().status, S::Invalid);
    assert_eq!(
        g.p.resolve_lifecycle_event(&id).unwrap().canonical_basis(),
        old
    );
    assert!(matches!(
        g.p.lifecycle_view_at(&fresh, 0),
        Err(LifecycleError::NotYetAdmitted(_))
    ));
}

#[test]
fn equivalent_graphs_and_unordered_witness_sets_produce_the_same_event() {
    let mut a = graph();
    let mut b = graph();
    let mut ar = request(&a.p, "same", "O", &a.roots[0], &a.witness);
    ar.evidence.push(a.second_state.clone());
    let mut br = request(&b.p, "same", "O", &b.roots[0], &b.witness);
    br.evidence = vec![b.second_state.clone(), b.witness.clone()];
    let ai = a.p.invalidate(ar.clone()).unwrap();
    let bi = b.p.invalidate(br).unwrap();
    assert_eq!(ai, bi);
    assert_eq!(
        ai.canonical_key(),
        a.p.resolve_lifecycle_event(&ai).unwrap().canonical_basis()
    );
    ar.evidence.reverse();
    assert_eq!(a.p.invalidate(ar).unwrap(), ai);
    assert_eq!(a.p.lifecycle_revision(), 1);
}

#[test]
fn registry_preserves_exact_versions_and_normalizes_only_reason_sets() {
    let mut r = registry();
    let original = r.lifecycle(&id("lifecycle/O")).unwrap().clone();
    assert!(matches!(
        r.register_lifecycle(original.clone()),
        Err(RegistryError::DuplicateReference(_))
    ));
    let mut invalid = original.clone();
    invalid.reference = id("empty");
    invalid.allowed_reasons.clear();
    assert_eq!(
        r.register_lifecycle(invalid),
        Err(RegistryError::InvalidLifecycleReasons)
    );
    let mut invalid = original.clone();
    invalid.reference = id("duplicate");
    invalid.allowed_reasons.push(invalid.allowed_reasons[0]);
    assert_eq!(
        r.register_lifecycle(invalid),
        Err(RegistryError::InvalidLifecycleReasons)
    );
    let mut reversed = original.clone();
    reversed.reference = VersionedId::new("lifecycle/O", 2).unwrap();
    reversed.allowed_reasons.reverse();
    r.register_lifecycle(reversed.clone()).unwrap();
    assert_eq!(
        r.lifecycle(&reversed.reference).unwrap().allowed_reasons,
        original.allowed_reasons
    );
    assert_eq!(r.lifecycle(&original.reference), Some(&original));
}

#[test]
fn partial_older_evaluation_does_not_block_propagation_or_lose_missing_definition_diagnostics() {
    let mut g = graph();
    let context =
        g.p.registry()
            .conformance(&id("check/T"))
            .unwrap()
            .context
            .clone();
    let old =
        g.p.evaluate(EvaluationRequest {
            contract: id("unknown-contract"),
            target: g.roots[5].clone(),
            context,
        })
        .unwrap();
    let old_bytes = basis(&g.p, &old);
    let req = request(&g.p, "withdraw", "O", &g.roots[0], &g.witness);
    let event = g.p.invalidate(req).unwrap();
    let record = g.p.resolve_lifecycle_event(&event).unwrap();
    assert!(record.affected().iter().any(|s| s.reference() == &old));
    assert!(record
        .unresolved_definitions()
        .iter()
        .any(|d| d.reference == id("unknown-contract")));
    assert_eq!(basis(&g.p, &old), old_bytes);
    assert_eq!(g.p.lifecycle_view(&old).unwrap().status, S::Invalid);
}

#[test]
fn rejection_snapshots_keep_the_original_epoch_after_later_invalidation() {
    let mut g = graph();
    let req = request(&g.p, "first", "O", &g.roots[0], &g.witness);
    g.p.invalidate(req).unwrap();
    let first_error = reconstruct(&mut g, 1).unwrap_err();
    let reference = g.p.rejections().last().unwrap().invalid_reference().clone();
    let bytes = g.p.resolve_invalid(&reference).unwrap().canonical_basis();
    let other = g
        .refs
        .iter()
        .find(|r| {
            g.p.resolve_observation(r)
                .is_some_and(|o| o.timestamp() == 20)
        })
        .unwrap()
        .clone();
    let req = request(&g.p, "second", "O", &other, &g.witness);
    g.p.invalidate(req).unwrap();
    assert_eq!(
        g.p.resolve_invalid(&reference).unwrap().canonical_basis(),
        bytes
    );
    assert_eq!(
        g.p.resolve_invalid(&reference).unwrap().error(),
        &first_error
    );
    let second_error = reconstruct(&mut g, 1).unwrap_err();
    assert_ne!(first_error, second_error);
    assert_ne!(
        g.p.rejections().last().unwrap().invalid_reference(),
        &reference
    );
}

#[test]
fn unknown_conformance_retains_lifecycle_context_without_inventing_a_semantic_result() {
    let mut g = graph();
    let req = request(&g.p, "withdraw", "O", &g.roots[0], &g.witness);
    let cause = g.p.invalidate(req).unwrap();
    let context =
        g.p.registry()
            .conformance(&id("check/T"))
            .unwrap()
            .context
            .clone();
    let e =
        g.p.evaluate(EvaluationRequest {
            contract: id("not-defined"),
            target: g.roots[5].clone(),
            context,
        })
        .unwrap();
    let evaluation = g.p.resolve_evaluation(&e).unwrap();
    assert_eq!(evaluation.result(), None);
    assert_eq!(evaluation.status(), S::Invalid);
    assert_eq!(
        evaluation.diagnostics(),
        &[EvaluationDiagnostic::ContractUnresolved]
    );
    assert_eq!(
        evaluation.lifecycle_context().unwrap().invalidated_by,
        vec![cause]
    );
}
