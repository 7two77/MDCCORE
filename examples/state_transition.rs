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
use mdc_core::pipeline::{
    CompositionRequest, MeasurementRequest, ObservationRequest, Pipeline, RelationRequest,
    StateRequest, TransitionRequest,
};
use mdc_core::provenance::ObjectRef;

fn reference(id: &str) -> VersionedId {
    VersionedId::new(id, 1).unwrap()
}

pub fn registry() -> Registry {
    let domain = DomainContext::new("fixture/counter", reference("fixture/ticks")).unwrap();
    let frame = RelationContext::new(
        domain.clone(),
        TemporalScope::new(0, 100).unwrap(),
        reference("fixture/frame"),
        reference("fixture/noncausal"),
    );
    let dynamics = DynamicsContext::new(
        frame.clone(),
        reference("STP-MDC-0.1"),
        reference("MDC-SCALAR-001"),
    );
    let mut registry = Registry::new();
    registry
        .register_schema(SchemaContract {
            reference: reference("schema"),
            format: ScalarSchema::SignedI64Le,
        })
        .unwrap();
    registry
        .register_source(SourceContract {
            reference: reference("source"),
            schema: reference("schema"),
            context: domain,
            unit: "count".into(),
        })
        .unwrap();
    registry
        .register_operator(OperatorContract {
            reference: reference("read"),
            kind: OperatorKind::Scalar,
        })
        .unwrap();
    registry
        .register_relation(RelationContract {
            reference: reference("ratio"),
            kind: RelationKind::Proportion,
            context: frame.clone(),
        })
        .unwrap();
    let mut constraints = Vec::new();
    for (name, kind) in [
        (
            "cardinality",
            ConstraintKind::Cardinality { min: 2, max: 2 },
        ),
        ("connected", ConstraintKind::Connected),
    ] {
        registry
            .register_constraint(ConstraintContract {
                reference: reference(name),
                kind,
            })
            .unwrap();
        constraints.push(reference(name));
    }
    registry
        .register_composition(CompositionContract {
            reference: reference("composition"),
            member_order: MemberOrder::Unordered,
            member_type: MemberType::Measurements,
            constraints,
            context: frame,
        })
        .unwrap();
    let mut criteria = Vec::new();
    for (name, kind) in [
        (
            "state/cardinality",
            StateCriterionKind::Cardinality { min: 2, max: 2 },
        ),
        (
            "state/relation",
            StateCriterionKind::HasRelation {
                relation: reference("ratio"),
            },
        ),
        (
            "state/connected",
            StateCriterionKind::ConstraintPassed {
                constraint: reference("connected"),
            },
        ),
        (
            "state/span",
            StateCriterionKind::MaxTemporalSpan { ticks: 20 },
        ),
    ] {
        registry
            .register_state_criterion(StateCriterionContract {
                reference: reference(name),
                kind,
            })
            .unwrap();
        criteria.push(reference(name));
    }
    registry
        .register_state(StateContract {
            reference: reference("state"),
            composition: reference("composition"),
            definition: StateDefinition::CompositionAbstraction,
            criterion_order: CriterionOrder::Unordered,
            criteria,
            context: dynamics.clone(),
        })
        .unwrap();
    let mut conditions = Vec::new();
    for (name, kind) in [
        (
            "transition/time",
            TransitionConditionKind::TemporalOrder { strict: true },
        ),
        (
            "transition/source",
            TransitionConditionKind::SourceCriteriaPass,
        ),
        (
            "transition/target",
            TransitionConditionKind::TargetCriteriaPass,
        ),
    ] {
        registry
            .register_transition_condition(TransitionConditionContract {
                reference: reference(name),
                kind,
            })
            .unwrap();
        conditions.push(reference(name));
    }
    registry
        .register_transition(TransitionContract {
            reference: reference("transition"),
            source_state: reference("state"),
            target_state: reference("state"),
            condition_order: CriterionOrder::Unordered,
            conditions,
            context: dynamics,
        })
        .unwrap();
    registry
}

pub fn state(pipeline: &mut Pipeline, first: u64, values: [i64; 2]) -> ObjectRef {
    let mut members = Vec::new();
    for (offset, value) in values.into_iter().enumerate() {
        let timestamp = first + offset as u64 * 10;
        let observation = pipeline
            .admit(ObservationRequest {
                source: reference("source"),
                timestamp: Some(timestamp),
                payload: value.to_le_bytes().to_vec(),
                acquisition_context: format!("fixture/record/{timestamp}"),
            })
            .unwrap();
        members.push(
            pipeline
                .measure(MeasurementRequest {
                    operator: reference("read"),
                    inputs: vec![observation],
                })
                .unwrap(),
        );
    }
    let edge = pipeline
        .relate(RelationRequest {
            contract: reference("ratio"),
            left: members[0].clone(),
            right: members[1].clone(),
        })
        .unwrap();
    let composition = pipeline
        .compose(CompositionRequest {
            contract: reference("composition"),
            members,
            relations: vec![edge],
        })
        .unwrap();
    pipeline
        .derive_state(StateRequest {
            contract: reference("state"),
            composition,
            observation_horizon: Some(50),
        })
        .unwrap()
}

fn main() {
    let mut pipeline = Pipeline::new(registry());
    let source = state(&mut pipeline, 10, [12, 6]);
    let target = state(&mut pipeline, 30, [8, 4]);
    assert_eq!(pipeline.transition_count(), 0);
    let reference = pipeline
        .transition(TransitionRequest {
            contract: reference("transition"),
            source,
            target,
            observation_horizon: Some(50),
        })
        .unwrap();
    let transition = pipeline.resolve_transition(&reference).unwrap();
    assert_eq!(transition.conditions_result(), ConformanceResult::Pass);
    assert_eq!(transition.status(), LifecycleStatus::Candidate);
    let trace = pipeline.trace(&reference).unwrap();
    assert_eq!(trace.len(), 16);
    println!(
        "states={} transitions={} result={:?} status={:?} lineage_nodes={}",
        pipeline.state_count(),
        pipeline.transition_count(),
        transition.conditions_result(),
        transition.status(),
        trace.len()
    );
}
