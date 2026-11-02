//! Production-API coverage of the bounded State / Transition slice.
//! These checks do not replace the full MDC-E2E-CONFORMANCE-001 gate.

use mdc_core::canonical::snapshot::snapshot;
use mdc_core::contract::dynamics::{
    CriterionOrder, DynamicsContext, StateContract, StateCriterionContract, StateCriterionKind,
    StateDefinition, TransitionConditionContract, TransitionConditionKind, TransitionContract,
};
use mdc_core::contract::evaluation::{ConformanceResult, LifecycleStatus};
use mdc_core::contract::registry::{
    DomainContext, OperatorContract, OperatorKind, Registry, ScalarSchema, SchemaContract,
    SourceContract, VersionedId,
};
use mdc_core::contract::structure::{
    CompositionContract, ConstraintContract, ConstraintKind, MemberOrder, MemberType,
    RelationContext, RelationContract, RelationKind, TemporalScope,
};
use mdc_core::identity::{CanonicalSemanticBasis, IdentityAdapter};
use mdc_core::model::state::{State, StateError};
use mdc_core::model::transition::{Transition, TransitionError};
use mdc_core::pipeline::{
    CompositionRequest, MeasurementRequest, ObservationRequest, Pipeline, PipelineError,
    RejectedRequest, RelationRequest, StateRequest, TransitionRequest,
};
use mdc_core::provenance::ObjectRef;

type CapturedGraph = Vec<(ObjectRef, u64, Vec<u8>)>;

fn id(name: &str) -> VersionedId {
    VersionedId::new(name, 1).unwrap()
}

fn frame() -> RelationContext {
    RelationContext::new(
        DomainContext::new("fixture", id("ticks")).unwrap(),
        TemporalScope::new(0, 100).unwrap(),
        id("frame"),
        id("noncausal"),
    )
}

fn dynamics() -> DynamicsContext {
    DynamicsContext::new(frame(), id("calculus"), id("scalar-domain"))
}

fn registry() -> Registry {
    ordered_registry(false, false, CriterionOrder::Unordered)
}

fn ordered_registry(
    reverse_state: bool,
    reverse_conditions: bool,
    order: CriterionOrder,
) -> Registry {
    let mut registry = Registry::new();
    registry
        .register_schema(SchemaContract {
            reference: id("i64-le"),
            format: ScalarSchema::SignedI64Le,
        })
        .unwrap();
    for source in [
        id("source"),
        id("confirmation"),
        id("other-confirmation"),
        VersionedId::new("confirmation", 2).unwrap(),
    ] {
        registry
            .register_source(SourceContract {
                reference: source,
                schema: id("i64-le"),
                context: frame().domain().clone(),
                unit: "count".into(),
            })
            .unwrap();
    }
    registry
        .register_operator(OperatorContract {
            reference: id("read"),
            kind: OperatorKind::Scalar,
        })
        .unwrap();
    registry
        .register_relation(RelationContract {
            reference: id("ratio"),
            kind: RelationKind::Proportion,
            context: frame(),
        })
        .unwrap();
    for (name, min, max) in [("pair-count", 2, 2), ("point-count", 1, 1)] {
        registry
            .register_constraint(ConstraintContract {
                reference: id(name),
                kind: ConstraintKind::Cardinality { min, max },
            })
            .unwrap();
    }
    for (name, constraint) in [
        ("pair-composition", "pair-count"),
        ("point-composition", "point-count"),
    ] {
        registry
            .register_composition(CompositionContract {
                reference: id(name),
                member_order: MemberOrder::Unordered,
                member_type: MemberType::Measurements,
                constraints: vec![id(constraint)],
                context: frame(),
            })
            .unwrap();
    }
    for (name, kind) in [
        (
            "count-two",
            StateCriterionKind::Cardinality { min: 2, max: 2 },
        ),
        (
            "count-three",
            StateCriterionKind::Cardinality { min: 3, max: 3 },
        ),
        (
            "count-one",
            StateCriterionKind::Cardinality { min: 1, max: 1 },
        ),
        (
            "has-ratio",
            StateCriterionKind::HasRelation {
                relation: id("ratio"),
            },
        ),
        (
            "passed-count",
            StateCriterionKind::ConstraintPassed {
                constraint: id("pair-count"),
            },
        ),
        (
            "short-span",
            StateCriterionKind::MaxTemporalSpan { ticks: 20 },
        ),
        (
            "confirmation",
            StateCriterionKind::ObservationAfter {
                source: id("confirmation"),
            },
        ),
    ] {
        registry
            .register_state_criterion(StateCriterionContract {
                reference: id(name),
                kind,
            })
            .unwrap();
    }
    for (name, composition, mut criteria) in [
        (
            "local",
            "pair-composition",
            vec![
                id("count-two"),
                id("has-ratio"),
                id("passed-count"),
                id("short-span"),
            ],
        ),
        (
            "confirmed",
            "pair-composition",
            vec![id("count-two"), id("has-ratio"), id("confirmation")],
        ),
        (
            "failed",
            "pair-composition",
            vec![id("count-three"), id("confirmation")],
        ),
        ("point", "point-composition", vec![id("count-one")]),
    ] {
        if reverse_state {
            criteria.reverse();
        }
        registry
            .register_state(StateContract {
                reference: id(name),
                composition: id(composition),
                definition: StateDefinition::CompositionAbstraction,
                criterion_order: order,
                criteria,
                context: dynamics(),
            })
            .unwrap();
    }
    for (name, kind) in [
        (
            "temporal",
            TransitionConditionKind::TemporalOrder { strict: false },
        ),
        (
            "strict-temporal",
            TransitionConditionKind::TemporalOrder { strict: true },
        ),
        ("source-pass", TransitionConditionKind::SourceCriteriaPass),
        ("target-pass", TransitionConditionKind::TargetCriteriaPass),
        (
            "after-target",
            TransitionConditionKind::ObservationAfterTarget {
                source: id("confirmation"),
            },
        ),
    ] {
        registry
            .register_transition_condition(TransitionConditionContract {
                reference: id(name),
                kind,
            })
            .unwrap();
    }
    for (name, source, target, temporal, extra) in [
        ("move", "local", "local", "temporal", false),
        ("strict", "local", "local", "strict-temporal", false),
        (
            "confirmed-move",
            "confirmed",
            "confirmed",
            "temporal",
            false,
        ),
        ("after-move", "local", "local", "temporal", true),
        ("point-move", "point", "point", "temporal", false),
        ("mixed", "confirmed", "local", "temporal", false),
        ("reverse-mixed", "local", "confirmed", "temporal", false),
    ] {
        let mut conditions = vec![id(temporal), id("source-pass"), id("target-pass")];
        if extra {
            conditions.push(id("after-target"));
        }
        if reverse_conditions {
            conditions.reverse();
        }
        registry
            .register_transition(TransitionContract {
                reference: id(name),
                source_state: id(source),
                target_state: id(target),
                condition_order: order,
                conditions,
                context: dynamics(),
            })
            .unwrap();
    }
    registry
}

