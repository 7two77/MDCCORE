//! Production-API coverage of the bounded Relation / Composition slice.
//! This is not the full MDC-E2E-CONFORMANCE-001 gate.

use mdc_core::canonical::snapshot::snapshot;
use mdc_core::contract::evaluation::{ConformanceResult, LifecycleStatus};
use mdc_core::contract::registry::{
    DomainContext, OperatorContract, OperatorKind, Registry, ScalarSchema, SchemaContract,
    SourceContract, VersionedId,
};
use mdc_core::contract::structure::{
    CompositionContract, ConstraintContract, ConstraintKind, MemberOrder, MemberType,
    RelationContext, RelationContract, RelationKind, TemporalScope,
};
use mdc_core::model::composition::CompositionError;
use mdc_core::model::relation::{RelationError, RelationValue};
use mdc_core::pipeline::{
    CompositionRequest, MeasurementRequest, ObservationRequest, Pipeline, PipelineError,
    RejectedRequest, RelationRequest,
};
use mdc_core::provenance::ObjectRef;

type CapturedGraph = Vec<(ObjectRef, u64, Vec<u8>)>;

fn id(name: &str) -> VersionedId {
    VersionedId::new(name, 1).unwrap()
}

fn domain() -> DomainContext {
    DomainContext::new("fixture", id("ticks")).unwrap()
}

fn context() -> RelationContext {
    RelationContext::new(
        domain(),
        TemporalScope::new(0, u64::MAX).unwrap(),
        id("fixture/frame"),
        id("fixture/noncausal"),
    )
}

fn register_relation(
    registry: &mut Registry,
    name: &str,
    kind: RelationKind,
    ctx: RelationContext,
) {
    registry
        .register_relation(RelationContract {
            reference: id(name),
            kind,
            context: ctx,
        })
        .unwrap();
}

fn registry() -> Registry {
    let mut registry = Registry::new();
    registry
        .register_schema(SchemaContract {
            reference: id("i64-le"),
            format: ScalarSchema::SignedI64Le,
        })
        .unwrap();
    for (source, unit) in [("source", "count"), ("price-source", "price")] {
        registry
            .register_source(SourceContract {
                reference: id(source),
                schema: id("i64-le"),
                context: domain(),
                unit: unit.into(),
            })
            .unwrap();
    }
    for (name, kind) in [
        ("read", OperatorKind::Scalar),
        ("difference", OperatorKind::Difference),
    ] {
        registry
            .register_operator(OperatorContract {
                reference: id(name),
                kind,
            })
            .unwrap();
    }
    for (name, kind) in [
        ("before", RelationKind::Precedes),
        ("distance", RelationKind::TemporalDistance),
        (
            "adjacent",
            RelationKind::Adjacent {
                step: 10,
                symmetric: false,
            },
        ),
        (
            "symmetric",
            RelationKind::Adjacent {
                step: 10,
                symmetric: true,
            },
        ),
        ("ratio", RelationKind::Proportion),
    ] {
        register_relation(&mut registry, name, kind, context());
    }
    for (name, kind) in [
        (
            "cardinality",
            ConstraintKind::Cardinality { min: 2, max: 4 },
        ),
        ("units", ConstraintKind::SameUnits),
        ("connected", ConstraintKind::Connected),
        ("acyclic", ConstraintKind::Acyclic),
        (
            "chain",
            ConstraintKind::OrderedChain {
                relation: id("before"),
            },
        ),
        (
            "required",
            ConstraintKind::RequiresRelation {
                relation: id("before"),
            },
        ),
    ] {
        registry
            .register_constraint(ConstraintContract {
                reference: id(name),
                kind,
            })
            .unwrap();
    }
    for (name, order, member_type, constraints) in [
        (
            "chain-composition",
            MemberOrder::Ordered,
            MemberType::Observations,
            vec![
                "cardinality",
                "units",
                "connected",
                "acyclic",
                "chain",
                "required",
            ],
        ),
        (
            "unordered-composition",
            MemberOrder::Unordered,
            MemberType::ScalarInputs,
            vec!["cardinality", "units", "connected", "acyclic"],
        ),
        (
            "ordered-composition",
            MemberOrder::Ordered,
            MemberType::ScalarInputs,
            vec!["cardinality"],
        ),
        (
            "measurement-composition",
            MemberOrder::Unordered,
            MemberType::Measurements,
            vec!["cardinality", "units", "connected", "acyclic"],
        ),
    ] {
        registry
            .register_composition(CompositionContract {
                reference: id(name),
                member_order: order,
                member_type,
                constraints: constraints.into_iter().map(id).collect(),
                context: context(),
            })
            .unwrap();
    }
    registry
}

