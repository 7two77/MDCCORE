#[allow(dead_code)]
#[path = "state_transition.rs"]
mod fixture;
use mdc_core::contract::evaluation::LifecycleStatus;
use mdc_core::contract::registry::VersionedId;
use mdc_core::model::invalid::InvalidBody;
use mdc_core::pipeline::{MeasurementRequest, Pipeline, PipelineError};
use mdc_core::provenance::ObjectRef;

fn main() {
    let mut pipeline = Pipeline::new(fixture::registry());
    let missing = ObjectRef::Observation(0);
    let error = pipeline
        .measure(MeasurementRequest {
            operator: VersionedId::new("read", 1).unwrap(),
            inputs: vec![missing.clone()],
        })
        .unwrap_err();
    assert_eq!(error, PipelineError::InputUnresolved(missing.clone()));
    let reference = pipeline.rejections().last().unwrap().invalid_reference();
    let invalid = pipeline.resolve_invalid(reference).unwrap();
    assert_eq!(invalid.status(), LifecycleStatus::Invalid);
    assert!(matches!(
        invalid.body(),
        InvalidBody::Measurement {
            value: None,
            definition: Some(_),
            ..
        }
    ));
    let report = pipeline.lineage_report(reference);
    assert!(!report.is_complete());
    assert_eq!(report.unresolved, vec![missing]);
    assert_eq!(pipeline.measurement_count(), 0);
    println!(
        "kind={:?} status={:?} reference_closure_complete={} unresolved={}",
        invalid.kind(),
        invalid.status(),
        report.is_complete(),
        report.unresolved.len()
    );
}
