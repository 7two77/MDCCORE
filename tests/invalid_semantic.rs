//! Typed invalid results are retained semantic objects, never usable inputs.
#[allow(dead_code)]
#[path = "../examples/state_transition.rs"]
mod fixture;

use mdc_core::contract::conformance::{
    ConformanceContract, ConformanceCriterionContract, ConformanceCriterionKind,
    MissingTargetPolicy, TargetType,
};
use mdc_core::contract::dynamics::{CriterionOrder, StateCriterionContract, StateCriterionKind};
use mdc_core::contract::evaluation::{ConformanceResult, LifecycleStatus};
use mdc_core::contract::registry::{Registry, VersionedId};
use mdc_core::identity::{CanonicalSemanticBasis, IdentityAdapter};
use mdc_core::model::evaluation::EvaluationDiagnostic;
use mdc_core::model::invalid::{DefinitionKind, InvalidBody, InvalidKind};
use mdc_core::pipeline::{
    CompositionRequest, EvaluationRequest, MeasurementRequest, ObservationRequest, Pipeline,
    PipelineError, RelationRequest, StateRequest, TransitionRequest,
};
use mdc_core::provenance::ObjectRef;

type Snapshot = Vec<(ObjectRef, Vec<u8>)>;

fn id(name: &str) -> VersionedId {
    VersionedId::new(name, 1).unwrap()
}

fn registry() -> Registry {
    let mut registry = fixture::registry();
    registry
        .register_state_criterion(StateCriterionContract {
            reference: id("three-members"),
            kind: StateCriterionKind::Cardinality { min: 3, max: 3 },
        })
        .unwrap();
    let mut confirmation = registry.source(&id("source")).unwrap().clone();
    confirmation.reference = id("confirmation-source");
    registry.register_source(confirmation).unwrap();
    registry
        .register_state_criterion(StateCriterionContract {
            reference: id("confirmation"),
            kind: StateCriterionKind::ObservationAfter {
                source: id("confirmation-source"),
            },
        })
        .unwrap();
    let mut bad_state = registry.state(&id("state")).unwrap().clone();
    bad_state.reference = id("failed-state");
    bad_state
        .criteria
        .extend([id("three-members"), id("confirmation")]);
    registry.register_state(bad_state).unwrap();
    for (name, kind) in [
        ("check-exists", ConformanceCriterionKind::TargetExists),
        ("check-lineage", ConformanceCriterionKind::LineageComplete),
    ] {
        registry
            .register_conformance_criterion(ConformanceCriterionContract {
                reference: id(name),
                kind,
            })
            .unwrap();
    }
    let context = registry.state(&id("state")).unwrap().context.clone();
    registry
        .register_conformance(ConformanceContract {
            reference: id("check-measurement"),
            target_type: TargetType::Measurement,
            criteria: vec![id("check-exists"), id("check-lineage")],
            criterion_order: CriterionOrder::Unordered,
            context,
            missing_target: MissingTargetPolicy::Unresolved,
        })
        .unwrap();
    registry
}

fn observation(timestamp: u64, value: i64) -> ObservationRequest {
    ObservationRequest {
        source: id("source"),
        timestamp: Some(timestamp),
        payload: value.to_le_bytes().to_vec(),
        acquisition_context: format!("fixture/extra/{timestamp}"),
    }
}

fn counts(pipeline: &Pipeline) -> [usize; 7] {
    [
        pipeline.observation_count(),
        pipeline.measurement_count(),
        pipeline.relation_count(),
        pipeline.composition_count(),
        pipeline.state_count(),
        pipeline.transition_count(),
        pipeline.evaluation_count(),
    ]
}

