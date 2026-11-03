#[allow(dead_code)]
#[path = "../examples/lifecycle.rs"]
mod fixture;
use fixture::*;
use mdc_core::contract::conformance::*;
use mdc_core::contract::evaluation::{ConformanceResult as R, LifecycleStatus as S};
use mdc_core::identity::CanonicalSemanticBasis;
use mdc_core::model::evaluation::EvaluationDiagnostic;
use mdc_core::pipeline::*;
use mdc_core::provenance::{EvaluationId, ObjectRef};

fn keyed(p: &mut Pipeline, kind: &str, target: &ObjectRef) -> ObjectRef {
    let contract = id(&format!("check/{kind}"));
    let context = p.registry().conformance(&contract).unwrap().context.clone();
    p.evaluate_keyed(EvaluationRequest {
        contract,
        target: target.clone(),
        context,
    })
    .unwrap()
}

#[test]
fn canonical_lookup_and_projection_work_for_all_six_target_types() {
    let mut g = graph();
    let before = capture(&g.p, &g.refs);
    for (kind, target) in ["O", "M", "R", "C", "S", "T"].iter().zip(g.roots.iter()) {
        let e = keyed(&mut g.p, kind, target);
        let ObjectRef::EvaluationKey(key) = &e else {
            panic!("canonical API must return a key")
        };
        let artifact = g.p.resolve_evaluation(&e).unwrap();
        assert_eq!(key, &artifact.key());
        assert_eq!(
            *key,
            EvaluationId::from_canonical_basis(artifact.canonical_basis())
        );
        assert_eq!(artifact.id(), key.digest());
        assert_eq!(g.p.canonical_evaluation_reference(&e), Some(e.clone()));
        assert!(g
            .p
            .project(ProjectionRequest {
                profile: id(&format!("project/{kind}")),
                target: target.clone(),
                evaluation: e
            })
            .is_ok());
    }
    assert_eq!(g.p.evaluation_count(), 6);
    assert_eq!(before, capture(&g.p, &g.refs));
}

#[test]
fn reference_policy_does_not_change_evaluation_semantics_bytes_or_digest() {
    let mut a = graph();
    let mut b = graph();
    for (i, kind) in ["O", "M", "R", "C", "S", "T"].iter().enumerate() {
        let old = fixture::evaluate(&mut a.p, kind, &a.roots[i]);
        let new = keyed(&mut b.p, kind, &b.roots[i]);
        assert!(matches!(old, ObjectRef::Evaluation(_)));
        assert!(matches!(new, ObjectRef::EvaluationKey(_)));
        assert_eq!(
            a.p.resolve_evaluation(&old).unwrap(),
            b.p.resolve_evaluation(&new).unwrap()
        );
        assert_eq!(a.p.canonical_evaluation_reference(&old), Some(new));
    }
}

#[test]
fn current_lifecycle_and_history_are_consistent_through_both_reference_forms() {
    let mut g = graph();
    let old = g.evaluation.clone();
    let key = keyed(&mut g.p, "T", &g.roots[5]);
    let before = g.p.resolve_evaluation(&old).unwrap().canonical_basis();
    let revoke = request(&g.p, "withdraw", "O", &g.roots[0], &g.witness);
    g.p.invalidate(revoke).unwrap();
    assert_eq!(g.p.lifecycle_view(&old), g.p.lifecycle_view(&key));
    assert_eq!(
        g.p.lifecycle_view_at(&old, 0),
        g.p.lifecycle_view_at(&key, 0)
    );
    assert_eq!(g.p.lifecycle_view(&key).unwrap().status, S::Invalid);
    assert_eq!(
        g.p.resolve_evaluation(&key).unwrap().canonical_basis(),
        before
    );
    let later = keyed(&mut g.p, "T", &g.roots[5]);
    assert_ne!(later, key);
    let assessment = g.p.resolve_evaluation(&later).unwrap();
    assert_eq!(assessment.result(), Some(R::Fail));
    assert!(assessment
        .diagnostics()
        .contains(&EvaluationDiagnostic::TargetInvalidated));
}

