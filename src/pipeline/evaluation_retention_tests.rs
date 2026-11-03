//! Collision injection changes only the private digest seam. Contracts, actual
//! criteria, canonical content, storage, lineage and projection remain real.
use super::*;
use crate::contract::conformance::*;
use crate::contract::dynamics::{CriterionOrder, DynamicsContext};
use crate::contract::lifecycle::*;
use crate::contract::projection::ProjectionProfile;
use crate::contract::registry::*;
use crate::contract::structure::{RelationContext, TemporalScope};
use crate::model::evaluation::EvaluationDiagnostic;
use crate::model::lifecycle::InvalidationRequest;
use alloc::{format, vec};

fn id(name: &str) -> VersionedId {
    VersionedId::new(name, 1).unwrap()
}
fn fixture() -> (Pipeline, [ObjectRef; 3]) {
    let mut registry = Registry::new();
    let domain = DomainContext::new("retention/fixture", id("ticks")).unwrap();
    let frame = RelationContext::new(
        domain.clone(),
        TemporalScope::new(0, 100).unwrap(),
        id("frame"),
        id("noncausal"),
    );
    let context = DynamicsContext::new(frame, id("calculus"), id("scalar"));
    registry
        .register_schema(SchemaContract {
            reference: id("schema"),
            format: ScalarSchema::SignedI64Le,
        })
        .unwrap();
    registry
        .register_source(SourceContract {
            reference: id("source"),
            schema: id("schema"),
            context: domain,
            unit: "count".into(),
        })
        .unwrap();
    registry
        .register_operator(OperatorContract {
            reference: id("read"),
            kind: OperatorKind::Scalar,
        })
        .unwrap();
    let mut criteria = Vec::new();
    for (name, kind) in [
        ("exists", ConformanceCriterionKind::TargetExists),
        ("lineage", ConformanceCriterionKind::LineageComplete),
        ("identity", ConformanceCriterionKind::IdentityCoherent),
        ("construction", ConformanceCriterionKind::ConstructionPass),
    ] {
        registry
            .register_conformance_criterion(ConformanceCriterionContract {
                reference: id(name),
                kind,
            })
            .unwrap();
        criteria.push(id(name));
    }
    for (name, target_type) in [
        ("O", TargetType::Observation),
        ("M", TargetType::Measurement),
    ] {
        registry
            .register_conformance(ConformanceContract {
                reference: id(&format!("check/{name}")),
                target_type,
                criteria: criteria.clone(),
                criterion_order: CriterionOrder::Unordered,
                context: context.clone(),
                missing_target: MissingTargetPolicy::Unresolved,
            })
            .unwrap();
        registry
            .register_projection(ProjectionProfile {
                reference: id(&format!("project/{name}")),
                schema: id("TEST-REP-01"),
                target_type,
                conformance: id(&format!("check/{name}")),
                context: context.clone(),
            })
            .unwrap();
        registry
            .register_lifecycle(LifecycleContract {
                reference: id(&format!("revoke/{name}")),
                target_type,
                context: context.clone(),
                allowed_reasons: vec![InvalidationReason::EvidenceWithdrawn],
                propagation: InvalidationPropagation::RequiredDeclaredInputs,
            })
            .unwrap();
    }
    let mut p = Pipeline::new(registry);
    let refs: Vec<_> = [3_i64, 4, 5]
        .into_iter()
        .enumerate()
        .map(|(i, n)| {
            p.admit(ObservationRequest {
                source: id("source"),
                timestamp: Some((i as u64 + 1) * 10),
                payload: n.to_le_bytes().to_vec(),
                acquisition_context: format!("record/{i}"),
            })
            .unwrap()
        })
        .collect();
    (p, refs.try_into().unwrap())
}
fn request(p: &Pipeline, kind: &str, target: &ObjectRef) -> EvaluationRequest {
    let contract = id(&format!("check/{kind}"));
    EvaluationRequest {
        context: p.registry.conformance(&contract).unwrap().context.clone(),
        contract,
        target: target.clone(),
    }
}
fn eval(p: &mut Pipeline, kind: &str, target: &ObjectRef) -> ObjectRef {
    let req = request(p, kind, target);
    p.evaluate(req).unwrap()
}
fn eval_keyed(p: &mut Pipeline, kind: &str, target: &ObjectRef) -> ObjectRef {
    let req = request(p, kind, target);
    p.evaluate_keyed(req).unwrap()
}
fn revoke(p: &mut Pipeline, target: &ObjectRef, witness: &ObjectRef) {
    p.invalidate(InvalidationRequest {
        request_id: "withdraw".into(),
        contract: id("revoke/O"),
        target: target.clone(),
        reason: InvalidationReason::EvidenceWithdrawn,
        evidence: vec![witness.clone()],
        expected_revision: p.lifecycle_revision(),
    })
    .unwrap();
}
fn project(
    p: &Pipeline,
    target: &ObjectRef,
    evaluation: &ObjectRef,
) -> Result<crate::model::representation::Representation, ProjectionError> {
    p.project(ProjectionRequest {
        profile: id("project/O"),
        target: target.clone(),
        evaluation: evaluation.clone(),
    })
}