fn capture(pipeline: &Pipeline, graph: &[ObjectRef]) -> Snapshot {
    graph
        .iter()
        .filter_map(|reference| {
            let basis = match reference {
                ObjectRef::Observation(_) => {
                    pipeline.resolve_observation(reference)?.canonical_basis()
                }
                ObjectRef::Measurement(_) => {
                    pipeline.resolve_measurement(reference)?.canonical_basis()
                }
                ObjectRef::Relation(_) => pipeline.resolve_relation(reference)?.canonical_basis(),
                ObjectRef::Composition(_) => {
                    pipeline.resolve_composition(reference)?.canonical_basis()
                }
                ObjectRef::State(_) => pipeline.resolve_state(reference)?.canonical_basis(),
                ObjectRef::Transition(_) => {
                    pipeline.resolve_transition(reference)?.canonical_basis()
                }
                _ => return None,
            };
            Some((reference.clone(), basis))
        })
        .collect()
}

fn fail(
    pipeline: &mut Pipeline,
    operation: impl FnOnce(&mut Pipeline) -> Result<ObjectRef, PipelineError>,
) -> (ObjectRef, PipelineError) {
    let before = counts(pipeline);
    let receipts = pipeline.rejections().len();
    let error = operation(pipeline).unwrap_err();
    assert_eq!(counts(pipeline), before);
    assert_eq!(pipeline.rejections().len(), receipts + 1);
    let receipt = pipeline.rejections().last().unwrap();
    assert_eq!(receipt.error(), &error);
    let reference = receipt.invalid_reference().clone();
    let invalid = pipeline.resolve_invalid(&reference).unwrap();
    assert_eq!(invalid.reference(), reference);
    assert_eq!(invalid.error(), &error);
    assert_eq!(invalid.status(), LifecycleStatus::Invalid);
    assert_eq!(invalid.id().canonical_key(), invalid.canonical_basis());
    assert_eq!(
        invalid.id().digest(),
        IdentityAdapter::derive_identity(invalid)
    );
    (reference, error)
}

fn members(pipeline: &Pipeline, state: &ObjectRef) -> (ObjectRef, Vec<ObjectRef>) {
    let composition = pipeline
        .resolve_state(state)
        .unwrap()
        .source_composition()
        .clone();
    let members = pipeline
        .resolve_composition(&composition)
        .unwrap()
        .members()
        .to_vec();
    (composition, members)
}

#[test]
fn failures_retain_six_typed_semantic_bodies_and_leave_the_valid_graph_unchanged() {
    let mut pipeline = Pipeline::new(registry());
    let source = fixture::state(&mut pipeline, 10, [12, 6]);
    let target = fixture::state(&mut pipeline, 30, [8, 4]);
    let mut graph = pipeline.trace(&source).unwrap();
    graph.extend(pipeline.trace(&target).unwrap());
    let before = capture(&pipeline, &graph);
    let (composition, members) = members(&pipeline, &source);
    let obs = pipeline.resolve_measurement(&members[0]).unwrap().inputs()[0].clone();
    let mut malformed = observation(80, 7);
    malformed.payload.pop();
    let (o, _) = fail(&mut pipeline, |p| p.admit(malformed.clone()));
    assert!(
        matches!(pipeline.resolve_invalid(&o).unwrap().body(), InvalidBody::Observation {
        definition: Some(_), schema: Some(_), timestamp: Some(80), payload, ..
    } if payload.len() == 7)
    );
    let (m, _) = fail(&mut pipeline, |p| {
        p.measure(MeasurementRequest {
            operator: id("read"),
            inputs: vec![ObjectRef::Observation(0)],
        })
    });
    assert!(matches!(
        pipeline.resolve_invalid(&m).unwrap().body(),
        InvalidBody::Measurement {
            definition: Some(_),
            value: None,
            ..
        }
    ));
    let (r, _) = fail(&mut pipeline, |p| {
        p.relate(RelationRequest {
            contract: id("ratio"),
            left: obs.clone(),
            right: obs.clone(),
        })
    });
    assert!(
        matches!(pipeline.resolve_invalid(&r).unwrap().body(), InvalidBody::Relation {
        definition: Some(_), left, right, value: None, ..
    } if left == &obs && right == &obs)
    );
    let (c, _) = fail(&mut pipeline, |p| {
        p.compose(CompositionRequest {
            contract: id("composition"),
            members: members.clone(),
            relations: vec![ObjectRef::Relation(0)],
        })
    });
    assert!(matches!(
        pipeline.resolve_invalid(&c).unwrap().body(),
        InvalidBody::Composition {
            definition: Some(_),
            constraints: None,
            ..
        }
    ));
    let (s, _) = fail(&mut pipeline, |p| {
        p.derive_state(StateRequest {
            contract: id("failed-state"),
            composition: composition.clone(),
            observation_horizon: Some(50),
        })
    });
    assert!(matches!(
        pipeline.resolve_invalid(&s).unwrap().body(),
        InvalidBody::State {
            definition: Some(_),
            interval: Some(_),
            criteria: Some(_),
            ..
        }
    ));
    let (t, _) = fail(&mut pipeline, |p| {
        p.transition(TransitionRequest {
            contract: id("transition"),
            source: target.clone(),
            target: source.clone(),
            observation_horizon: Some(50),
        })
    });
    assert!(matches!(
        pipeline.resolve_invalid(&t).unwrap().body(),
        InvalidBody::Transition {
            definition: Some(_),
            conditions: Some(_),
            ..
        }
    ));
    for (reference, kind) in [
        (o, InvalidKind::Observation),
        (m, InvalidKind::Measurement),
        (r, InvalidKind::Relation),
        (c, InvalidKind::Composition),
        (s, InvalidKind::State),
        (t, InvalidKind::Transition),
    ] {
        assert_eq!(pipeline.resolve_invalid(&reference).unwrap().kind(), kind);
    }
    assert_eq!(pipeline.invalid_count(), 6);
    assert_eq!(capture(&pipeline, &graph), before);
}