fn observe(pipeline: &mut Pipeline, source: VersionedId, timestamp: u64, value: i64) -> ObjectRef {
    pipeline
        .admit(ObservationRequest {
            source,
            timestamp: Some(timestamp),
            payload: value.to_le_bytes().to_vec(),
            acquisition_context: "fixture-record".into(),
        })
        .unwrap()
}

fn slice(
    pipeline: &mut Pipeline,
    start: u64,
    end: u64,
    offset: i64,
) -> (ObjectRef, Vec<ObjectRef>) {
    let mut graph = Vec::new();
    let mut measurements = Vec::new();
    for (timestamp, value) in [(start, 17 + offset), (end, 5 + offset)] {
        let observation = observe(pipeline, id("source"), timestamp, value);
        let measurement = pipeline
            .measure(MeasurementRequest {
                operator: id("read"),
                inputs: vec![observation.clone()],
            })
            .unwrap();
        graph.extend([observation, measurement.clone()]);
        measurements.push(measurement);
    }
    let ratio = pipeline
        .relate(RelationRequest {
            contract: id("ratio"),
            left: measurements[0].clone(),
            right: measurements[1].clone(),
        })
        .unwrap();
    let composition = pipeline
        .compose(CompositionRequest {
            contract: id("pair-composition"),
            members: measurements,
            relations: vec![ratio.clone()],
        })
        .unwrap();
    graph.extend([ratio, composition.clone()]);
    (composition, graph)
}

fn point(pipeline: &mut Pipeline, time: u64, value: i64) -> ObjectRef {
    let observation = observe(pipeline, id("source"), time, value);
    let measurement = pipeline
        .measure(MeasurementRequest {
            operator: id("read"),
            inputs: vec![observation],
        })
        .unwrap();
    pipeline
        .compose(CompositionRequest {
            contract: id("point-composition"),
            members: vec![measurement],
            relations: vec![],
        })
        .unwrap()
}

fn state_request(contract: &str, composition: &ObjectRef, horizon: Option<u64>) -> StateRequest {
    StateRequest {
        contract: id(contract),
        composition: composition.clone(),
        observation_horizon: horizon,
    }
}

fn transition_request(
    contract: &str,
    source: &ObjectRef,
    target: &ObjectRef,
    horizon: Option<u64>,
) -> TransitionRequest {
    TransitionRequest {
        contract: id(contract),
        source: source.clone(),
        target: target.clone(),
        observation_horizon: horizon,
    }
}