#[test]
fn same_digest_retains_both_contents_and_never_reassigns_the_first_alias() {
    let (mut p, o) = fixture();
    p.evaluation_digest_override = Some(7);
    let a = eval(&mut p, "O", &o[0]);
    let original = p.resolve_evaluation(&a).unwrap().clone();
    let b = eval(&mut p, "O", &o[1]);
    assert_eq!(a, ObjectRef::Evaluation(7));
    assert!(matches!(b, ObjectRef::EvaluationKey(_)));
    assert_ne!(a, b);
    assert_eq!(p.evaluation_count(), 2);
    assert_eq!(p.resolve_evaluation(&a), Some(&original));
    assert_eq!(p.resolve_evaluation(&b).unwrap().target(), &o[1]);
    assert_eq!(eval(&mut p, "O", &o[0]), a);
    assert_eq!(eval(&mut p, "O", &o[1]), b);
    assert_eq!(p.evaluation_count(), 2);
}

#[test]
fn full_key_and_numeric_alias_resolve_the_same_stable_artifact_without_double_counting() {
    let (mut p, o) = fixture();
    p.evaluation_digest_override = Some(7);
    let legacy = eval(&mut p, "O", &o[0]);
    let key = eval_keyed(&mut p, "O", &o[0]);
    assert_eq!(p.canonical_evaluation_reference(&legacy), Some(key.clone()));
    assert_eq!(p.resolve_evaluation(&key), p.resolve_evaluation(&legacy));
    assert_eq!(p.evaluation_references(), vec![legacy.clone()]);
    assert_eq!(p.lifecycle_view(&key), p.lifecycle_view(&legacy));
    assert_eq!(p.evaluations_with_digest(7), vec![key]);
}

#[test]
fn keyed_first_never_mints_a_later_numeric_alias_for_that_object() {
    let (mut p, o) = fixture();
    p.evaluation_digest_override = Some(7);
    let first = eval_keyed(&mut p, "O", &o[0]);
    assert_eq!(eval(&mut p, "O", &o[0]), first);
    assert!(p.resolve_evaluation(&ObjectRef::Evaluation(7)).is_none());
    let second = eval(&mut p, "O", &o[1]);
    assert_eq!(second, ObjectRef::Evaluation(7));
    assert_eq!(p.resolve_evaluation(&first).unwrap().target(), &o[0]);
    assert_eq!(p.resolve_evaluation(&second).unwrap().target(), &o[1]);
    assert_eq!(p.evaluation_count(), 2);
}

#[test]
fn canonical_keyed_references_and_buckets_are_independent_of_collision_insertion_order() {
    let (mut a, oa) = fixture();
    let (mut b, ob) = fixture();
    a.evaluation_digest_override = Some(7);
    b.evaluation_digest_override = Some(7);
    let a0 = eval_keyed(&mut a, "O", &oa[0]);
    let a1 = eval_keyed(&mut a, "O", &oa[1]);
    let b1 = eval_keyed(&mut b, "O", &ob[1]);
    let b0 = eval_keyed(&mut b, "O", &ob[0]);
    assert_eq!((a0, a1), (b0, b1));
    assert_eq!(a.evaluations_with_digest(7), b.evaluations_with_digest(7));
    assert_eq!(a.evaluation_references(), b.evaluation_references());
}

#[test]
fn collision_with_a_core_object_keeps_both_namespaces_and_leaves_the_object_unchanged() {
    let (mut p, o) = fixture();
    let original = p.resolve_observation(&o[0]).unwrap().clone();
    p.evaluation_digest_override = Some(original.id());
    let e = eval(&mut p, "O", &o[1]);
    assert!(matches!(e, ObjectRef::EvaluationKey(_)));
    assert!(p
        .resolve_evaluation(&ObjectRef::Evaluation(original.id()))
        .is_none());
    assert_eq!(p.resolve_observation(&o[0]), Some(&original));
    assert_eq!(
        p.resolve_evaluation(&e).unwrap().result(),
        Some(crate::contract::evaluation::ConformanceResult::Pass)
    );
    assert!(project(&p, &o[1], &e).is_ok());
}

