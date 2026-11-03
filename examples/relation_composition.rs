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
};

fn reference(id: &str) -> VersionedId {
    VersionedId::new(id, 1).unwrap()
}

fn main() {
    let domain = DomainContext::new("fixture/counter", reference("fixture/ticks")).unwrap();
    let context = RelationContext::new(
        domain.clone(),
        TemporalScope::new(0, 100).unwrap(),
        reference("fixture/frame"),
        reference("fixture/noncausal"),
    );
    let mut registry = Registry::new();
    registry
        .register_schema(SchemaContract {
            reference: reference("fixture/i64-le"),
            format: ScalarSchema::SignedI64Le,
        })
        .unwrap();
    registry
        .register_source(SourceContract {
            reference: reference("fixture/source"),
            schema: reference("fixture/i64-le"),
            context: domain,
            unit: "count".into(),
        })
        .unwrap();
    registry
        .register_operator(OperatorContract {
            reference: reference("fixture/read"),
            kind: OperatorKind::Scalar,
        })
        .unwrap();
    registry
        .register_relation(RelationContract {
            reference: reference("fixture/proportion"),
            kind: RelationKind::Proportion,
            context: context.clone(),
        })
        .unwrap();
    let mut constraints = Vec::new();
    for (name, kind) in [
        (
            "fixture/cardinality",
            ConstraintKind::Cardinality { min: 2, max: 2 },
        ),
        ("fixture/connected", ConstraintKind::Connected),
        ("fixture/units", ConstraintKind::SameUnits),
    ] {
        let constraint = reference(name);
        registry
            .register_constraint(ConstraintContract {
                reference: constraint.clone(),
                kind,
            })
            .unwrap();
        constraints.push(constraint);
    }
    registry
        .register_composition(CompositionContract {
            reference: reference("fixture/composition"),
            member_order: MemberOrder::Unordered,
            member_type: MemberType::Measurements,
            constraints,
            context,
        })
        .unwrap();
    let mut pipeline = Pipeline::new(registry);
    let mut members = Vec::new();
    for (timestamp, value, coordinate) in [
        (10, 17_i64, "fixture/record-a"),
        (20, 5_i64, "fixture/record-b"),
    ] {
        let observation = pipeline
            .admit(ObservationRequest {
                source: reference("fixture/source"),
                timestamp: Some(timestamp),
                payload: value.to_le_bytes().to_vec(),
                acquisition_context: coordinate.into(),
            })
            .unwrap();
        members.push(
            pipeline
                .measure(MeasurementRequest {
                    operator: reference("fixture/read"),
                    inputs: vec![observation],
                })
                .unwrap(),
        );
    }
    let relation = pipeline
        .relate(RelationRequest {
            contract: reference("fixture/proportion"),
            left: members[0].clone(),
            right: members[1].clone(),
        })
        .unwrap();
    let composed = pipeline
        .compose(CompositionRequest {
            contract: reference("fixture/composition"),
            members,
            relations: vec![relation],
        })
        .unwrap();
    let composition = pipeline.resolve_composition(&composed).unwrap();
    assert_eq!(composition.constraints_result(), ConformanceResult::Pass);
    assert_eq!(composition.status(), LifecycleStatus::Candidate);
    let trace = pipeline.trace(&composed).unwrap();
    assert_eq!(trace.len(), 7);
    println!(
        "members={} relations={} status={:?} lineage_nodes={}",
        composition.members().len(),
        composition.relations().len(),
        composition.status(),
        trace.len()
    );
}
