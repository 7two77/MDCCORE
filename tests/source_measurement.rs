//! Executable coverage of the bounded Source -> Observation -> Measurement slice.
//! These tests call production admission/measurement APIs; they are not the full
//! MDC-E2E-CONFORMANCE-001 fixture.

use mdc_core::canonical::snapshot::snapshot;
use mdc_core::contract::evaluation::LifecycleStatus;
use mdc_core::contract::registry::{
    DomainContext, OperatorContract, OperatorKind, Registry, ScalarSchema, SchemaContract,
    SourceContract, VersionedId,
};
use mdc_core::identity::{CanonicalSemanticBasis, IdentityAdapter};
use mdc_core::model::measurement::MeasurementError;
use mdc_core::model::observation::{AdmissionError, Observation, Source};
use mdc_core::pipeline::{
    MeasurementRequest, ObservationRequest, Pipeline, PipelineError, RejectedRequest,
};
use mdc_core::provenance::ObjectRef;

type CapturedGraph = Vec<(ObjectRef, u64, Vec<u8>)>;

fn reference(name: &str, version: u32) -> VersionedId {
    VersionedId::new(name, version).unwrap()
}

fn context(domain: &str, time_basis: &str) -> DomainContext {
    DomainContext::new(domain, reference(time_basis, 1)).unwrap()
}

fn registry() -> Registry {
    let mut registry = Registry::new();
    registry
        .register_schema(SchemaContract {
            reference: reference("scalar-i64-le", 1),
            format: ScalarSchema::SignedI64Le,
        })
        .unwrap();
    add_source(&mut registry, "source", "fixture", "ticks", "count");
    registry
        .register_operator(OperatorContract {
            reference: reference("read", 1),
            kind: OperatorKind::Scalar,
        })
        .unwrap();
    registry
        .register_operator(OperatorContract {
            reference: reference("delta", 1),
            kind: OperatorKind::Difference,
        })
        .unwrap();
    registry
}

fn add_source(registry: &mut Registry, name: &str, domain: &str, time_basis: &str, unit: &str) {
    registry
        .register_source(SourceContract {
            reference: reference(name, 1),
            schema: reference("scalar-i64-le", 1),
            context: context(domain, time_basis),
            unit: unit.into(),
        })
        .unwrap();
}

fn observation_request(source: &str, timestamp: u64, value: i64) -> ObservationRequest {
    ObservationRequest {
        source: reference(source, 1),
        timestamp: Some(timestamp),
        payload: value.to_le_bytes().to_vec(),
        acquisition_context: "fixture-batch-001".into(),
    }
}