#[test]
fn pass_fail_unresolved_and_no_result_all_survive_in_one_digest_bucket() {
    use crate::contract::evaluation::ConformanceResult as R;
    let (mut p, o) = fixture();
    p.evaluation_digest_override = Some(7);
    let pass = eval(&mut p, "O", &o[0]);
    let fail = eval(&mut p, "M", &o[0]);
    let unresolved = eval(&mut p, "O", &ObjectRef::Observation(0));
    let mut unknown = request(&p, "O", &o[0]);
    unknown.contract = id("absent");
    let missing = p.evaluate(unknown).unwrap();
    for (reference, result) in [
        (pass, Some(R::Pass)),
        (fail, Some(R::Fail)),
        (unresolved, Some(R::Unresolved)),
        (missing, None),
    ] {
        assert_eq!(p.resolve_evaluation(&reference).unwrap().result(), result);
    }
    assert_eq!(p.evaluations_with_digest(7).len(), 4);
    assert_eq!(p.evaluation_count(), 4);
}

#[test]
fn projection_uses_the_exact_key_and_rejects_another_target_or_failed_member() {
    let (mut p, o) = fixture();
    p.evaluation_digest_override = Some(7);
    let a = eval(&mut p, "O", &o[0]);
    let b = eval(&mut p, "O", &o[1]);
    let bad = eval(&mut p, "M", &o[1]);
    assert!(project(&p, &o[0], &a).is_ok());
    assert!(project(&p, &o[1], &b).is_ok());
    assert_eq!(
        project(&p, &o[0], &b),
        Err(ProjectionError::EvaluationMismatch)
    );
    assert_eq!(
        project(&p, &o[1], &bad),
        Err(ProjectionError::ConformanceRequired)
    );
}

#[test]
fn revocation_distinguishes_colliding_results_and_blocks_both_alias_forms() {
    let (mut p, o) = fixture();
    p.evaluation_digest_override = Some(7);
    let a = eval(&mut p, "O", &o[0]);
    let ak = p.canonical_evaluation_reference(&a).unwrap();
    let b = eval(&mut p, "O", &o[1]);
    let old = p.resolve_evaluation(&a).unwrap().canonical_basis();
    revoke(&mut p, &o[0], &o[2]);
    assert_eq!(p.lifecycle_view(&ak), p.lifecycle_view(&a));
    assert_eq!(
        p.lifecycle_view(&a).unwrap().status,
        LifecycleStatus::Invalid
    );
    assert_eq!(
        p.lifecycle_view(&b).unwrap().status,
        LifecycleStatus::Conformant
    );
    assert_eq!(
        p.lifecycle_view_at(&ak, 0).unwrap().status,
        LifecycleStatus::Conformant
    );
    assert_eq!(
        project(&p, &o[0], &ak),
        Err(ProjectionError::InvalidatedInput(o[0].clone()))
    );
    assert!(project(&p, &o[1], &b).is_ok());
    assert_eq!(p.resolve_evaluation(&a).unwrap().canonical_basis(), old);
    let fresh = eval(&mut p, "O", &o[0]);
    assert_ne!(fresh, a);
    assert_ne!(fresh, b);
    assert!(p
        .resolve_evaluation(&fresh)
        .unwrap()
        .diagnostics()
        .contains(&EvaluationDiagnostic::TargetInvalidated));
    assert_eq!(p.evaluation_count(), 3);
    assert_eq!(p.resolve_evaluation(&a).unwrap().target(), &o[0]);
}

#[test]
fn keyed_primary_revocation_has_no_numeric_alias_escape() {
    let (mut p, o) = fixture();
    p.evaluation_digest_override = Some(7);
    let a = eval_keyed(&mut p, "O", &o[0]);
    let b = eval_keyed(&mut p, "O", &o[1]);
    revoke(&mut p, &o[0], &o[2]);
    assert!(p.resolve_evaluation(&ObjectRef::Evaluation(7)).is_none());
    assert_eq!(
        p.lifecycle_view(&a).unwrap().status,
        LifecycleStatus::Invalid
    );
    assert_eq!(
        p.lifecycle_view(&b).unwrap().status,
        LifecycleStatus::Conformant
    );
    assert!(project(&p, &o[1], &b).is_ok());
}

#[test]
fn invalidated_keyed_digest_does_not_block_an_unrelated_numeric_core_identity() {
    let (mut p, o) = fixture();
    let input = p.resolve_input(&o[1]).unwrap();
    let measurement =
        Measurement::derive(p.registry.operator(&id("read")).unwrap(), &[input]).unwrap();
    p.evaluation_digest_override = Some(measurement.id());
    let e = eval_keyed(&mut p, "O", &o[0]);
    revoke(&mut p, &o[0], &o[2]);
    let m = p
        .measure(MeasurementRequest {
            operator: id("read"),
            inputs: vec![o[1].clone()],
        })
        .unwrap();
    assert_eq!(p.resolve_measurement(&m).unwrap().id(), measurement.id());
    assert_eq!(
        p.lifecycle_view(&e).unwrap().status,
        LifecycleStatus::Invalid
    );
    assert_eq!(
        p.lifecycle_view(&m).unwrap().status,
        LifecycleStatus::Candidate
    );
}