fn capture(pipeline: &Pipeline, references: &[ObjectRef]) -> CapturedGraph {
    references
        .iter()
        .map(|reference| {
            let captured = match reference {
                ObjectRef::Observation(_) => {
                    snapshot(pipeline.resolve_observation(reference).unwrap())
                }
                ObjectRef::Measurement(_) => {
                    snapshot(pipeline.resolve_measurement(reference).unwrap())
                }
                ObjectRef::Relation(_) => snapshot(pipeline.resolve_relation(reference).unwrap()),
                ObjectRef::Composition(_) => {
                    snapshot(pipeline.resolve_composition(reference).unwrap())
                }
                ObjectRef::State(_) => snapshot(pipeline.resolve_state(reference).unwrap()),
                ObjectRef::Transition(_) => {
                    snapshot(pipeline.resolve_transition(reference).unwrap())
                }
                ObjectRef::Source(_)
                | ObjectRef::Evaluation(_)
                | ObjectRef::InvalidObservation(_)
                | ObjectRef::InvalidMeasurement(_)
                | ObjectRef::InvalidRelation(_)
                | ObjectRef::InvalidComposition(_)
                | ObjectRef::InvalidState(_)
                | ObjectRef::InvalidTransition(_) => {
                    panic!("capture expects admitted semantic objects")
                }
            };
            (
                reference.clone(),
                captured.identity,
                captured.semantic_basis,
            )
        })
        .collect()
}

fn counts(pipeline: &Pipeline) -> [usize; 6] {
    [
        pipeline.observation_count(),
        pipeline.measurement_count(),
        pipeline.relation_count(),
        pipeline.composition_count(),
        pipeline.state_count(),
        pipeline.transition_count(),
    ]
}

fn reject(
    pipeline: &mut Pipeline,
    request: RejectedRequest,
    upstream: &[ObjectRef],
) -> PipelineError {
    let before = capture(pipeline, upstream);
    let count = counts(pipeline);
    let rejected = pipeline.rejections().len();
    let result = match request.clone() {
        RejectedRequest::Observation(request) => pipeline.admit(request),
        RejectedRequest::Measurement(request) => pipeline.measure(request),
        RejectedRequest::Relation(request) => pipeline.relate(request),
        RejectedRequest::Composition(request) => pipeline.compose(request),
        RejectedRequest::State(request) => pipeline.derive_state(request),
        RejectedRequest::Transition(request) => pipeline.transition(request),
    };
    let error = result.unwrap_err();
    assert_eq!(counts(pipeline), count);
    assert_eq!(capture(pipeline, upstream), before);
    assert_eq!(pipeline.rejections().len(), rejected + 1);
    let record = pipeline.rejections().last().unwrap();
    assert_eq!(record.request(), &request);
    assert_eq!(record.error(), &error);
    assert_eq!(record.status(), LifecycleStatus::Invalid);
    error
}

#[test]
fn real_state_transition_chain_reaches_sources_and_preserves_upstream_content() {
    let mut pipeline = Pipeline::new(registry());
    let (a, mut graph) = slice(&mut pipeline, 10, 20, 0);
    let (b, next_graph) = slice(&mut pipeline, 30, 40, 1);
    graph.extend(next_graph);
    let before = capture(&pipeline, &graph);
    let source = pipeline
        .derive_state(state_request("local", &a, Some(60)))
        .unwrap();
    let target = pipeline
        .derive_state(state_request("local", &b, Some(60)))
        .unwrap();
    assert_eq!(capture(&pipeline, &graph), before);
    assert_eq!(pipeline.transition_count(), 0);
    for (reference, composition, start, end) in [(&source, &a, 10, 20), (&target, &b, 30, 40)] {
        let state = pipeline.resolve_state(reference).unwrap();
        assert_eq!(state.source_composition(), composition);
        assert_eq!(state.criteria_result(), ConformanceResult::Pass);
        assert!(state
            .criteria()
            .iter()
            .all(|criterion| criterion.result() == ConformanceResult::Pass));
        assert_eq!(
            (
                state.validity_interval().start(),
                state.validity_interval().end()
            ),
            (start, end)
        );
        assert_eq!(state.context().calculus_version(), &id("calculus"));
        assert_eq!(state.context().domain_version(), &id("scalar-domain"));
        assert_eq!(state.status(), LifecycleStatus::Candidate);
        assert_eq!(
            state.id(),
            IdentityAdapter::derive_identity_from_basis(&state.canonical_basis())
        );
    }
    graph.extend([source.clone(), target.clone()]);
    let before = capture(&pipeline, &graph);
    let transition = pipeline
        .transition(transition_request("move", &source, &target, Some(60)))
        .unwrap();
    assert_eq!(capture(&pipeline, &graph), before);
    let object = pipeline.resolve_transition(&transition).unwrap();
    assert_eq!(object.source_state(), &source);
    assert_eq!(object.target_state(), &target);
    assert_eq!(object.conditions_result(), ConformanceResult::Pass);
    assert_eq!(object.status(), LifecycleStatus::Candidate);
    assert_eq!(&object.provenance().inputs()[..2], &[source, target]);
    let mut expected = graph;
    expected.extend([transition.clone(), ObjectRef::Source(id("source"))]);
    expected.sort();
    assert_eq!(pipeline.trace(&transition).unwrap(), expected);
}