fn measure(operator: &str, inputs: Vec<ObjectRef>) -> MeasurementRequest {
    MeasurementRequest {
        operator: reference(operator, 1),
        inputs,
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
                ObjectRef::Source(_)
                | ObjectRef::Relation(_)
                | ObjectRef::Composition(_)
                | ObjectRef::State(_)
                | ObjectRef::Transition(_)
                | ObjectRef::Evaluation(_)
                | ObjectRef::EvaluationKey(_)
                | ObjectRef::InvalidObservation(_)
                | ObjectRef::InvalidMeasurement(_)
                | ObjectRef::InvalidRelation(_)
                | ObjectRef::InvalidComposition(_)
                | ObjectRef::InvalidState(_)
                | ObjectRef::InvalidTransition(_) => {
                    panic!("capture expects scalar semantic objects")
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

fn assert_rejected(pipeline: &mut Pipeline, request: RejectedRequest, expected: PipelineError) {
    let counts = (pipeline.observation_count(), pipeline.measurement_count());
    let rejected_count = pipeline.rejections().len();
    let result = match request.clone() {
        RejectedRequest::Observation(request) => pipeline.admit(request),
        RejectedRequest::Measurement(request) => pipeline.measure(request),
        RejectedRequest::Relation(_)
        | RejectedRequest::Composition(_)
        | RejectedRequest::State(_)
        | RejectedRequest::Transition(_) => {
            panic!("this suite submits only observation/measurement requests")
        }
    };
    assert_eq!(result, Err(expected.clone()));
    assert_eq!(
        (pipeline.observation_count(), pipeline.measurement_count()),
        counts,
    );
    assert_eq!(pipeline.rejections().len(), rejected_count + 1);
    let rejection = pipeline.rejections().last().unwrap();
    assert_eq!(rejection.request(), &request);
    assert_eq!(rejection.error(), &expected);
    assert_eq!(rejection.status(), LifecycleStatus::Invalid);
}

#[test]
fn admission_records_resolved_source_schema_context_and_provenance() {
    let mut pipeline = Pipeline::new(registry());
    let reference = pipeline
        .admit(observation_request("source", 10, -17))
        .unwrap();
    let observation = pipeline.resolve_observation(&reference).unwrap();

    assert_eq!(reference, ObjectRef::Observation(observation.id()));
    assert_eq!(observation.source_ref(), &self::reference("source", 1));
    assert_eq!(observation.schema(), &self::reference("scalar-i64-le", 1));
    assert_eq!(observation.timestamp(), 10);
    assert_eq!(observation.payload(), &(-17_i64).to_le_bytes());
    assert_eq!(observation.context(), &context("fixture", "ticks"));
    assert_eq!(observation.unit(), "count");
    assert_eq!(observation.provenance().source(), observation.source_ref());
    assert_eq!(observation.provenance().schema(), observation.schema());
    assert_eq!(
        observation.provenance().acquisition_context(),
        "fixture-batch-001",
    );
    assert_eq!(observation.status(), LifecycleStatus::Candidate);
    assert_eq!(
        observation.id(),
        IdentityAdapter::derive_identity_from_basis(&observation.canonical_basis()),
    );
}

#[test]
fn registered_measurements_compute_values_and_recursive_lineage_to_source() {
    let mut pipeline = Pipeline::new(registry());
    let a = pipeline
        .admit(observation_request("source", 10, 17))
        .unwrap();
    let b = pipeline
        .admit(observation_request("source", 20, 5))
        .unwrap();
    let ma = pipeline.measure(measure("read", vec![a.clone()])).unwrap();
    let mb = pipeline.measure(measure("read", vec![b.clone()])).unwrap();
    let upstream = vec![a.clone(), b.clone(), ma.clone(), mb.clone()];
    let before = capture(&pipeline, &upstream);
    let difference = pipeline
        .measure(measure("delta", vec![ma.clone(), mb.clone()]))
        .unwrap();
    let result = pipeline.resolve_measurement(&difference).unwrap();

    assert_eq!(result.value(), 12);
    assert_eq!(result.unit(), "count");
    assert_eq!(result.context(), &context("fixture", "ticks"));
    assert_eq!(result.operator(), &reference("delta", 1));
    assert_eq!(result.inputs(), &[ma.clone(), mb.clone()]);
    assert_eq!(result.provenance().contract(), result.operator());
    assert_eq!(result.provenance().inputs(), result.inputs());
    assert_eq!(result.status(), LifecycleStatus::Candidate);
    let mut expected = vec![
        ObjectRef::Source(reference("source", 1)),
        a,
        b,
        ma,
        mb,
        difference.clone(),
    ];
    expected.sort();
    assert_eq!(pipeline.trace(&difference).unwrap(), expected);
    assert_eq!(capture(&pipeline, &upstream), before);
}

#[test]
fn noncommutative_inputs_preserve_order_in_results_and_canonical_content() {
    let mut pipeline = Pipeline::new(registry());
    let a = pipeline
        .admit(observation_request("source", 10, 17))
        .unwrap();
    let b = pipeline
        .admit(observation_request("source", 20, 5))
        .unwrap();
    let forward = pipeline
        .measure(measure("delta", vec![a.clone(), b.clone()]))
        .unwrap();
    let reverse = pipeline
        .measure(measure("delta", vec![b.clone(), a.clone()]))
        .unwrap();
    let forward = pipeline.resolve_measurement(&forward).unwrap();
    let reverse = pipeline.resolve_measurement(&reverse).unwrap();

    assert_eq!(forward.value(), 12);
    assert_eq!(reverse.value(), -12);
    assert_eq!(forward.inputs(), &[a.clone(), b.clone()]);
    assert_eq!(reverse.inputs(), &[b, a]);
    assert_ne!(forward.canonical_basis(), reverse.canonical_basis());
    assert_ne!(forward.id(), reverse.id());
}

#[test]
fn equivalent_graphs_are_independent_of_admission_and_construction_order() {
    fn run(reverse: bool) -> (CapturedGraph, Vec<ObjectRef>) {
        let mut pipeline = Pipeline::new(registry());
        let request_a = observation_request("source", 10, 17);
        let request_b = observation_request("source", 20, 5);
        let (a, b) = if reverse {
            let b = pipeline.admit(request_b).unwrap();
            let a = pipeline.admit(request_a).unwrap();
            (a, b)
        } else {
            let a = pipeline.admit(request_a).unwrap();
            let b = pipeline.admit(request_b).unwrap();
            (a, b)
        };
        let (ma, mb) = if reverse {
            let mb = pipeline.measure(measure("read", vec![b.clone()])).unwrap();
            let ma = pipeline.measure(measure("read", vec![a.clone()])).unwrap();
            (ma, mb)
        } else {
            let ma = pipeline.measure(measure("read", vec![a.clone()])).unwrap();
            let mb = pipeline.measure(measure("read", vec![b.clone()])).unwrap();
            (ma, mb)
        };
        let difference = pipeline
            .measure(measure("delta", vec![ma.clone(), mb.clone()]))
            .unwrap();
        let mut objects = vec![a, b, ma, mb, difference.clone()];
        objects.sort();
        (
            capture(&pipeline, &objects),
            pipeline.trace(&difference).unwrap(),
        )
    }

    assert_eq!(run(false), run(false));
    assert_eq!(run(false), run(true));
}

#[test]
fn repeated_requests_reuse_objects_without_changing_upstream_snapshots() {
    let mut pipeline = Pipeline::new(registry());
    let admission = observation_request("source", 10, 17);
    let observation = pipeline.admit(admission.clone()).unwrap();
    assert_eq!(pipeline.admit(admission).unwrap(), observation);
    let request = measure("read", vec![observation.clone()]);
    let measurement = pipeline.measure(request.clone()).unwrap();
    let objects = vec![observation, measurement.clone()];
    let before = capture(&pipeline, &objects);
    assert_eq!(pipeline.measure(request).unwrap(), measurement);
    assert_eq!(pipeline.observation_count(), 1);
    assert_eq!(pipeline.measurement_count(), 1);
    assert_eq!(capture(&pipeline, &objects), before);
    assert!(pipeline.rejections().is_empty());
}

#[test]
fn missing_source_or_source_version_is_rejected_without_fallback() {
    let mut pipeline = Pipeline::new(registry());
    for source in [reference("missing", 1), reference("source", 2)] {
        let mut request = observation_request("source", 10, 17);
        request.source = source.clone();
        assert_rejected(
            &mut pipeline,
            RejectedRequest::Observation(request),
            PipelineError::SourceUnresolved(source),
        );
    }
}

#[test]
fn malformed_or_incomplete_admissions_are_retained_with_exact_reasons() {
    let mut pipeline = Pipeline::new(registry());
    let existing = pipeline
        .admit(observation_request("source", 10, 17))
        .unwrap();
    let before = capture(&pipeline, std::slice::from_ref(&existing));
    let mut malformed = observation_request("source", 20, 5);
    malformed.payload.pop();
    let mut timeless = observation_request("source", 20, 5);
    timeless.timestamp = None;
    let mut no_acquisition = observation_request("source", 20, 5);
    no_acquisition.acquisition_context.clear();

    for (request, error) in [
        (
            malformed,
            AdmissionError::InvalidPayload {
                expected: 8,
                actual: 7,
            },
        ),
        (timeless, AdmissionError::MissingTimestamp),
        (no_acquisition, AdmissionError::EmptyAcquisitionContext),
    ] {
        assert_rejected(
            &mut pipeline,
            RejectedRequest::Observation(request),
            PipelineError::Admission(error),
        );
    }
    assert_eq!(capture(&pipeline, &[existing]), before);
}

#[test]
fn operators_must_be_registered_at_the_requested_version() {
    let mut pipeline = Pipeline::new(registry());
    let observation = pipeline
        .admit(observation_request("source", 10, 17))
        .unwrap();
    for operator in [reference("missing", 1), reference("read", 2)] {
        assert_rejected(
            &mut pipeline,
            RejectedRequest::Measurement(MeasurementRequest {
                operator: operator.clone(),
                inputs: vec![observation.clone()],
            }),
            PipelineError::OperatorUnresolved(operator),
        );
    }
}

#[test]
fn missing_or_wrongly_typed_inputs_are_not_synthesized() {
    let mut pipeline = Pipeline::new(registry());
    for missing in [ObjectRef::Observation(0), ObjectRef::Measurement(0)] {
        assert_rejected(
            &mut pipeline,
            RejectedRequest::Measurement(measure("read", vec![missing.clone()])),
            PipelineError::InputUnresolved(missing.clone()),
        );
        assert!(pipeline.resolve_observation(&missing).is_none());
        assert!(pipeline.resolve_measurement(&missing).is_none());
    }
    let source = ObjectRef::Source(reference("source", 1));
    assert_rejected(
        &mut pipeline,
        RejectedRequest::Measurement(measure("read", vec![source.clone()])),
        PipelineError::InvalidInputType(source),
    );
    assert_eq!(pipeline.observation_count(), 0);
    assert_eq!(pipeline.measurement_count(), 0);
}

#[test]
fn wrong_arity_is_rejected_without_a_default_or_fallback_value() {
    let mut pipeline = Pipeline::new(registry());
    let observation = pipeline
        .admit(observation_request("source", 10, 17))
        .unwrap();
    for inputs in [vec![], vec![observation.clone(), observation]] {
        let actual = inputs.len();
        assert_rejected(
            &mut pipeline,
            RejectedRequest::Measurement(measure("read", inputs)),
            PipelineError::Measurement(MeasurementError::WrongInputCount {
                expected: 1,
                actual,
            }),
        );
    }
}

#[test]
fn incompatible_units_domains_and_temporal_contexts_are_rejected() {
    let mut registry = registry();
    add_source(&mut registry, "other-unit", "fixture", "ticks", "price");
    add_source(&mut registry, "other-domain", "other", "ticks", "count");
    add_source(&mut registry, "other-time", "fixture", "seconds", "count");
    registry
        .register_source(SourceContract {
            reference: reference("other-time-version", 1),
            schema: reference("scalar-i64-le", 1),
            context: DomainContext::new("fixture", reference("ticks", 2)).unwrap(),
            unit: "count".into(),
        })
        .unwrap();
    let mut pipeline = Pipeline::new(registry);
    let base = pipeline
        .admit(observation_request("source", 10, 17))
        .unwrap();
    for (source, expected) in [
        ("other-unit", MeasurementError::IncompatibleUnits),
        ("other-domain", MeasurementError::IncompatibleContext),
        ("other-time", MeasurementError::IncompatibleContext),
        ("other-time-version", MeasurementError::IncompatibleContext),
    ] {
        let other = pipeline.admit(observation_request(source, 20, 5)).unwrap();
        assert_rejected(
            &mut pipeline,
            RejectedRequest::Measurement(measure("delta", vec![base.clone(), other])),
            PipelineError::Measurement(expected),
        );
    }
}

#[test]
fn source_schema_versions_units_and_context_are_in_the_observation_basis() {
    fn admit(
        source_version: u32,
        schema_version: u32,
        unit: &str,
        domain: &str,
        time_basis_version: u32,
    ) -> Vec<u8> {
        let mut registry = Registry::new();
        let schema = reference("scalar-i64-le", schema_version);
        registry
            .register_schema(SchemaContract {
                reference: schema.clone(),
                format: ScalarSchema::SignedI64Le,
            })
            .unwrap();
        let source = reference("source", source_version);
        registry
            .register_source(SourceContract {
                reference: source.clone(),
                schema,
                context: DomainContext::new(domain, reference("ticks", time_basis_version))
                    .unwrap(),
                unit: unit.into(),
            })
            .unwrap();
        let mut pipeline = Pipeline::new(registry);
        let mut request = observation_request("source", 10, 17);
        request.source = source;
        let reference = pipeline.admit(request).unwrap();
        pipeline
            .resolve_observation(&reference)
            .unwrap()
            .canonical_basis()
    }

    let original = admit(1, 1, "count", "fixture", 1);
    for changed in [
        admit(2, 1, "count", "fixture", 1),
        admit(1, 2, "count", "fixture", 1),
        admit(1, 1, "price", "fixture", 1),
        admit(1, 1, "count", "other", 1),
        admit(1, 1, "count", "fixture", 2),
    ] {
        assert_ne!(original, changed);
    }
}

#[test]
fn arithmetic_overflow_is_retained_without_mutating_the_admitted_graph() {
    let mut pipeline = Pipeline::new(registry());
    let zero = pipeline.admit(observation_request("source", 0, 0)).unwrap();
    let start = pipeline
        .admit(observation_request("source", 1, i64::MAX))
        .unwrap();
    let mut current = pipeline.measure(measure("read", vec![start])).unwrap();
    let mut value = i128::from(i64::MAX);
    loop {
        let negative = pipeline
            .measure(measure("delta", vec![zero.clone(), current.clone()]))
            .unwrap();
        let request = measure("delta", vec![current.clone(), negative]);
        if let Some(next) = value.checked_mul(2) {
            current = pipeline.measure(request).unwrap();
            value = next;
            assert_eq!(
                pipeline.resolve_measurement(&current).unwrap().value(),
                value
            );
        } else {
            let before = capture(&pipeline, std::slice::from_ref(&current));
            assert_rejected(
                &mut pipeline,
                RejectedRequest::Measurement(request),
                PipelineError::Measurement(MeasurementError::ArithmeticOverflow),
            );
            assert_eq!(capture(&pipeline, &[current]), before);
            break;
        }
    }
}

#[test]
fn legacy_unadmitted_observations_do_not_bypass_admission() {
    struct LegacySource;
    impl Source for LegacySource {
        fn id(&self) -> &str {
            "source"
        }
    }
    let legacy = Observation::new(LegacySource, 10, 17_i64.to_le_bytes().to_vec()).unwrap();
    let unadmitted = ObjectRef::Observation(IdentityAdapter::derive_identity(&legacy));
    let mut pipeline = Pipeline::new(registry());
    assert_rejected(
        &mut pipeline,
        RejectedRequest::Measurement(measure("read", vec![unadmitted.clone()])),
        PipelineError::InputUnresolved(unadmitted),
    );
}
