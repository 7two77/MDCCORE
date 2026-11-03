use mdc_core::contract::registry::{
    DomainContext, OperatorContract, OperatorKind, Registry, ScalarSchema, SchemaContract,
    SourceContract, VersionedId,
};
use mdc_core::pipeline::{MeasurementRequest, ObservationRequest, Pipeline};

fn reference(id: &str) -> VersionedId {
    VersionedId::new(id, 1).unwrap()
}

fn main() {
    let schema = reference("fixture/i64-le");
    let source = reference("fixture/source");
    let difference = reference("fixture/difference");
    let mut registry = Registry::new();
    registry
        .register_schema(SchemaContract {
            reference: schema.clone(),
            format: ScalarSchema::SignedI64Le,
        })
        .unwrap();
    registry
        .register_source(SourceContract {
            reference: source.clone(),
            schema,
            context: DomainContext::new("fixture/counter", reference("fixture/ticks")).unwrap(),
            unit: "count".into(),
        })
        .unwrap();
    registry
        .register_operator(OperatorContract {
            reference: difference.clone(),
            kind: OperatorKind::Difference,
        })
        .unwrap();

    let mut pipeline = Pipeline::new(registry);
    let first = pipeline
        .admit(ObservationRequest {
            source: source.clone(),
            timestamp: Some(100),
            payload: 17_i64.to_le_bytes().to_vec(),
            acquisition_context: "fixture/record-a".into(),
        })
        .unwrap();
    let second = pipeline
        .admit(ObservationRequest {
            source,
            timestamp: Some(101),
            payload: 5_i64.to_le_bytes().to_vec(),
            acquisition_context: "fixture/record-b".into(),
        })
        .unwrap();
    let result = pipeline
        .measure(MeasurementRequest {
            operator: difference,
            inputs: vec![first, second],
        })
        .unwrap();
    let measurement = pipeline.resolve_measurement(&result).unwrap();
    assert_eq!(measurement.value(), 12);
    let trace = pipeline.trace(&result).unwrap();
    assert_eq!(trace.len(), 4);
    println!(
        "value={} unit={} lineage_nodes={}",
        measurement.value(),
        measurement.unit(),
        trace.len()
    );
}