#[test]
fn missing_confirmation_stays_unresolved_without_promoting_candidate() {
    let mut pipeline = Pipeline::new(registry());
    let (composition, _) = slice(&mut pipeline, 10, 20, 0);
    let state = pipeline
        .derive_state(state_request("confirmed", &composition, Some(60)))
        .unwrap();
    let object = pipeline.resolve_state(&state).unwrap();
    let criterion = object
        .criteria()
        .iter()
        .find(|criterion| criterion.reference() == &id("confirmation"))
        .unwrap();
    assert_eq!(criterion.result(), ConformanceResult::Unresolved);
    assert!(criterion.evidence().is_empty());
    assert_eq!(object.criteria_result(), ConformanceResult::Unresolved);
    assert_eq!(object.status(), LifecycleStatus::Candidate);
}

#[test]
fn future_or_wrong_source_witnesses_do_not_cross_the_requested_horizon() {
    let mut pipeline = Pipeline::new(registry());
    let (composition, _) = slice(&mut pipeline, 10, 20, 0);
    let future = observe(&mut pipeline, id("confirmation"), 80, 9);
    let wrong = observe(&mut pipeline, id("other-confirmation"), 30, 9);
    let other_version = observe(
        &mut pipeline,
        VersionedId::new("confirmation", 2).unwrap(),
        30,
        9,
    );
    for horizon in [None, Some(60)] {
        let state = pipeline
            .derive_state(state_request("confirmed", &composition, horizon))
            .unwrap();
        assert_eq!(
            pipeline.resolve_state(&state).unwrap().criteria_result(),
            ConformanceResult::Unresolved
        );
        let trace = pipeline.trace(&state).unwrap();
        assert!(!trace.contains(&future));
        assert!(!trace.contains(&wrong));
        assert!(!trace.contains(&other_version));
    }
    let accepted = pipeline
        .derive_state(state_request("confirmed", &composition, Some(80)))
        .unwrap();
    assert_eq!(
        pipeline.resolve_state(&accepted).unwrap().criteria_result(),
        ConformanceResult::Pass
    );
    assert!(pipeline.trace(&accepted).unwrap().contains(&future));
}

#[test]
fn late_evidence_creates_a_new_state_and_preserves_the_old_evidence_snapshot() {
    let mut pipeline = Pipeline::new(registry());
    let (composition, _) = slice(&mut pipeline, 10, 20, 0);
    let request = state_request("confirmed", &composition, Some(60));
    let old = pipeline.derive_state(request.clone()).unwrap();
    let before = capture(&pipeline, std::slice::from_ref(&old));
    let witness = observe(&mut pipeline, id("confirmation"), 50, 9);
    let new = pipeline.derive_state(request.clone()).unwrap();
    assert_ne!(old, new);
    assert_eq!(capture(&pipeline, std::slice::from_ref(&old)), before);
    assert_eq!(
        pipeline.resolve_state(&old).unwrap().criteria_result(),
        ConformanceResult::Unresolved
    );
    let updated = pipeline.resolve_state(&new).unwrap();
    assert_eq!(updated.criteria_result(), ConformanceResult::Pass);
    assert_eq!(
        (
            updated.validity_interval().start(),
            updated.validity_interval().end()
        ),
        (10, 20)
    );
    assert!(updated.provenance().inputs().contains(&witness));
    assert_eq!(pipeline.derive_state(request).unwrap(), new);
    assert_eq!(pipeline.state_count(), 2);
}

#[test]
fn an_absent_horizon_does_not_use_an_already_stored_confirmation() {
    let mut pipeline = Pipeline::new(registry());
    let (composition, _) = slice(&mut pipeline, 10, 20, 0);
    let witness = observe(&mut pipeline, id("confirmation"), 30, 9);
    let unknown = pipeline
        .derive_state(state_request("confirmed", &composition, None))
        .unwrap();
    assert_eq!(
        pipeline.resolve_state(&unknown).unwrap().criteria_result(),
        ConformanceResult::Unresolved
    );
    assert!(!pipeline.trace(&unknown).unwrap().contains(&witness));
    let known = pipeline
        .derive_state(state_request("confirmed", &composition, Some(30)))
        .unwrap();
    assert_eq!(
        pipeline.resolve_state(&known).unwrap().criteria_result(),
        ConformanceResult::Pass
    );
}

#[test]
fn failed_state_criteria_retain_fail_and_unresolved_without_admitting_a_state() {
    let mut pipeline = Pipeline::new(registry());
    let (composition, graph) = slice(&mut pipeline, 10, 20, 0);
    let error = reject(
        &mut pipeline,
        RejectedRequest::State(state_request("failed", &composition, Some(60))),
        &graph,
    );
    let PipelineError::State(StateError::FailedCriteria(results)) = error else {
        panic!("expected evaluated criterion failure")
    };
    assert_eq!(results.len(), 2);
    assert!(results
        .iter()
        .any(|result| result.reference() == &id("count-three")
            && result.result() == ConformanceResult::Fail));
    assert!(results
        .iter()
        .any(|result| result.reference() == &id("confirmation")
            && result.result() == ConformanceResult::Unresolved));
    assert_eq!(pipeline.state_count(), 0);
}

