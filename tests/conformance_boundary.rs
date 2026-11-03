#[allow(dead_code)]
#[path = "../examples/state_transition.rs"]
mod fixture;
use mdc_core::contract::conformance::*;
use mdc_core::contract::dynamics::{CriterionOrder, StateCriterionContract, StateCriterionKind};
use mdc_core::contract::evaluation::{ConformanceResult as R, LifecycleStatus as S};
use mdc_core::contract::registry::{Registry, RegistryError, VersionedId};
use mdc_core::identity::CanonicalSemanticBasis;
use mdc_core::model::evaluation::EvaluationDiagnostic as D;
use mdc_core::pipeline::*;
use mdc_core::provenance::ObjectRef;
fn id(s: &str) -> VersionedId {
    VersionedId::new(s, 1).unwrap()
}
fn contracts(reverse: bool, order: CriterionOrder) -> Registry {
    let mut r = fixture::registry();
    for (name, kind) in [
        ("exists", ConformanceCriterionKind::TargetExists),
        ("lineage", ConformanceCriterionKind::LineageComplete),
        ("identity", ConformanceCriterionKind::IdentityCoherent),
        ("construction", ConformanceCriterionKind::ConstructionPass),
        (
            "count",
            ConformanceCriterionKind::MemberCount { min: 3, max: 3 },
        ),
    ] {
        r.register_conformance_criterion(ConformanceCriterionContract {
            reference: id(name),
            kind,
        })
        .unwrap();
    }
    let context = r.state(&id("state")).unwrap().context.clone();
    for (name, target) in [
        ("O", TargetType::Observation),
        ("M", TargetType::Measurement),
        ("R", TargetType::Relation),
        ("C", TargetType::Composition),
        ("S", TargetType::State),
        ("T", TargetType::Transition),
    ] {
        let mut criteria = vec![
            id("exists"),
            id("lineage"),
            id("identity"),
            id("construction"),
        ];
        if reverse {
            criteria.reverse();
        }
        r.register_conformance(ConformanceContract {
            reference: id(name),
            target_type: target,
            criteria,
            criterion_order: order,
            context: context.clone(),
            missing_target: MissingTargetPolicy::Unresolved,
        })
        .unwrap();
    }
    let mut fail = r.conformance(&id("C")).unwrap().clone();
    fail.reference = id("fail-count");
    fail.criteria.push(id("count"));
    r.register_conformance(fail).unwrap();
    let mut missing = r.conformance(&id("S")).unwrap().clone();
    missing.reference = id("missing-fail");
    missing.missing_target = MissingTargetPolicy::Fail;
    r.register_conformance(missing).unwrap();
    r.register_state_criterion(StateCriterionContract {
        reference: id("after"),
        kind: StateCriterionKind::ObservationAfter {
            source: id("source"),
        },
    })
    .unwrap();
    let mut state = r.state(&id("state")).unwrap().clone();
    state.reference = id("pending");
    state.criteria.push(id("after"));
    r.register_state(state).unwrap();
    r
}
fn request(p: &Pipeline, name: &str, target: ObjectRef) -> EvaluationRequest {
    EvaluationRequest {
        contract: id(name),
        target,
        context: p.registry().conformance(&id(name)).unwrap().context.clone(),
    }
}
fn basis(p: &Pipeline, r: &ObjectRef) -> Vec<u8> {
    match r {
        ObjectRef::Observation(_) => p.resolve_observation(r).unwrap().canonical_basis(),
        ObjectRef::Measurement(_) => p.resolve_measurement(r).unwrap().canonical_basis(),
        ObjectRef::Relation(_) => p.resolve_relation(r).unwrap().canonical_basis(),
        ObjectRef::Composition(_) => p.resolve_composition(r).unwrap().canonical_basis(),
        ObjectRef::State(_) => p.resolve_state(r).unwrap().canonical_basis(),
        ObjectRef::Transition(_) => p.resolve_transition(r).unwrap().canonical_basis(),
        _ => Vec::new(),
    }
}
#[test]
fn evaluates_all_six_types_and_keeps_the_entire_target_graph_unchanged() {
    let mut p = Pipeline::new(contracts(false, CriterionOrder::Unordered));
    let a = fixture::state(&mut p, 10, [12, 6]);
    let b = fixture::state(&mut p, 30, [8, 4]);
    let t = p
        .transition(TransitionRequest {
            contract: id("transition"),
            source: a,
            target: b,
            observation_horizon: Some(50),
        })
        .unwrap();
    let graph = p.trace(&t).unwrap();
    let before: Vec<_> = graph.iter().map(|r| basis(&p, r)).collect();
    for (name, kind) in [("O", 0), ("M", 1), ("R", 2), ("C", 3), ("S", 4), ("T", 5)] {
        let target = graph
            .iter()
            .find(|r| {
                matches!(
                    (kind, r),
                    (0, ObjectRef::Observation(_))
                        | (1, ObjectRef::Measurement(_))
                        | (2, ObjectRef::Relation(_))
                        | (3, ObjectRef::Composition(_))
                        | (4, ObjectRef::State(_))
                        | (5, ObjectRef::Transition(_))
                )
            })
            .unwrap()
            .clone();
        let input = request(&p, name, target.clone());
        let e = p.evaluate(input.clone()).unwrap();
        let artifact = p.resolve_evaluation(&e).unwrap();
        assert_eq!(artifact.result(), Some(R::Pass));
        assert_eq!(artifact.status(), S::Conformant);
        assert_eq!(artifact.request(), &input);
        assert_eq!(artifact.target_basis(), Some(basis(&p, &target).as_slice()));
        assert!(p.trace(&e).unwrap().contains(&target));
        assert_eq!(p.evaluate(input).unwrap(), e);
    }
    assert_eq!(p.evaluation_count(), 6);
    assert_eq!(
        graph.iter().map(|r| basis(&p, r)).collect::<Vec<_>>(),
        before
    );
    assert_eq!(p.resolve_transition(&t).unwrap().status(), S::Candidate);
}
#[test]
fn real_fail_and_unresolved_are_retained_without_promoting_targets() {
    let mut p = Pipeline::new(contracts(false, CriterionOrder::Unordered));
    let state = fixture::state(&mut p, 30, [8, 4]);
    let c = p
        .resolve_state(&state)
        .unwrap()
        .source_composition()
        .clone();
    let pending = p
        .derive_state(StateRequest {
            contract: id("pending"),
            composition: c.clone(),
            observation_horizon: Some(50),
        })
        .unwrap();
    let u = p.evaluate(request(&p, "S", pending)).unwrap();
    assert_eq!(
        p.resolve_evaluation(&u).unwrap().result(),
        Some(R::Unresolved)
    );
    assert_eq!(p.resolve_evaluation(&u).unwrap().status(), S::Candidate);
    let e = p.evaluate(request(&p, "fail-count", c)).unwrap();
    let artifact = p.resolve_evaluation(&e).unwrap();
    assert_eq!(artifact.result(), Some(R::Fail));
    assert!(artifact.criteria().iter().any(|c| c.result() == R::Pass));
    assert!(artifact.criteria().iter().any(|c| c.result() == R::Fail));
    assert!(artifact.diagnostics().is_empty());
    assert_eq!(artifact.status(), S::Candidate);
    assert!(p.trace(&e).is_ok());
}
#[test]
fn missing_target_policies_and_unknown_contract_have_different_semantics() {
    let mut p = Pipeline::new(contracts(false, CriterionOrder::Unordered));
    for (name, expected) in [("S", R::Unresolved), ("missing-fail", R::Fail)] {
        let e = p.evaluate(request(&p, name, ObjectRef::State(0))).unwrap();
        let a = p.resolve_evaluation(&e).unwrap();
        assert_eq!(a.result(), Some(expected));
        assert_eq!(a.status(), S::Invalid);
        assert!(a.diagnostics().contains(&D::TargetUnresolved));
        assert!(p.trace(&e).is_err());
    }
    let mut input = request(&p, "S", ObjectRef::State(0));
    input.contract = VersionedId::new("S", 2).unwrap();
    let e = p.evaluate(input).unwrap();
    let a = p.resolve_evaluation(&e).unwrap();
    assert_eq!(a.result(), None);
    assert_eq!(a.diagnostics(), &[D::ContractUnresolved]);
    assert!(a.criteria().is_empty());
    assert_eq!(p.state_count(), 0);
    assert_eq!(p.evaluation_count(), 3);
}
#[test]
fn incompatible_target_and_context_produce_invalid_artifacts() {
    let mut isolated = Pipeline::new(contracts(false, CriterionOrder::Unordered));
    let invalid = isolated
        .evaluate(request(&isolated, "S", ObjectRef::Measurement(0)))
        .unwrap();
    assert_eq!(
        isolated.resolve_evaluation(&invalid).unwrap().result(),
        Some(R::Fail)
    );
    assert!(isolated
        .resolve_evaluation(&invalid)
        .unwrap()
        .diagnostics()
        .contains(&D::InvalidTargetType));

    let mut p = Pipeline::new(contracts(false, CriterionOrder::Unordered));
    let state = fixture::state(&mut p, 10, [12, 6]);
    let c = p
        .resolve_state(&state)
        .unwrap()
        .source_composition()
        .clone();
    let e = p.evaluate(request(&p, "S", c)).unwrap();
    assert!(p
        .resolve_evaluation(&e)
        .unwrap()
        .diagnostics()
        .contains(&D::InvalidTargetType));
    let mut input = request(&p, "S", state);
    let old = input.context.clone();
    input.context = mdc_core::contract::dynamics::DynamicsContext::new(
        old.frame().clone(),
        id("wrong-calculus"),
        old.domain_version().clone(),
    );
    let e = p.evaluate(input).unwrap();
    assert!(p
        .resolve_evaluation(&e)
        .unwrap()
        .diagnostics()
        .contains(&D::ContextMismatch));
    assert_eq!(p.resolve_evaluation(&e).unwrap().status(), S::Invalid);
}
#[test]
fn unordered_contract_permutation_is_stable_and_ordered_permutation_is_semantic() {
    fn run(reverse: bool, order: CriterionOrder) -> Vec<u8> {
        let mut p = Pipeline::new(contracts(reverse, order));
        let state = fixture::state(&mut p, 10, [12, 6]);
        let e = p.evaluate(request(&p, "S", state)).unwrap();
        p.resolve_evaluation(&e).unwrap().canonical_basis()
    }
    assert_eq!(
        run(false, CriterionOrder::Unordered),
        run(true, CriterionOrder::Unordered)
    );
    assert_ne!(
        run(false, CriterionOrder::Ordered),
        run(true, CriterionOrder::Ordered)
    );
}
#[test]
fn registry_rejects_incomplete_or_inapplicable_definitions_and_overwrites() {
    let mut r = contracts(false, CriterionOrder::Unordered);
    let original = r.conformance(&id("S")).unwrap().clone();
    assert!(matches!(
        r.register_conformance(original.clone()),
        Err(RegistryError::DuplicateReference(_))
    ));
    let mut bad = original.clone();
    bad.reference = id("bad");
    bad.criteria.clear();
    assert_eq!(
        r.register_conformance(bad),
        Err(RegistryError::EmptyConformanceCriteria)
    );
    let mut bad = original.clone();
    bad.reference = id("bad");
    bad.criteria.push(id("count"));
    assert_eq!(
        r.register_conformance(bad),
        Err(RegistryError::InvalidConformanceDefinition)
    );
    let mut bad = original.clone();
    bad.reference = id("bad");
    bad.criteria = vec![id("construction")];
    assert_eq!(
        r.register_conformance(bad),
        Err(RegistryError::InvalidConformanceDefinition)
    );
    let mut bad = original.clone();
    bad.reference = id("bad");
    bad.criteria.push(VersionedId::new("exists", 2).unwrap());
    assert!(matches!(
        r.register_conformance(bad),
        Err(RegistryError::ConformanceCriterionUnresolved(_))
    ));
    assert_eq!(r.conformance(&id("S")), Some(&original));
}