#[test]
fn failed_local_evaluations_preserve_every_pass_fail_and_unresolved_result() {
    let mut pipeline = Pipeline::new(registry());
    let state = fixture::state(&mut pipeline, 10, [12, 6]);
    let (composition, members) = members(&pipeline, &state);
    let (c, _) = fail(&mut pipeline, |p| {
        p.compose(CompositionRequest {
            contract: id("composition"),
            members: vec![members[0].clone()],
            relations: vec![],
        })
    });
    match pipeline.resolve_invalid(&c).unwrap().body() {
        InvalidBody::Composition {
            constraints: Some(results),
            ..
        } => {
            assert_eq!(results.len(), 2);
            assert!(results
                .iter()
                .any(|r| r.result() == ConformanceResult::Pass));
            assert!(results
                .iter()
                .any(|r| r.result() == ConformanceResult::Fail));
        }
        other => panic!("missing computed constraints: {other:?}"),
    }
    let (s, _) = fail(&mut pipeline, |p| {
        p.derive_state(StateRequest {
            contract: id("failed-state"),
            composition,
            observation_horizon: Some(50),
        })
    });
    match pipeline.resolve_invalid(&s).unwrap().body() {
        InvalidBody::State {
            criteria: Some(results),
            ..
        } => {
            for expected in [
                ConformanceResult::Pass,
                ConformanceResult::Fail,
                ConformanceResult::Unresolved,
            ] {
                assert!(results.iter().any(|r| r.result() == expected));
            }
        }
        other => panic!("missing computed state criteria: {other:?}"),
    }
}

#[test]
fn complete_reference_closure_does_not_make_an_invalid_relation_admissible() {
    let mut pipeline = Pipeline::new(registry());
    let a = pipeline.admit(observation(70, 8)).unwrap();
    let b = pipeline.admit(observation(80, 4)).unwrap();
    let (invalid, _) = fail(&mut pipeline, |p| {
        p.relate(RelationRequest {
            contract: id("ratio"),
            left: a.clone(),
            right: b.clone(),
        })
    });
    let report = pipeline.lineage_report(&invalid);
    assert!(report.is_complete());
    assert!(report.resolved.contains(&invalid));
    assert!(report.resolved.contains(&a) && report.resolved.contains(&b));
    assert!(pipeline.trace(&invalid).is_ok());
    let (_, error) = fail(&mut pipeline, |p| {
        p.measure(MeasurementRequest {
            operator: id("read"),
            inputs: vec![invalid.clone()],
        })
    });
    assert_eq!(error, PipelineError::InvalidInputType(invalid));
}