#[test]
fn temporal_span_criterion_uses_actual_source_interval_not_declared_scope() {
    let mut pipeline = Pipeline::new(registry());
    let (short, _) = slice(&mut pipeline, 10, 20, 0);
    pipeline
        .derive_state(state_request("local", &short, Some(60)))
        .unwrap();
    let (long, graph) = slice(&mut pipeline, 10, 50, 1);
    let error = reject(
        &mut pipeline,
        RejectedRequest::State(state_request("local", &long, Some(60))),
        &graph,
    );
    let PipelineError::State(StateError::FailedCriteria(results)) = error else {
        panic!("expected span failure")
    };
    assert_eq!(
        results
            .iter()
            .filter(|result| result.result() == ConformanceResult::Fail)
            .count(),
        1
    );
    assert!(results
        .iter()
        .any(|result| result.reference() == &id("short-span")
            && result.result() == ConformanceResult::Fail));
}

#[test]
fn state_horizon_and_contract_type_checks_reject_without_changing_inputs() {
    let mut pipeline = Pipeline::new(registry());
    let (composition, graph) = slice(&mut pipeline, 10, 20, 0);
    for (horizon, expected) in [
        (Some(19), StateError::HorizonBeforeInput),
        (Some(101), StateError::HorizonOutsideScope),
    ] {
        assert_eq!(
            reject(
                &mut pipeline,
                RejectedRequest::State(state_request("local", &composition, horizon)),
                &graph
            ),
            PipelineError::State(expected)
        );
    }
    let point = point(&mut pipeline, 10, 7);
    assert_eq!(
        reject(
            &mut pipeline,
            RejectedRequest::State(state_request("local", &point, Some(60))),
            &graph
        ),
        PipelineError::State(StateError::CompositionContractMismatch)
    );
}

#[test]
fn unresolved_state_results_propagate_to_transition_conditions() {
    let mut pipeline = Pipeline::new(registry());
    let (a, _) = slice(&mut pipeline, 10, 20, 0);
    let (b, _) = slice(&mut pipeline, 30, 40, 1);
    let source = pipeline
        .derive_state(state_request("confirmed", &a, Some(60)))
        .unwrap();
    let target = pipeline
        .derive_state(state_request("confirmed", &b, Some(60)))
        .unwrap();
    let result = pipeline
        .transition(transition_request(
            "confirmed-move",
            &source,
            &target,
            Some(60),
        ))
        .unwrap();
    let object = pipeline.resolve_transition(&result).unwrap();
    assert_eq!(object.conditions_result(), ConformanceResult::Unresolved);
    assert_eq!(object.status(), LifecycleStatus::Candidate);
    for name in ["source-pass", "target-pass"] {
        assert_eq!(
            object
                .conditions()
                .iter()
                .find(|condition| condition.reference() == &id(name))
                .unwrap()
                .result(),
            ConformanceResult::Unresolved
        );
    }
}

#[test]
fn source_and_target_criterion_results_are_propagated_independently() {
    let mut pipeline = Pipeline::new(registry());
    let (a, _) = slice(&mut pipeline, 10, 20, 0);
    let (b, _) = slice(&mut pipeline, 30, 40, 1);
    let early_known = pipeline
        .derive_state(state_request("local", &a, Some(60)))
        .unwrap();
    let early_unknown = pipeline
        .derive_state(state_request("confirmed", &a, Some(60)))
        .unwrap();
    let late_known = pipeline
        .derive_state(state_request("local", &b, Some(60)))
        .unwrap();
    let late_unknown = pipeline
        .derive_state(state_request("confirmed", &b, Some(60)))
        .unwrap();
    for (contract, source, target, expected_source, expected_target) in [
        (
            "mixed",
            early_unknown,
            late_known,
            ConformanceResult::Unresolved,
            ConformanceResult::Pass,
        ),
        (
            "reverse-mixed",
            early_known,
            late_unknown,
            ConformanceResult::Pass,
            ConformanceResult::Unresolved,
        ),
    ] {
        let reference = pipeline
            .transition(transition_request(contract, &source, &target, Some(60)))
            .unwrap();
        let transition = pipeline.resolve_transition(&reference).unwrap();
        assert_eq!(
            transition.conditions_result(),
            ConformanceResult::Unresolved
        );
        for (name, expected) in [
            ("source-pass", expected_source),
            ("target-pass", expected_target),
        ] {
            assert_eq!(
                transition
                    .conditions()
                    .iter()
                    .find(|condition| condition.reference() == &id(name))
                    .unwrap()
                    .result(),
                expected
            );
        }
    }
}