#[test]
fn reserved_legacy_digest_still_guards_core_objects_without_overwriting_the_evaluation() {
    let (mut p, o) = fixture();
    let input = p.resolve_input(&o[1]).unwrap();
    let measurement =
        Measurement::derive(p.registry.operator(&id("read")).unwrap(), &[input]).unwrap();
    p.evaluation_digest_override = Some(measurement.id());
    let e = eval(&mut p, "O", &o[0]);
    let before = p.resolve_evaluation(&e).unwrap().canonical_basis();
    assert_eq!(
        p.measure(MeasurementRequest {
            operator: id("read"),
            inputs: vec![o[1].clone()]
        }),
        Err(PipelineError::IdentityCollision(measurement.id()))
    );
    assert_eq!(p.resolve_evaluation(&e).unwrap().canonical_basis(), before);
    assert_eq!(p.evaluation_count(), 1);
}

#[test]
fn lineage_and_captured_invalid_inputs_resolve_the_exact_evaluation_key() {
    let (mut p, o) = fixture();
    p.evaluation_digest_override = Some(7);
    let _ = eval(&mut p, "O", &o[0]);
    let b = eval(&mut p, "O", &o[1]);
    let trace = p.trace(&b).unwrap();
    assert!(trace.contains(&o[1]));
    assert!(!trace.contains(&o[0]));
    assert_eq!(
        p.measure(MeasurementRequest {
            operator: id("read"),
            inputs: vec![b.clone()]
        }),
        Err(PipelineError::InvalidInputType(b.clone()))
    );
    let invalid = p
        .resolve_invalid(p.rejections().last().unwrap().invalid_reference())
        .unwrap();
    assert!(invalid
        .captured_inputs()
        .iter()
        .any(|snapshot| snapshot.reference() == &b
            && snapshot.basis() == p.resolve_evaluation(&b).unwrap().canonical_basis()));
}

#[test]
fn an_unknown_full_key_never_falls_back_to_a_known_digest_member() {
    let (mut p, o) = fixture();
    p.evaluation_digest_override = Some(7);
    let _ = eval(&mut p, "O", &o[0]);
    let forged = ObjectRef::EvaluationKey(EvaluationId::for_artifact(7, vec![1, 2, 3]));
    assert!(p.resolve_evaluation(&forged).is_none());
    assert!(p.canonical_evaluation_reference(&forged).is_none());
    assert!(p.trace(&forged).is_err());
    assert!(!p.lineage_report(&forged).is_complete());
    assert_eq!(
        project(&p, &o[0], &forged),
        Err(ProjectionError::EvaluationUnresolved)
    );
    assert_eq!(p.evaluation_count(), 1);
}

#[test]
fn a_keyed_evaluation_is_not_a_supported_conformance_target_and_is_not_treated_as_missing() {
    let (mut p, o) = fixture();
    p.evaluation_digest_override = Some(7);
    let target = eval_keyed(&mut p, "O", &o[0]);
    let invalid = eval_keyed(&mut p, "O", &target);
    let e = p.resolve_evaluation(&invalid).unwrap();
    assert!(e
        .diagnostics()
        .contains(&EvaluationDiagnostic::InvalidTargetType));
    assert!(!e
        .diagnostics()
        .contains(&EvaluationDiagnostic::TargetUnresolved));
    assert!(p.lineage_report(&invalid).resolved.contains(&target));
}

#[test]
fn pure_keyed_collision_graphs_have_deterministic_lifecycle_events() {
    let (mut a, oa) = fixture();
    let (mut b, ob) = fixture();
    a.evaluation_digest_override = Some(7);
    b.evaluation_digest_override = Some(7);
    let _ = eval_keyed(&mut a, "O", &oa[0]);
    let _ = eval_keyed(&mut a, "O", &oa[1]);
    let _ = eval_keyed(&mut b, "O", &ob[1]);
    let _ = eval_keyed(&mut b, "O", &ob[0]);
    revoke(&mut a, &oa[0], &oa[2]);
    revoke(&mut b, &ob[0], &ob[2]);
    assert_eq!(a.lifecycle_events(), b.lifecycle_events());
}

#[test]
fn a_mismatching_digest_does_not_resolve_an_existing_full_body() {
    let (mut p, o) = fixture();
    p.evaluation_digest_override = Some(7);
    let existing = eval_keyed(&mut p, "O", &o[0]);
    let basis = p.resolve_evaluation(&existing).unwrap().canonical_basis();
    let wrong = ObjectRef::EvaluationKey(EvaluationId::for_artifact(8, basis));
    assert!(p.resolve_evaluation(&wrong).is_none());
    assert!(p.evaluations_with_digest(8).is_empty());
    assert_eq!(p.evaluation_count(), 1);
}
