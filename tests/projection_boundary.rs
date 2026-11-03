//! Registered projection remains outside the admitted calculus graph.
#[allow(dead_code)]
#[path = "../examples/state_transition.rs"]
mod fixture;
use mdc_core::contract::conformance::*;
use mdc_core::contract::dynamics::{CriterionOrder, StateCriterionContract, StateCriterionKind};
use mdc_core::contract::evaluation::{ConformanceResult, LifecycleStatus};
use mdc_core::contract::projection::ProjectionProfile;
use mdc_core::contract::registry::{Registry, RegistryError, VersionedId};
use mdc_core::identity::CanonicalSemanticBasis;
use mdc_core::pipeline::{
    EvaluationRequest, Pipeline, ProjectionError, ProjectionRequest, StateRequest,
    TransitionRequest,
};
use mdc_core::provenance::ObjectRef;

fn id(name: &str) -> VersionedId {
    VersionedId::new(name, 1).unwrap()
}
fn registry() -> Registry {
    let mut registry = fixture::registry();
    let context = registry.state(&id("state")).unwrap().context.clone();
    let mut criteria = Vec::new();
    for (name, kind) in [
        ("check/exists", ConformanceCriterionKind::TargetExists),
        ("check/lineage", ConformanceCriterionKind::LineageComplete),
        (
            "check/construction",
            ConformanceCriterionKind::ConstructionPass,
        ),
    ] {
        let reference = id(name);
        registry
            .register_conformance_criterion(ConformanceCriterionContract {
                reference: reference.clone(),
                kind,
            })
            .unwrap();
        criteria.push(reference);
    }
    for (name, target_type) in [
        ("observation", TargetType::Observation),
        ("measurement", TargetType::Measurement),
        ("relation", TargetType::Relation),
        ("composition", TargetType::Composition),
        ("state", TargetType::State),
        ("transition", TargetType::Transition),
    ] {
        let conformance = id(&format!("check/{name}"));
        registry
            .register_conformance(ConformanceContract {
                reference: conformance.clone(),
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
                conformance,
                context: context.clone(),
            })
            .unwrap();
    }
    registry
        .register_state_criterion(StateCriterionContract {
            reference: id("after"),
            kind: StateCriterionKind::ObservationAfter {
                source: id("source"),
            },
        })
        .unwrap();
    let mut pending = registry.state(&id("state")).unwrap().clone();
    pending.reference = id("pending");
    pending.criteria.push(id("after"));
    registry.register_state(pending).unwrap();
    registry
        .register_conformance_criterion(ConformanceCriterionContract {
            reference: id("count3"),
            kind: ConformanceCriterionKind::MemberCount { min: 3, max: 3 },
        })
        .unwrap();
    let mut fail = registry
        .conformance(&id("check/composition"))
        .unwrap()
        .clone();
    fail.reference = id("fail");
    fail.criteria.push(id("count3"));
    registry.register_conformance(fail).unwrap();
    registry
}
fn evaluation(pipeline: &mut Pipeline, contract: &str, target: &ObjectRef) -> ObjectRef {
    let context = pipeline
        .registry()
        .conformance(&id(contract))
        .unwrap()
        .context
        .clone();
    pipeline
        .evaluate(EvaluationRequest {
            contract: id(contract),
            target: target.clone(),
            context,
        })
        .unwrap()
}
fn graph() -> (Pipeline, ObjectRef) {
    let mut pipeline = Pipeline::new(registry());
    let source = fixture::state(&mut pipeline, 10, [12, 6]);
    let target = fixture::state(&mut pipeline, 30, [8, 4]);
    let transition = pipeline
        .transition(TransitionRequest {
            contract: id("transition"),
            source,
            target,
            observation_horizon: Some(50),
        })
        .unwrap();
    (pipeline, transition)
}
fn request(kind: &str, target: &ObjectRef, evaluation: &ObjectRef) -> ProjectionRequest {
    ProjectionRequest {
        profile: id(&format!("project/{kind}")),
        target: target.clone(),
        evaluation: evaluation.clone(),
    }
}
fn capture(pipeline: &Pipeline, refs: &[ObjectRef]) -> Vec<Vec<u8>> {
    refs.iter()
        .filter_map(|reference| match reference {
            ObjectRef::Observation(_) => Some(
                pipeline
                    .resolve_observation(reference)
                    .unwrap()
                    .canonical_basis(),
            ),
            ObjectRef::Measurement(_) => Some(
                pipeline
                    .resolve_measurement(reference)
                    .unwrap()
                    .canonical_basis(),
            ),
            ObjectRef::Relation(_) => Some(
                pipeline
                    .resolve_relation(reference)
                    .unwrap()
                    .canonical_basis(),
            ),
            ObjectRef::Composition(_) => Some(
                pipeline
                    .resolve_composition(reference)
                    .unwrap()
                    .canonical_basis(),
            ),
            ObjectRef::State(_) => {
                Some(pipeline.resolve_state(reference).unwrap().canonical_basis())
            }
            ObjectRef::Transition(_) => Some(
                pipeline
                    .resolve_transition(reference)
                    .unwrap()
                    .canonical_basis(),
            ),
            ObjectRef::Evaluation(_) | ObjectRef::EvaluationKey(_) => Some(
                pipeline
                    .resolve_evaluation(reference)
                    .unwrap()
                    .canonical_basis(),
            ),
            _ => None,
        })
        .collect()
}
fn counts(p: &Pipeline) -> [usize; 8] {
    [
        p.observation_count(),
        p.measurement_count(),
        p.relation_count(),
        p.composition_count(),
        p.state_count(),
        p.transition_count(),
        p.evaluation_count(),
        p.invalid_count(),
    ]
}