#[test]
fn a_failed_temporal_condition_takes_precedence_over_unresolved_states() {
    let mut pipeline = Pipeline::new(registry());
    let (a, _) = slice(&mut pipeline, 10, 20, 0);
    let (b, _) = slice(&mut pipeline, 30, 40, 1);
    let early = pipeline
        .derive_state(state_request("confirmed", &a, Some(60)))
        .unwrap();
    let late = pipeline
        .derive_state(state_request("confirmed", &b, Some(60)))
        .unwrap();
    let error = reject(
        &mut pipeline,
        RejectedRequest::Transition(transition_request(
            "confirmed-move",
            &late,
            &early,
            Some(60),
        )),
        &[early, late],
    );
    let PipelineError::Transition(TransitionError::FailedConditions(results)) = error else {
        panic!("expected computed conditions")
    };
    assert_eq!(results.len(), 3);
    assert_eq!(
        results
            .iter()
            .filter(|condition| condition.result() == ConformanceResult::Unresolved)
            .count(),
        2
    );
    assert!(results
        .iter()
        .any(|condition| condition.reference() == &id("temporal")
            && condition.result() == ConformanceResult::Fail));
    assert_eq!(pipeline.transition_count(), 0);
}

#[test]
fn transition_horizon_covers_state_witnesses_without_expanding_validity_intervals() {
    let mut pipeline = Pipeline::new(registry());
    let (a, _) = slice(&mut pipeline, 10, 20, 0);
    let (b, _) = slice(&mut pipeline, 30, 40, 1);
    let witness = observe(&mut pipeline, id("confirmation"), 50, 9);
    let source = pipeline
        .derive_state(state_request("confirmed", &a, Some(60)))
        .unwrap();
    let target = pipeline
        .derive_state(state_request("confirmed", &b, Some(60)))
        .unwrap();
    assert_eq!(
        pipeline
            .resolve_state(&source)
            .unwrap()
            .validity_interval()
            .end(),
        20
    );
    assert_eq!(
        pipeline
            .resolve_state(&target)
            .unwrap()
            .validity_interval()
            .end(),
        40
    );
    assert_eq!(
        reject(
            &mut pipeline,
            RejectedRequest::Transition(transition_request(
                "confirmed-move",
                &source,
                &target,
                Some(49)
            )),
            &[source.clone(), target.clone(), witness.clone()]
        ),
        PipelineError::Transition(TransitionError::HorizonBeforeInput)
    );
    let result = pipeline
        .transition(transition_request(
            "confirmed-move",
            &source,
            &target,
            Some(50),
        ))
        .unwrap();
    assert_eq!(
        pipeline
            .resolve_transition(&result)
            .unwrap()
            .conditions_result(),
        ConformanceResult::Pass
    );
    let trace = pipeline.trace(&result).unwrap();
    assert!(trace.contains(&witness));
    assert!(trace.contains(&ObjectRef::Source(id("confirmation"))));
}

#[test]
fn transition_confirmation_requires_real_in_horizon_evidence() {
    let mut pipeline = Pipeline::new(registry());
    let (a, _) = slice(&mut pipeline, 10, 20, 0);
    let (b, _) = slice(&mut pipeline, 30, 40, 1);
    let source = pipeline
        .derive_state(state_request("local", &a, Some(60)))
        .unwrap();
    let target = pipeline
        .derive_state(state_request("local", &b, Some(60)))
        .unwrap();
    let witness = observe(&mut pipeline, id("confirmation"), 80, 9);
    for horizon in [None, Some(60)] {
        let result = pipeline
            .transition(transition_request("after-move", &source, &target, horizon))
            .unwrap();
        assert_eq!(
            pipeline
                .resolve_transition(&result)
                .unwrap()
                .conditions_result(),
            ConformanceResult::Unresolved
        );
        assert!(!pipeline.trace(&result).unwrap().contains(&witness));
    }
    let passed = pipeline
        .transition(transition_request("after-move", &source, &target, Some(80)))
        .unwrap();
    let object = pipeline.resolve_transition(&passed).unwrap();
    assert_eq!(object.conditions_result(), ConformanceResult::Pass);
    assert!(object.provenance().inputs().contains(&witness));
}