#[test]
fn missing_object_references_and_missing_definitions_are_separate_and_never_synthesized() {
    let mut pipeline = Pipeline::new(registry());
    let (missing, _) = fail(&mut pipeline, |p| {
        p.measure(MeasurementRequest {
            operator: id("read"),
            inputs: vec![ObjectRef::Observation(0)],
        })
    });
    let report = pipeline.lineage_report(&missing);
    assert!(!report.is_complete());
    assert!(report.unresolved.contains(&ObjectRef::Observation(0)));
    assert!(report.unresolved_definitions.is_empty());
    assert!(pipeline
        .resolve_observation(&ObjectRef::Observation(0))
        .is_none());
    assert!(pipeline.trace(&missing).is_err());
    let actual = pipeline.admit(observation(70, 7)).unwrap();
    let unknown = VersionedId::new("read", 2).unwrap();
    let (definition, _) = fail(&mut pipeline, |p| {
        p.measure(MeasurementRequest {
            operator: unknown.clone(),
            inputs: vec![actual.clone()],
        })
    });
    let report = pipeline.lineage_report(&definition);
    assert!(report.unresolved.is_empty());
    assert!(report
        .unresolved_definitions
        .iter()
        .any(|d| d.kind == DefinitionKind::Operator && d.reference == unknown));
    assert!(!report.is_complete());
    assert!(matches!(
        pipeline.resolve_invalid(&definition).unwrap().body(),
        InvalidBody::Measurement {
            definition: None,
            value: None,
            ..
        }
    ));
}

#[test]
fn invalid_input_chains_retain_captured_missing_dependencies() {
    let mut pipeline = Pipeline::new(registry());
    let (first, _) = fail(&mut pipeline, |p| {
        p.measure(MeasurementRequest {
            operator: id("read"),
            inputs: vec![ObjectRef::Observation(0)],
        })
    });
    let (second, error) = fail(&mut pipeline, |p| {
        p.measure(MeasurementRequest {
            operator: id("read"),
            inputs: vec![first.clone()],
        })
    });
    assert_eq!(error, PipelineError::InvalidInputType(first.clone()));
    let report = pipeline.lineage_report(&second);
    assert!(report.resolved.contains(&first));
    assert!(report.resolved.contains(&second));
    assert!(report.unresolved.contains(&ObjectRef::Observation(0)));
}

#[test]
fn repeated_equivalent_failures_deduplicate_but_keep_every_audit_receipt() {
    fn run(reverse: bool) -> (ObjectRef, Vec<u8>) {
        let mut pipeline = Pipeline::new(registry());
        let a = observation(70, 7);
        let b = observation(80, 8);
        let (a, b) = if reverse {
            let b = pipeline.admit(b).unwrap();
            let a = pipeline.admit(a).unwrap();
            (a, b)
        } else {
            (pipeline.admit(a).unwrap(), pipeline.admit(b).unwrap())
        };
        let request = MeasurementRequest {
            operator: id("missing"),
            inputs: vec![a, b],
        };
        let (first, _) = fail(&mut pipeline, |p| p.measure(request.clone()));
        let (second, _) = fail(&mut pipeline, |p| p.measure(request));
        assert_eq!(first, second);
        assert_eq!(pipeline.invalid_count(), 1);
        assert_eq!(pipeline.rejections().len(), 2);
        let basis = pipeline.resolve_invalid(&first).unwrap().canonical_basis();
        (first, basis)
    }
    assert_eq!(run(false), run(true));
}

#[test]
fn later_admission_does_not_repair_the_old_invalid_snapshot() {
    let mut predictor = Pipeline::new(registry());
    let future_request = observation(80, 8);
    let future = predictor.admit(future_request.clone()).unwrap();
    let mut pipeline = Pipeline::new(registry());
    let request = MeasurementRequest {
        operator: id("missing"),
        inputs: vec![future.clone()],
    };
    let (old, old_error) = fail(&mut pipeline, |p| p.measure(request.clone()));
    let old_basis = pipeline.resolve_invalid(&old).unwrap().canonical_basis();
    let report = pipeline.lineage_report(&old);
    assert!(report.unresolved.contains(&future));
    assert_eq!(pipeline.admit(future_request).unwrap(), future);
    assert_eq!(pipeline.lineage_report(&old), report);
    assert_eq!(
        pipeline.resolve_invalid(&old).unwrap().canonical_basis(),
        old_basis
    );
    let (new, new_error) = fail(&mut pipeline, |p| p.measure(request));
    assert_eq!(old_error, new_error);
    assert_ne!(old, new);
    assert!(pipeline.lineage_report(&new).unresolved.is_empty());
    assert_eq!(pipeline.invalid_count(), 2);
}