fn observation(pipeline: &mut Pipeline, timestamp: u64, value: i64) -> ObjectRef {
    observation_from(pipeline, "source", timestamp, value)
}

fn observation_from(
    pipeline: &mut Pipeline,
    source: &str,
    timestamp: u64,
    value: i64,
) -> ObjectRef {
    pipeline
        .admit(ObservationRequest {
            source: id(source),
            timestamp: Some(timestamp),
            payload: value.to_le_bytes().to_vec(),
            acquisition_context: "fixture/source-coordinate".into(),
        })
        .unwrap()
}

fn read(pipeline: &mut Pipeline, reference: &ObjectRef) -> ObjectRef {
    pipeline
        .measure(MeasurementRequest {
            operator: id("read"),
            inputs: vec![reference.clone()],
        })
        .unwrap()
}

fn relation(contract: &str, left: &ObjectRef, right: &ObjectRef) -> RelationRequest {
    RelationRequest {
        contract: id(contract),
        left: left.clone(),
        right: right.clone(),
    }
}

fn composition(
    contract: &str,
    members: Vec<ObjectRef>,
    relations: Vec<ObjectRef>,
) -> CompositionRequest {
    CompositionRequest {
        contract: id(contract),
        members,
        relations,
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
                ObjectRef::Source(_)
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
                    panic!("capture expects O/M/R/C semantic objects")
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

fn counts(pipeline: &Pipeline) -> (usize, usize, usize, usize) {
    (
        pipeline.observation_count(),
        pipeline.measurement_count(),
        pipeline.relation_count(),
        pipeline.composition_count(),
    )
}

fn reject(
    pipeline: &mut Pipeline,
    request: RejectedRequest,
    upstream: &[ObjectRef],
) -> PipelineError {
    let before_counts = counts(pipeline);
    let before_snapshots = capture(pipeline, upstream);
    let before_rejections = pipeline.rejections().len();
    let result = match request.clone() {
        RejectedRequest::Observation(request) => pipeline.admit(request),
        RejectedRequest::Measurement(request) => pipeline.measure(request),
        RejectedRequest::Relation(request) => pipeline.relate(request),
        RejectedRequest::Composition(request) => pipeline.compose(request),
        RejectedRequest::State(_) | RejectedRequest::Transition(_) => {
            panic!("this suite submits only O/M/R/C requests")
        }
    };
    let error = result.unwrap_err();
    assert_eq!(counts(pipeline), before_counts);
    assert_eq!(capture(pipeline, upstream), before_snapshots);
    assert_eq!(pipeline.rejections().len(), before_rejections + 1);
    let retained = pipeline.rejections().last().unwrap();
    assert_eq!(retained.request(), &request);
    assert_eq!(retained.error(), &error);
    assert_eq!(retained.status(), LifecycleStatus::Invalid);
    error
}

fn failed_constraints(error: PipelineError) -> Vec<(VersionedId, ConformanceResult)> {
    match error {
        PipelineError::Composition(CompositionError::FailedConstraints(results)) => results
            .into_iter()
            .map(|result| (result.reference().clone(), result.result()))
            .collect(),
        other => panic!("expected computed constraint failure, got {other:?}"),
    }
}

#[test]
fn full_scalar_slice_reaches_sources_and_preserves_all_upstream_snapshots() {
    let mut pipeline = Pipeline::new(registry());
    let observations = [
        observation(&mut pipeline, 10, 12),
        observation(&mut pipeline, 20, 6),
        observation(&mut pipeline, 30, 3),
    ];
    let measurements = observations
        .iter()
        .map(|input| read(&mut pipeline, input))
        .collect::<Vec<_>>();
    let mut upstream = observations.to_vec();
    upstream.extend(measurements.clone());
    let before_relations = capture(&pipeline, &upstream);
    let first = pipeline
        .relate(relation("ratio", &measurements[0], &measurements[1]))
        .unwrap();
    let second = pipeline
        .relate(relation("ratio", &measurements[1], &measurements[2]))
        .unwrap();
    assert_eq!(capture(&pipeline, &upstream), before_relations);
    upstream.extend([first.clone(), second.clone()]);
    let before_composition = capture(&pipeline, &upstream);
    let composed = pipeline
        .compose(composition(
            "measurement-composition",
            measurements,
            vec![second, first],
        ))
        .unwrap();
    let object = pipeline.resolve_composition(&composed).unwrap();
    assert_eq!(object.status(), LifecycleStatus::Candidate);
    assert_eq!(object.constraints_result(), ConformanceResult::Pass);
    assert!(object
        .constraints()
        .iter()
        .all(|result| result.result() == ConformanceResult::Pass));
    assert_eq!(capture(&pipeline, &upstream), before_composition);
    let mut expected = upstream;
    expected.extend([ObjectRef::Source(id("source")), composed]);
    expected.sort();
    assert_eq!(
        pipeline
            .trace(
                expected
                    .iter()
                    .find(|r| matches!(r, ObjectRef::Composition(_)))
                    .unwrap()
            )
            .unwrap(),
        expected
    );
}

#[test]
fn precedence_and_directed_adjacency_enforce_real_temporal_conditions() {
    let mut pipeline = Pipeline::new(registry());
    let a = observation(&mut pipeline, 10, 1);
    let b = observation(&mut pipeline, 20, 2);
    let equal_time = observation(&mut pipeline, 10, 3);
    let distant = observation(&mut pipeline, 35, 4);
    pipeline.relate(relation("before", &a, &b)).unwrap();
    pipeline.relate(relation("adjacent", &a, &b)).unwrap();
    for (left, right, expected) in [
        (b.clone(), a.clone(), RelationError::TemporalOrder),
        (a.clone(), equal_time, RelationError::TemporalOrder),
    ] {
        assert_eq!(
            reject(
                &mut pipeline,
                RejectedRequest::Relation(relation("before", &left, &right)),
                &[a.clone(), b.clone()]
            ),
            PipelineError::Relation(expected)
        );
    }
    for (left, right) in [(b.clone(), a.clone()), (b, distant)] {
        assert_eq!(
            reject(
                &mut pipeline,
                RejectedRequest::Relation(relation("adjacent", &left, &right)),
                std::slice::from_ref(&a)
            ),
            PipelineError::Relation(RelationError::NotAdjacent)
        );
    }
}

#[test]
fn symmetric_adjacency_deduplicates_endpoints_and_provenance_together() {
    let mut pipeline = Pipeline::new(registry());
    let a = observation(&mut pipeline, 10, 1);
    let b = observation(&mut pipeline, 20, 2);
    let forward = pipeline.relate(relation("symmetric", &a, &b)).unwrap();
    let before = capture(&pipeline, std::slice::from_ref(&forward));
    let reverse = pipeline.relate(relation("symmetric", &b, &a)).unwrap();
    assert_eq!(forward, reverse);
    assert_eq!(pipeline.relation_count(), 1);
    assert_eq!(capture(&pipeline, &[reverse]), before);
    let object = pipeline.resolve_relation(&forward).unwrap();
    assert_eq!(
        object.provenance().inputs(),
        &[object.left_ref().clone(), object.right_ref().clone()]
    );
    assert!(!object.is_directed());
}

#[test]
fn ordered_ratios_retain_exact_operands_without_creating_directed_arcs() {
    let mut pipeline = Pipeline::new(registry());
    let a = observation(&mut pipeline, 10, -7);
    let b = observation(&mut pipeline, 20, 2);
    let ma = read(&mut pipeline, &a);
    let mb = read(&mut pipeline, &b);
    let first = pipeline.relate(relation("ratio", &ma, &mb)).unwrap();
    let second = pipeline.relate(relation("ratio", &mb, &ma)).unwrap();
    assert_ne!(first, second);
    assert_eq!(
        pipeline.resolve_relation(&first).unwrap().value(),
        &RelationValue::Proportion {
            numerator: -7,
            denominator: 2
        }
    );
    assert_eq!(
        pipeline.resolve_relation(&second).unwrap().value(),
        &RelationValue::Proportion {
            numerator: 2,
            denominator: -7
        }
    );
    assert!(!pipeline.resolve_relation(&first).unwrap().is_directed());
    // Ordered numeric operands alone must not make these two ratios a DAG cycle.
    pipeline
        .compose(composition(
            "measurement-composition",
            vec![ma, mb],
            vec![first, second],
        ))
        .unwrap();
}

#[test]
fn extreme_temporal_distances_are_signed_without_overflow() {
    let mut pipeline = Pipeline::new(registry());
    let first = observation(&mut pipeline, 0, 1);
    let last = observation(&mut pipeline, u64::MAX, 2);
    let forward = pipeline
        .relate(relation("distance", &first, &last))
        .unwrap();
    let reverse = pipeline
        .relate(relation("distance", &last, &first))
        .unwrap();
    assert_eq!(
        pipeline.resolve_relation(&forward).unwrap().value(),
        &RelationValue::TemporalDistance(i128::from(u64::MAX))
    );
    assert_eq!(
        pipeline.resolve_relation(&reverse).unwrap().value(),
        &RelationValue::TemporalDistance(-i128::from(u64::MAX))
    );
}

#[test]
fn relation_kinds_reject_wrong_types_zero_denominators_and_incompatible_units() {
    let mut pipeline = Pipeline::new(registry());
    let a = observation(&mut pipeline, 10, 7);
    let zero = observation(&mut pipeline, 20, 0);
    let price = observation_from(&mut pipeline, "price-source", 20, 5);
    let ma = read(&mut pipeline, &a);
    let mz = read(&mut pipeline, &zero);
    let mp = read(&mut pipeline, &price);
    assert_eq!(
        reject(
            &mut pipeline,
            RejectedRequest::Relation(relation("before", &ma, &mz)),
            &[a.clone(), ma.clone()]
        ),
        PipelineError::Relation(RelationError::InvalidInputType(ma.clone()))
    );
    assert_eq!(
        reject(
            &mut pipeline,
            RejectedRequest::Relation(relation("ratio", &a, &zero)),
            &[a.clone(), ma.clone()]
        ),
        PipelineError::Relation(RelationError::InvalidInputType(a))
    );
    assert_eq!(
        reject(
            &mut pipeline,
            RejectedRequest::Relation(relation("ratio", &ma, &mz)),
            std::slice::from_ref(&ma)
        ),
        PipelineError::Relation(RelationError::ZeroDenominator)
    );
    assert_eq!(
        reject(
            &mut pipeline,
            RejectedRequest::Relation(relation("ratio", &ma, &mp)),
            std::slice::from_ref(&ma)
        ),
        PipelineError::Relation(RelationError::IncompatibleUnits)
    );
}

#[test]
fn relation_scope_covers_every_source_observation_of_prior_measurements() {
    let mut registry = registry();
    let narrow = RelationContext::new(
        domain(),
        TemporalScope::new(10, 20).unwrap(),
        id("fixture/frame"),
        id("fixture/noncausal"),
    );
    register_relation(
        &mut registry,
        "narrow-ratio",
        RelationKind::Proportion,
        narrow,
    );
    let mut pipeline = Pipeline::new(registry);
    let outside = observation(&mut pipeline, 0, 8);
    let inside = observation(&mut pipeline, 15, 2);
    let derived = pipeline
        .measure(MeasurementRequest {
            operator: id("difference"),
            inputs: vec![outside.clone(), inside.clone()],
        })
        .unwrap();
    let local = read(&mut pipeline, &inside);
    assert_eq!(
        reject(
            &mut pipeline,
            RejectedRequest::Relation(relation("narrow-ratio", &derived, &local)),
            &[outside, inside, derived.clone(), local.clone()]
        ),
        PipelineError::Relation(RelationError::OutsideTemporalScope)
    );
}

#[test]
fn missing_contract_versions_and_references_never_fall_back_or_synthesize() {
    let mut pipeline = Pipeline::new(registry());
    let a = observation(&mut pipeline, 10, 7);
    let b = observation(&mut pipeline, 20, 3);
    for contract in [id("missing"), VersionedId::new("before", 2).unwrap()] {
        let mut request = relation("before", &a, &b);
        request.contract = contract.clone();
        assert_eq!(
            reject(
                &mut pipeline,
                RejectedRequest::Relation(request),
                &[a.clone(), b.clone()]
            ),
            PipelineError::RelationContractUnresolved(contract)
        );
    }
    let missing = ObjectRef::Observation(0);
    assert_eq!(
        reject(
            &mut pipeline,
            RejectedRequest::Relation(relation("before", &a, &missing)),
            &[a.clone(), b.clone()]
        ),
        PipelineError::InputUnresolved(missing)
    );
    for contract in [
        id("missing"),
        VersionedId::new("unordered-composition", 2).unwrap(),
    ] {
        let mut request = composition("unordered-composition", vec![a.clone(), b.clone()], vec![]);
        request.contract = contract.clone();
        assert_eq!(
            reject(
                &mut pipeline,
                RejectedRequest::Composition(request),
                &[a.clone(), b.clone()]
            ),
            PipelineError::CompositionContractUnresolved(contract)
        );
    }
    assert!(pipeline.trace(&ObjectRef::Relation(0)).is_err());
    assert!(pipeline.trace(&ObjectRef::Composition(0)).is_err());
    assert_eq!(pipeline.relation_count(), 0);
    assert_eq!(pipeline.composition_count(), 0);
}

#[test]
fn composition_requires_direct_membership_not_merely_transitive_lineage() {
    let mut pipeline = Pipeline::new(registry());
    let a = observation(&mut pipeline, 10, 7);
    let b = observation(&mut pipeline, 20, 3);
    let ma = read(&mut pipeline, &a);
    let mb = read(&mut pipeline, &b);
    let edge = pipeline.relate(relation("before", &a, &b)).unwrap();
    let error = reject(
        &mut pipeline,
        RejectedRequest::Composition(composition(
            "unordered-composition",
            vec![ma.clone(), mb.clone()],
            vec![edge.clone()],
        )),
        &[a.clone(), b.clone(), ma, mb, edge],
    );
    assert!(
        matches!(error, PipelineError::Composition(CompositionError::ExternalEndpoint(endpoint)) if endpoint == a || endpoint == b)
    );
}

#[test]
fn composition_context_includes_frame_causal_descriptor_and_scope() {
    let mut registry = registry();
    let contexts = [
        RelationContext::new(
            domain(),
            TemporalScope::new(0, u64::MAX).unwrap(),
            id("other-frame"),
            id("fixture/noncausal"),
        ),
        RelationContext::new(
            domain(),
            TemporalScope::new(0, u64::MAX).unwrap(),
            id("fixture/frame"),
            id("other-causal-descriptor"),
        ),
        RelationContext::new(
            domain(),
            TemporalScope::new(0, 100).unwrap(),
            id("fixture/frame"),
            id("fixture/noncausal"),
        ),
    ];
    for (index, ctx) in contexts.into_iter().enumerate() {
        register_relation(
            &mut registry,
            &format!("context-{index}"),
            RelationKind::TemporalDistance,
            ctx,
        );
    }
    let mut pipeline = Pipeline::new(registry);
    let a = observation(&mut pipeline, 10, 7);
    let b = observation(&mut pipeline, 20, 3);
    for index in 0..3 {
        let edge = pipeline
            .relate(relation(&format!("context-{index}"), &a, &b))
            .unwrap();
        assert_eq!(
            reject(
                &mut pipeline,
                RejectedRequest::Composition(composition(
                    "unordered-composition",
                    vec![a.clone(), b.clone()],
                    vec![edge.clone()]
                )),
                &[a.clone(), b.clone(), edge.clone()]
            ),
            PipelineError::Composition(CompositionError::RelationContextMismatch(edge))
        );
    }
}

#[test]
fn connected_constraint_detects_an_isolated_declared_member() {
    let mut pipeline = Pipeline::new(registry());
    let a = observation(&mut pipeline, 10, 7);
    let b = observation(&mut pipeline, 20, 3);
    let c = observation(&mut pipeline, 30, 2);
    let edge = pipeline.relate(relation("before", &a, &b)).unwrap();
    let results = failed_constraints(reject(
        &mut pipeline,
        RejectedRequest::Composition(composition(
            "unordered-composition",
            vec![a.clone(), b.clone(), c.clone()],
            vec![edge.clone()],
        )),
        &[a, b, c, edge],
    ));
    assert!(results.contains(&(id("connected"), ConformanceResult::Fail)));
    assert!(results.contains(&(id("cardinality"), ConformanceResult::Pass)));
    assert_eq!(results.len(), 4);
}

#[test]
fn acyclic_constraint_detects_a_cycle_of_admissible_directed_relations() {
    let mut pipeline = Pipeline::new(registry());
    let a = observation(&mut pipeline, 10, 7);
    let b = observation(&mut pipeline, 20, 3);
    let forward = pipeline.relate(relation("distance", &a, &b)).unwrap();
    let reverse = pipeline.relate(relation("distance", &b, &a)).unwrap();
    let results = failed_constraints(reject(
        &mut pipeline,
        RejectedRequest::Composition(composition(
            "unordered-composition",
            vec![a.clone(), b.clone()],
            vec![forward.clone(), reverse.clone()],
        )),
        &[a, b, forward, reverse],
    ));
    assert!(results.contains(&(id("acyclic"), ConformanceResult::Fail)));
    assert!(results.contains(&(id("connected"), ConformanceResult::Pass)));
}

#[test]
fn parallel_directed_edges_do_not_create_a_false_cycle() {
    let mut pipeline = Pipeline::new(registry());
    let a = observation(&mut pipeline, 10, 7);
    let b = observation(&mut pipeline, 20, 3);
    let before = pipeline.relate(relation("before", &a, &b)).unwrap();
    let distance = pipeline.relate(relation("distance", &a, &b)).unwrap();
    let composed = pipeline
        .compose(composition(
            "unordered-composition",
            vec![a, b],
            vec![before, distance],
        ))
        .unwrap();
    let object = pipeline.resolve_composition(&composed).unwrap();
    assert!(object
        .constraints()
        .iter()
        .any(|result| result.reference() == &id("acyclic")
            && result.result() == ConformanceResult::Pass));
}

#[test]
fn ordered_chain_requires_the_declared_relation_contract_for_each_pair() {
    let mut pipeline = Pipeline::new(registry());
    let a = observation(&mut pipeline, 10, 7);
    let b = observation(&mut pipeline, 20, 3);
    let c = observation(&mut pipeline, 30, 2);
    let ab = pipeline.relate(relation("before", &a, &b)).unwrap();
    let bc = pipeline.relate(relation("before", &b, &c)).unwrap();
    let valid = pipeline
        .compose(composition(
            "chain-composition",
            vec![a.clone(), b.clone(), c.clone()],
            vec![bc.clone(), ab.clone()],
        ))
        .unwrap();
    let upstream = [a.clone(), b.clone(), c.clone(), ab, bc, valid];
    let wrong_ab = pipeline.relate(relation("distance", &a, &b)).unwrap();
    let wrong_bc = pipeline.relate(relation("distance", &b, &c)).unwrap();
    let results = failed_constraints(reject(
        &mut pipeline,
        RejectedRequest::Composition(composition(
            "chain-composition",
            vec![a, b, c],
            vec![wrong_ab, wrong_bc],
        )),
        &upstream,
    ));
    assert!(results.contains(&(id("chain"), ConformanceResult::Fail)));
    assert!(results.contains(&(id("required"), ConformanceResult::Fail)));
}

#[test]
fn unordered_compositions_normalize_members_relations_results_and_provenance() {
    fn run(reverse: bool) -> (CapturedGraph, Vec<ObjectRef>) {
        let mut pipeline = Pipeline::new(registry());
        let (a, b, c) = if reverse {
            let c = observation(&mut pipeline, 30, 2);
            let b = observation(&mut pipeline, 20, 3);
            let a = observation(&mut pipeline, 10, 7);
            (a, b, c)
        } else {
            (
                observation(&mut pipeline, 10, 7),
                observation(&mut pipeline, 20, 3),
                observation(&mut pipeline, 30, 2),
            )
        };
        let ab = pipeline.relate(relation("before", &a, &b)).unwrap();
        let bc = pipeline.relate(relation("before", &b, &c)).unwrap();
        let (members, relations) = if reverse {
            (
                vec![c.clone(), a.clone(), b.clone()],
                vec![bc.clone(), ab.clone()],
            )
        } else {
            (
                vec![a.clone(), b.clone(), c.clone()],
                vec![ab.clone(), bc.clone()],
            )
        };
        let request = composition("unordered-composition", members, relations);
        let composed = pipeline.compose(request.clone()).unwrap();
        assert_eq!(pipeline.compose(request).unwrap(), composed);
        assert_eq!(pipeline.composition_count(), 1);
        let object = pipeline.resolve_composition(&composed).unwrap();
        let mut expected_inputs = object.members().to_vec();
        expected_inputs.extend_from_slice(object.relations());
        assert_eq!(object.provenance().inputs(), expected_inputs);
        let mut objects = vec![a, b, c, ab, bc, composed.clone()];
        objects.sort();
        (
            capture(&pipeline, &objects),
            pipeline.trace(&composed).unwrap(),
        )
    }
    assert_eq!(run(false), run(true));
}

#[test]
fn ordered_member_permutation_changes_content_when_the_rule_preserves_order() {
    let mut pipeline = Pipeline::new(registry());
    let a = observation(&mut pipeline, 10, 7);
    let b = observation(&mut pipeline, 20, 3);
    let first = pipeline
        .compose(composition(
            "ordered-composition",
            vec![a.clone(), b.clone()],
            vec![],
        ))
        .unwrap();
    let second = pipeline
        .compose(composition("ordered-composition", vec![b, a], vec![]))
        .unwrap();
    assert_ne!(first, second);
    assert_ne!(capture(&pipeline, &[first]), capture(&pipeline, &[second]));
}

#[test]
fn duplicate_or_wrongly_typed_composition_inputs_are_rejected_atomically() {
    let mut pipeline = Pipeline::new(registry());
    let a = observation(&mut pipeline, 10, 7);
    let b = observation(&mut pipeline, 20, 3);
    let edge = pipeline.relate(relation("before", &a, &b)).unwrap();
    let upstream = [a.clone(), b.clone(), edge.clone()];
    assert_eq!(
        reject(
            &mut pipeline,
            RejectedRequest::Composition(composition(
                "ordered-composition",
                vec![a.clone(), a.clone()],
                vec![]
            )),
            &upstream
        ),
        PipelineError::Composition(CompositionError::DuplicateMember(a.clone()))
    );
    assert_eq!(
        reject(
            &mut pipeline,
            RejectedRequest::Composition(composition(
                "unordered-composition",
                vec![a.clone(), b.clone()],
                vec![edge.clone(), edge.clone()]
            )),
            &upstream
        ),
        PipelineError::Composition(CompositionError::DuplicateRelation(edge.clone()))
    );
    assert_eq!(
        reject(
            &mut pipeline,
            RejectedRequest::Composition(composition(
                "measurement-composition",
                vec![a.clone(), b.clone()],
                vec![edge.clone()]
            )),
            &upstream
        ),
        PipelineError::Composition(CompositionError::InvalidMemberType(a))
    );
    let missing = ObjectRef::Relation(0);
    assert_eq!(
        reject(
            &mut pipeline,
            RejectedRequest::Composition(composition(
                "unordered-composition",
                vec![upstream[0].clone(), b],
                vec![missing.clone()]
            )),
            &upstream
        ),
        PipelineError::InputUnresolved(missing)
    );
    assert_eq!(
        reject(
            &mut pipeline,
            RejectedRequest::Composition(composition(
                "ordered-composition",
                vec![edge.clone(), upstream[0].clone()],
                vec![]
            )),
            &upstream
        ),
        PipelineError::InvalidInputType(edge)
    );
}

#[test]
fn raw_legacy_relation_ids_do_not_bypass_the_admitted_store() {
    let mut pipeline = Pipeline::new(registry());
    let a = observation(&mut pipeline, 10, 7);
    let b = observation(&mut pipeline, 20, 3);
    let raw = mdc_core::model::relation::Relation {
        id: 42,
        kind: "PRECEDES",
        left_ref: 1,
        right_ref: 2,
    };
    let reference = ObjectRef::Relation(raw.id);
    assert_eq!(
        reject(
            &mut pipeline,
            RejectedRequest::Composition(composition(
                "unordered-composition",
                vec![a.clone(), b.clone()],
                vec![reference.clone()]
            )),
            &[a, b]
        ),
        PipelineError::InputUnresolved(reference)
    );
}