#[test]
fn projection_all_six_types_is_deterministic_and_changes_no_calculus_objects() {
    let (mut pipeline, transition) = graph();
    let refs = pipeline.trace(&transition).unwrap();
    let targets = [
        (
            "observation",
            refs.iter()
                .find(|r| matches!(r, ObjectRef::Observation(_)))
                .unwrap()
                .clone(),
        ),
        (
            "measurement",
            refs.iter()
                .find(|r| matches!(r, ObjectRef::Measurement(_)))
                .unwrap()
                .clone(),
        ),
        (
            "relation",
            refs.iter()
                .find(|r| matches!(r, ObjectRef::Relation(_)))
                .unwrap()
                .clone(),
        ),
        (
            "composition",
            refs.iter()
                .find(|r| matches!(r, ObjectRef::Composition(_)))
                .unwrap()
                .clone(),
        ),
        (
            "state",
            refs.iter()
                .find(|r| matches!(r, ObjectRef::State(_)))
                .unwrap()
                .clone(),
        ),
        ("transition", transition),
    ];
    for (kind, target) in targets {
        let evaluation = evaluation(&mut pipeline, &format!("check/{kind}"), &target);
        let refs = pipeline.trace(&evaluation).unwrap();
        let before = capture(&pipeline, &refs);
        let before_counts = counts(&pipeline);
        let input = request(kind, &target, &evaluation);
        let mut representation = pipeline.project(input.clone()).unwrap();
        assert_eq!(representation, pipeline.project(input).unwrap());
        assert_eq!(representation.schema(), &id("TEST-REP-01"));
        assert_eq!(representation.source(), &target);
        assert_eq!(representation.evaluation(), &evaluation);
        assert_eq!(
            representation.source_basis(),
            pipeline
                .resolve_evaluation(&evaluation)
                .unwrap()
                .target_basis()
                .unwrap()
        );
        assert_eq!(
            representation.evaluation_basis(),
            pipeline
                .resolve_evaluation(&evaluation)
                .unwrap()
                .canonical_basis()
        );
        let original = representation.canonical_bytes();
        representation.label = "WAVE_3 BUY".into();
        assert_ne!(representation.canonical_bytes(), original);
        assert_eq!(
            pipeline.apply_representation(&representation),
            Err(ProjectionError::BackflowForbidden)
        );
        assert_eq!(capture(&pipeline, &refs), before);
        assert_eq!(counts(&pipeline), before_counts);
        assert_eq!(
            pipeline.resolve_evaluation(&evaluation).unwrap().status(),
            LifecycleStatus::Conformant
        );
    }
}