#[test]
fn invalid_objects_are_rejected_by_all_downstream_construction_apis() {
    let mut pipeline = Pipeline::new(registry());
    let state = fixture::state(&mut pipeline, 10, [12, 6]);
    let (_, members) = members(&pipeline, &state);
    let (invalid, _) = fail(&mut pipeline, |p| {
        p.measure(MeasurementRequest {
            operator: id("missing"),
            inputs: vec![members[0].clone()],
        })
    });
    let (_, error) = fail(&mut pipeline, |p| {
        p.relate(RelationRequest {
            contract: id("ratio"),
            left: invalid.clone(),
            right: members[0].clone(),
        })
    });
    assert_eq!(error, PipelineError::InvalidInputType(invalid.clone()));
    let (_, error) = fail(&mut pipeline, |p| {
        p.compose(CompositionRequest {
            contract: id("composition"),
            members: vec![invalid.clone(), members[0].clone()],
            relations: vec![],
        })
    });
    assert_eq!(error, PipelineError::InvalidInputType(invalid.clone()));
    let (_, error) = fail(&mut pipeline, |p| {
        p.derive_state(StateRequest {
            contract: id("state"),
            composition: invalid.clone(),
            observation_horizon: Some(50),
        })
    });
    assert_eq!(error, PipelineError::InvalidInputType(invalid.clone()));
    let (_, error) = fail(&mut pipeline, |p| {
        p.transition(TransitionRequest {
            contract: id("transition"),
            source: invalid.clone(),
            target: state,
            observation_horizon: Some(50),
        })
    });
    assert_eq!(error, PipelineError::InvalidInputType(invalid));
}

#[test]
fn evaluation_distinguishes_existing_invalid_measurement_from_missing_target() {
    let mut pipeline = Pipeline::new(registry());
    let (invalid, _) = fail(&mut pipeline, |p| {
        p.measure(MeasurementRequest {
            operator: id("read"),
            inputs: vec![ObjectRef::Observation(0)],
        })
    });
    let context = pipeline
        .registry()
        .conformance(&id("check-measurement"))
        .unwrap()
        .context
        .clone();
    let evaluation = pipeline
        .evaluate(EvaluationRequest {
            contract: id("check-measurement"),
            target: invalid.clone(),
            context: context.clone(),
        })
        .unwrap();
    let artifact = pipeline.resolve_evaluation(&evaluation).unwrap();
    assert_eq!(artifact.result(), Some(ConformanceResult::Fail));
    assert_eq!(artifact.status(), LifecycleStatus::Invalid);
    assert!(artifact
        .diagnostics()
        .contains(&EvaluationDiagnostic::TargetInvalid));
    assert!(!artifact
        .diagnostics()
        .contains(&EvaluationDiagnostic::TargetUnresolved));
    assert_eq!(
        artifact.target_basis(),
        Some(
            pipeline
                .resolve_invalid(&invalid)
                .unwrap()
                .canonical_basis()
                .as_slice()
        )
    );
    let mut other = Pipeline::new(registry());
    let missing = other
        .evaluate(EvaluationRequest {
            contract: id("check-measurement"),
            target: invalid,
            context,
        })
        .unwrap();
    assert_eq!(
        other.resolve_evaluation(&missing).unwrap().result(),
        Some(ConformanceResult::Unresolved)
    );
    assert!(other
        .resolve_evaluation(&missing)
        .unwrap()
        .diagnostics()
        .contains(&EvaluationDiagnostic::TargetUnresolved));
}