#[test]
fn actual_failed_unresolved_and_unknown_contract_assessments_are_retained_as_keys() {
    let mut r = registry();
    r.register_conformance_criterion(ConformanceCriterionContract {
        reference: id("three"),
        kind: ConformanceCriterionKind::MemberCount { min: 3, max: 3 },
    })
    .unwrap();
    let mut fail = r.conformance(&id("check/C")).unwrap().clone();
    fail.reference = id("fail/C");
    fail.criteria.push(id("three"));
    r.register_conformance(fail).unwrap();
    let mut g = graph_with_registry(r);
    let pending =
        g.p.derive_state(StateRequest {
            contract: id("pending"),
            composition: g.roots[3].clone(),
            observation_horizon: Some(50),
        })
        .unwrap();
    let unknown = keyed(&mut g.p, "S", &pending);
    assert_eq!(
        g.p.resolve_evaluation(&unknown).unwrap().result(),
        Some(R::Unresolved)
    );
    assert_eq!(
        g.p.resolve_evaluation(&unknown).unwrap().status(),
        S::Candidate
    );
    let context =
        g.p.registry()
            .conformance(&id("check/C"))
            .unwrap()
            .context
            .clone();
    let failed =
        g.p.evaluate_keyed(EvaluationRequest {
            contract: id("fail/C"),
            target: g.roots[3].clone(),
            context: context.clone(),
        })
        .unwrap();
    assert_eq!(
        g.p.resolve_evaluation(&failed).unwrap().result(),
        Some(R::Fail)
    );
    assert!(g
        .p
        .resolve_evaluation(&failed)
        .unwrap()
        .diagnostics()
        .is_empty());
    let missing =
        g.p.evaluate_keyed(EvaluationRequest {
            contract: id("undefined"),
            target: g.roots[3].clone(),
            context,
        })
        .unwrap();
    assert_eq!(g.p.resolve_evaluation(&missing).unwrap().result(), None);
    assert!(matches!(
        (unknown, failed, missing),
        (
            ObjectRef::EvaluationKey(_),
            ObjectRef::EvaluationKey(_),
            ObjectRef::EvaluationKey(_)
        )
    ));
}

#[test]
fn a_lookup_key_does_not_admit_arbitrary_bytes_or_retype_an_observation() {
    let g = graph();
    for bytes in [
        vec![0],
        g.p.resolve_observation(&g.roots[0])
            .unwrap()
            .canonical_basis(),
    ] {
        let reference = ObjectRef::EvaluationKey(EvaluationId::from_canonical_basis(bytes));
        assert!(g.p.resolve_evaluation(&reference).is_none());
        assert!(g.p.evaluation_key(&reference).is_none());
        assert!(!g.p.lineage_report(&reference).is_complete());
    }
    assert_eq!(g.p.evaluation_count(), 1);
}

#[test]
fn external_representation_preserves_the_selected_handle_without_mutating_core_content() {
    let g = graph();
    let before = capture(&g.p, &g.refs);
    let key = g.p.canonical_evaluation_reference(&g.evaluation).unwrap();
    let legacy =
        g.p.project(ProjectionRequest {
            profile: id("project/T"),
            target: g.roots[5].clone(),
            evaluation: g.evaluation.clone(),
        })
        .unwrap();
    let canonical =
        g.p.project(ProjectionRequest {
            profile: id("project/T"),
            target: g.roots[5].clone(),
            evaluation: key.clone(),
        })
        .unwrap();
    assert_eq!(canonical.evaluation(), &key);
    assert_eq!(legacy.evaluation_basis(), canonical.evaluation_basis());
    assert_eq!(legacy.source_basis(), canonical.source_basis());
    assert_ne!(legacy.canonical_bytes(), canonical.canonical_bytes());
    assert_eq!(before, capture(&g.p, &g.refs));
}