#[test]
fn unresolved_failed_and_invalid_evaluations_never_authorize_projection() {
    let (mut p, t) = graph();
    let target = p.resolve_transition(&t).unwrap().target_state().clone();
    let composition = p
        .resolve_state(&target)
        .unwrap()
        .source_composition()
        .clone();
    let pending = p
        .derive_state(StateRequest {
            contract: id("pending"),
            composition: composition.clone(),
            observation_horizon: Some(50),
        })
        .unwrap();
    let unknown = evaluation(&mut p, "check/state", &ObjectRef::State(0));
    let unresolved = evaluation(&mut p, "check/state", &pending);
    let failed = evaluation(&mut p, "fail", &composition);
    assert_eq!(
        p.resolve_evaluation(&unresolved).unwrap().result(),
        Some(ConformanceResult::Unresolved)
    );
    assert_eq!(
        p.resolve_evaluation(&failed).unwrap().result(),
        Some(ConformanceResult::Fail)
    );
    for (kind, target, e) in [
        ("state", target, unknown),
        ("state", pending, unresolved),
        ("composition", composition, failed),
    ] {
        let before = counts(&p);
        assert_eq!(
            p.project(request(kind, &target, &e)),
            Err(ProjectionError::ConformanceRequired)
        );
        assert_eq!(counts(&p), before);
    }
}

#[test]
fn missing_versions_types_and_unrelated_evaluations_are_rejected() {
    let (mut p, t) = graph();
    let e = evaluation(&mut p, "check/transition", &t);
    let s = p.resolve_transition(&t).unwrap().source_state().clone();
    let se = evaluation(&mut p, "check/state", &s);
    let original = request("transition", &t, &e);
    let mut missing = original.clone();
    missing.profile = VersionedId::new("project/transition", 2).unwrap();
    assert_eq!(p.project(missing), Err(ProjectionError::ProfileUnresolved));
    let mut missing = original.clone();
    missing.target = ObjectRef::Transition(0);
    assert_eq!(p.project(missing), Err(ProjectionError::TargetUnresolved));
    let mut wrong = original.clone();
    wrong.target = s.clone();
    assert_eq!(p.project(wrong), Err(ProjectionError::InvalidTargetType));
    let mut missing = original.clone();
    missing.evaluation = ObjectRef::Evaluation(0);
    assert_eq!(
        p.project(missing),
        Err(ProjectionError::EvaluationUnresolved)
    );
    let mut unrelated = original.clone();
    unrelated.evaluation = se;
    assert_eq!(
        p.project(unrelated),
        Err(ProjectionError::EvaluationMismatch)
    );
    let other = fixture::state(&mut p, 10, [11, 3]);
    let other_e = evaluation(&mut p, "check/state", &other);
    assert_eq!(
        p.project(request("state", &s, &other_e)),
        Err(ProjectionError::EvaluationMismatch)
    );
    let mut wrong = original;
    wrong.target = e;
    assert_eq!(p.project(wrong), Err(ProjectionError::InvalidTargetType));
}

#[test]
fn registry_requires_the_implemented_schema_and_matching_existing_contract() {
    let mut registry = registry();
    let original = registry.projection(&id("project/state")).unwrap().clone();
    assert!(matches!(
        registry.register_projection(original.clone()),
        Err(RegistryError::DuplicateReference(_))
    ));
    let mut bad = original.clone();
    bad.reference = id("bad");
    bad.schema = VersionedId::new("TEST-REP-01", 2).unwrap();
    assert_eq!(
        registry.register_projection(bad),
        Err(RegistryError::ProjectionContractMismatch)
    );
    let mut bad = original.clone();
    bad.reference = id("bad");
    bad.conformance = VersionedId::new("check/state", 2).unwrap();
    assert!(matches!(
        registry.register_projection(bad),
        Err(RegistryError::ProjectionConformanceUnresolved(_))
    ));
    let mut bad = original.clone();
    bad.reference = id("bad");
    bad.target_type = TargetType::Measurement;
    assert_eq!(
        registry.register_projection(bad),
        Err(RegistryError::ProjectionContractMismatch)
    );
    let mut bad = original.clone();
    bad.reference = id("bad");
    let c = bad.context.clone();
    bad.context = mdc_core::contract::dynamics::DynamicsContext::new(
        c.frame().clone(),
        id("wrong-calculus"),
        c.domain_version().clone(),
    );
    assert_eq!(
        registry.register_projection(bad),
        Err(RegistryError::ProjectionContractMismatch)
    );
    assert_eq!(registry.projection(&id("project/state")), Some(&original));
}