#[test]
fn missing_references_versions_and_wrong_input_types_never_synthesize_targets() {
    let mut pipeline = Pipeline::new(registry());
    let (composition, mut graph) = slice(&mut pipeline, 10, 20, 0);
    let source = pipeline
        .derive_state(state_request("local", &composition, Some(60)))
        .unwrap();
    graph.push(source.clone());
    let mut absent_contract = state_request("local", &composition, Some(60));
    absent_contract.contract = VersionedId::new("local", 2).unwrap();
    assert_eq!(
        reject(
            &mut pipeline,
            RejectedRequest::State(absent_contract.clone()),
            &graph
        ),
        PipelineError::StateContractUnresolved(absent_contract.contract)
    );
    let missing_comp = ObjectRef::Composition(0);
    assert!(pipeline.resolve_composition(&missing_comp).is_none());
    assert_eq!(
        reject(
            &mut pipeline,
            RejectedRequest::State(state_request("local", &missing_comp, Some(60))),
            &graph
        ),
        PipelineError::InputUnresolved(missing_comp)
    );
    let missing_target = ObjectRef::State(0);
    assert!(pipeline.resolve_state(&missing_target).is_none());
    assert_eq!(
        reject(
            &mut pipeline,
            RejectedRequest::Transition(transition_request(
                "move",
                &source,
                &missing_target,
                Some(60)
            )),
            &graph
        ),
        PipelineError::InputUnresolved(missing_target.clone())
    );
    assert!(pipeline.resolve_state(&missing_target).is_none());
    assert_eq!(
        reject(
            &mut pipeline,
            RejectedRequest::Transition(transition_request(
                "move",
                &source,
                &composition,
                Some(60)
            )),
            &graph
        ),
        PipelineError::InvalidInputType(composition.clone())
    );
    let mut absent_transition = transition_request("move", &source, &source, Some(60));
    absent_transition.contract = VersionedId::new("move", 2).unwrap();
    assert_eq!(
        reject(
            &mut pipeline,
            RejectedRequest::Transition(absent_transition.clone()),
            &graph
        ),
        PipelineError::TransitionContractUnresolved(absent_transition.contract)
    );
}

#[test]
fn transition_contracts_and_self_reference_are_checked_before_admission() {
    let mut pipeline = Pipeline::new(registry());
    let (a, _) = slice(&mut pipeline, 10, 20, 0);
    let (b, _) = slice(&mut pipeline, 30, 40, 1);
    let source = pipeline
        .derive_state(state_request("local", &a, Some(60)))
        .unwrap();
    let target = pipeline
        .derive_state(state_request("local", &b, Some(60)))
        .unwrap();
    let confirmed = pipeline
        .derive_state(state_request("confirmed", &b, Some(60)))
        .unwrap();
    let upstream = [source.clone(), target.clone(), confirmed.clone()];
    assert_eq!(
        reject(
            &mut pipeline,
            RejectedRequest::Transition(transition_request("move", &source, &source, Some(60))),
            &upstream
        ),
        PipelineError::Transition(TransitionError::SameState)
    );
    assert_eq!(
        reject(
            &mut pipeline,
            RejectedRequest::Transition(transition_request("move", &confirmed, &target, Some(60))),
            &upstream
        ),
        PipelineError::Transition(TransitionError::SourceContractMismatch)
    );
    assert_eq!(
        reject(
            &mut pipeline,
            RejectedRequest::Transition(transition_request("move", &source, &confirmed, Some(60))),
            &upstream
        ),
        PipelineError::Transition(TransitionError::TargetContractMismatch)
    );
    assert_eq!(
        reject(
            &mut pipeline,
            RejectedRequest::Transition(transition_request("move", &source, &target, Some(101))),
            &upstream
        ),
        PipelineError::Transition(TransitionError::HorizonOutsideScope)
    );
}

#[test]
fn overlap_reverse_and_strict_temporal_order_are_computed_not_assumed() {
    let mut pipeline = Pipeline::new(registry());
    let (a, _) = slice(&mut pipeline, 10, 20, 0);
    let (b, _) = slice(&mut pipeline, 15, 25, 1);
    let (c, _) = slice(&mut pipeline, 20, 30, 2);
    let source = pipeline
        .derive_state(state_request("local", &a, Some(60)))
        .unwrap();
    let overlapping = pipeline
        .derive_state(state_request("local", &b, Some(60)))
        .unwrap();
    let touching = pipeline
        .derive_state(state_request("local", &c, Some(60)))
        .unwrap();
    for (contract, left, right) in [
        ("move", &source, &overlapping),
        ("move", &touching, &source),
        ("strict", &source, &touching),
    ] {
        let error = reject(
            &mut pipeline,
            RejectedRequest::Transition(transition_request(contract, left, right, Some(60))),
            &[source.clone(), overlapping.clone(), touching.clone()],
        );
        assert!(matches!(
            error,
            PipelineError::Transition(TransitionError::FailedConditions(_))
        ));
    }
    let accepted = pipeline
        .transition(transition_request("move", &source, &touching, Some(60)))
        .unwrap();
    assert_eq!(
        pipeline
            .resolve_transition(&accepted)
            .unwrap()
            .conditions_result(),
        ConformanceResult::Pass
    );
}

#[test]
fn source_target_order_remains_semantic_when_both_directions_are_admissible() {
    let mut pipeline = Pipeline::new(registry());
    let a = point(&mut pipeline, 10, 17);
    let b = point(&mut pipeline, 10, 5);
    let a = pipeline
        .derive_state(state_request("point", &a, Some(60)))
        .unwrap();
    let b = pipeline
        .derive_state(state_request("point", &b, Some(60)))
        .unwrap();
    let forward_request = transition_request("point-move", &a, &b, Some(60));
    let forward = pipeline.transition(forward_request.clone()).unwrap();
    let reverse = pipeline
        .transition(transition_request("point-move", &b, &a, Some(60)))
        .unwrap();
    assert_ne!(forward, reverse);
    let first = pipeline.resolve_transition(&forward).unwrap();
    let second = pipeline.resolve_transition(&reverse).unwrap();
    assert_ne!(first.canonical_basis(), second.canonical_basis());
    assert_eq!(&first.provenance().inputs()[..2], &[a.clone(), b.clone()]);
    assert_eq!(&second.provenance().inputs()[..2], &[b, a]);
    assert_eq!(pipeline.transition(forward_request).unwrap(), forward);
    assert_eq!(pipeline.transition_count(), 2);
}

#[derive(Debug, Eq, PartialEq)]
struct RunGraph {
    states: Vec<Vec<u8>>,
    transition: Vec<u8>,
    graph: CapturedGraph,
}

fn run_graph(
    reverse_creation: bool,
    reverse_state: bool,
    reverse_conditions: bool,
    order: CriterionOrder,
) -> RunGraph {
    let mut pipeline = Pipeline::new(ordered_registry(reverse_state, reverse_conditions, order));
    let ((a, graph_a), (b, graph_b)) = if reverse_creation {
        let b = slice(&mut pipeline, 30, 40, 1);
        let a = slice(&mut pipeline, 10, 20, 0);
        (a, b)
    } else {
        let a = slice(&mut pipeline, 10, 20, 0);
        let b = slice(&mut pipeline, 30, 40, 1);
        (a, b)
    };
    let witness_one;
    let witness_two;
    if reverse_creation {
        witness_two = observe(&mut pipeline, id("confirmation"), 55, 2);
        witness_one = observe(&mut pipeline, id("confirmation"), 50, 1);
    } else {
        witness_one = observe(&mut pipeline, id("confirmation"), 50, 1);
        witness_two = observe(&mut pipeline, id("confirmation"), 55, 2);
    }
    let source = pipeline
        .derive_state(state_request("confirmed", &a, Some(60)))
        .unwrap();
    let target = pipeline
        .derive_state(state_request("confirmed", &b, Some(60)))
        .unwrap();
    let transition = pipeline
        .transition(transition_request(
            "confirmed-move",
            &source,
            &target,
            Some(60),
        ))
        .unwrap();
    let states = [&source, &target]
        .iter()
        .map(|reference| pipeline.resolve_state(reference).unwrap().canonical_basis())
        .collect();
    let transition_basis = pipeline
        .resolve_transition(&transition)
        .unwrap()
        .canonical_basis();
    let mut graph = graph_a;
    graph.extend(graph_b);
    graph.extend([witness_one, witness_two, source, target, transition]);
    graph.sort();
    RunGraph {
        states,
        transition: transition_basis,
        graph: capture(&pipeline, &graph),
    }
}

#[test]
fn equivalent_graphs_are_deterministic_under_creation_and_unordered_contract_permutations() {
    let original = run_graph(false, false, false, CriterionOrder::Unordered);
    assert_eq!(
        original,
        run_graph(false, false, false, CriterionOrder::Unordered)
    );
    assert_eq!(
        original,
        run_graph(true, true, true, CriterionOrder::Unordered)
    );
}

#[test]
fn ordered_criteria_and_conditions_preserve_declared_semantic_order() {
    let original = run_graph(false, false, false, CriterionOrder::Ordered);
    let reordered_criteria = run_graph(false, true, false, CriterionOrder::Ordered);
    assert_ne!(original.states, reordered_criteria.states);
    let reordered_conditions = run_graph(false, false, true, CriterionOrder::Ordered);
    assert_eq!(original.states, reordered_conditions.states);
    assert_ne!(original.transition, reordered_conditions.transition);
}

#[test]
fn raw_legacy_state_and_transition_values_do_not_enter_the_admitted_graph() {
    let mut pipeline = Pipeline::new(registry());
    let raw_state = State {
        id: 42,
        source_composition: 11,
        criteria_met: true,
        status: LifecycleStatus::Candidate,
    };
    let raw_transition = Transition {
        id: 43,
        source_state: raw_state.id,
        target_state: 99,
        condition_met: true,
        status: LifecycleStatus::Candidate,
    };
    let reference = ObjectRef::State(raw_state.id);
    assert!(pipeline.resolve_state(&reference).is_none());
    assert!(pipeline
        .resolve_transition(&ObjectRef::Transition(raw_transition.id))
        .is_none());
    assert_eq!(
        reject(
            &mut pipeline,
            RejectedRequest::Transition(transition_request(
                "move",
                &reference,
                &ObjectRef::State(99),
                Some(60)
            )),
            &[]
        ),
        PipelineError::InputUnresolved(reference)
    );
    assert_eq!(pipeline.state_count(), 0);
    assert_eq!(pipeline.transition_count(), 0);
}
