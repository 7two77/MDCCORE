//! Shared recorded-history fixture. Every semantic operation uses the core API.
#[allow(dead_code)]
#[path = "../lifecycle.rs"]
mod foundation;

use mdc_core::contract::conformance::*;
use mdc_core::contract::dynamics::*;
use mdc_core::contract::evaluation::{ConformanceResult as R, LifecycleStatus as S};
use mdc_core::contract::lifecycle::InvalidationReason;
use mdc_core::contract::registry::*;
use mdc_core::contract::structure::*;
use mdc_core::identity::{CanonicalSemanticBasis, IdentityProtocol};
use mdc_core::model::lifecycle::{InvalidationRequest, LifecycleEventId};
use mdc_core::pipeline::*;
use mdc_core::provenance::ObjectRef;
use std::collections::BTreeSet;

pub use foundation::id;

pub fn version(name: &str, version: u32) -> VersionedId {
    VersionedId::new(name, version).unwrap()
}

/// Includes definitions unused by the initial graph, exact version 2 entries,
/// and every currently supported relation/constraint/condition encoding branch.
pub fn registry() -> Registry {
    let mut r = foundation::registry();
    let frame = r.relation(&id("ratio")).unwrap().context.clone();
    r.register_schema(SchemaContract {
        reference: version("unused/schema", 2),
        format: ScalarSchema::SignedI64Le,
    })
    .unwrap();
    let mut source = r.source(&id("source")).unwrap().clone();
    source.reference = version("unused/source", 2);
    source.schema = version("unused/schema", 2);
    r.register_source(source).unwrap();
    for (name, kind) in [
        ("unused/read", OperatorKind::Scalar),
        ("unused/difference", OperatorKind::Difference),
    ] {
        r.register_operator(OperatorContract {
            reference: version(name, 2),
            kind,
        })
        .unwrap();
    }
    for (name, kind) in [
        ("unused/precedes", RelationKind::Precedes),
        ("unused/distance", RelationKind::TemporalDistance),
        (
            "unused/directed",
            RelationKind::Adjacent {
                step: 10,
                symmetric: false,
            },
        ),
        (
            "unused/symmetric",
            RelationKind::Adjacent {
                step: 10,
                symmetric: true,
            },
        ),
    ] {
        r.register_relation(RelationContract {
            reference: id(name),
            kind,
            context: frame.clone(),
        })
        .unwrap();
    }
    for (name, kind) in [
        ("unused/units", ConstraintKind::SameUnits),
        ("unused/acyclic", ConstraintKind::Acyclic),
        (
            "unused/chain",
            ConstraintKind::OrderedChain {
                relation: id("unused/precedes"),
            },
        ),
        (
            "unused/requires",
            ConstraintKind::RequiresRelation {
                relation: id("ratio"),
            },
        ),
    ] {
        r.register_constraint(ConstraintContract {
            reference: id(name),
            kind,
        })
        .unwrap();
    }
    r.register_composition(CompositionContract {
        reference: id("unused/ordered"),
        member_order: MemberOrder::Ordered,
        member_type: MemberType::Observations,
        constraints: vec![id("cardinality"), id("unused/chain")],
        context: frame.clone(),
    })
    .unwrap();
    r.register_composition(CompositionContract {
        reference: id("unused/scalar"),
        member_order: MemberOrder::Unordered,
        member_type: MemberType::ScalarInputs,
        constraints: vec![id("cardinality"), id("unused/units")],
        context: frame,
    })
    .unwrap();
    r.register_state_criterion(StateCriterionContract {
        reference: version("unused/cardinality", 2),
        kind: StateCriterionKind::Cardinality { min: 1, max: 4 },
    })
    .unwrap();
    let mut state = r.state(&id("state")).unwrap().clone();
    state.reference = version("unused/state", 2);
    state.composition = id("unused/scalar");
    state.criteria = vec![version("unused/cardinality", 2)];
    state.criterion_order = CriterionOrder::Ordered;
    r.register_state(state).unwrap();
    r.register_transition_condition(TransitionConditionContract {
        reference: id("unused/witness"),
        kind: TransitionConditionKind::ObservationAfterTarget {
            source: id("confirmation"),
        },
    })
    .unwrap();
    let mut transition = r.transition(&id("transition")).unwrap().clone();
    transition.reference = version("unused/transition", 2);
    transition.condition_order = CriterionOrder::Ordered;
    transition.conditions.push(id("unused/witness"));
    r.register_transition(transition).unwrap();
    r.register_conformance_criterion(ConformanceCriterionContract {
        reference: id("snapshot/three"),
        kind: ConformanceCriterionKind::MemberCount { min: 3, max: 3 },
    })
    .unwrap();
    let mut fail = r.conformance(&id("check/C")).unwrap().clone();
    fail.reference = id("snapshot/fail/C");
    fail.criteria.push(id("snapshot/three"));
    r.register_conformance(fail).unwrap();
    let mut missing = r.conformance(&id("check/C")).unwrap().clone();
    missing.reference = version("unused/check/C", 2);
    missing.missing_target = MissingTargetPolicy::Fail;
    missing.criterion_order = CriterionOrder::Ordered;
    r.register_conformance(missing).unwrap();
    let mut projection = r.projection(&id("project/C")).unwrap().clone();
    projection.reference = version("unused/project/C", 2);
    projection.conformance = version("unused/check/C", 2);
    r.register_projection(projection).unwrap();
    let mut lifecycle = r.lifecycle(&id("lifecycle/O")).unwrap().clone();
    lifecycle.reference = version("unused/revoke/O", 2);
    r.register_lifecycle(lifecycle).unwrap();
    r
}

pub fn observation(timestamp: u64, value: i64) -> ObservationRequest {
    ObservationRequest {
        source: id("source"),
        timestamp: Some(timestamp),
        payload: value.to_le_bytes().to_vec(),
        acquisition_context: format!("snapshot/record/{timestamp}/{value}"),
    }
}

pub struct StateRefs {
    pub observations: [ObjectRef; 2],
    pub measurements: [ObjectRef; 2],
    pub relation: ObjectRef,
    pub composition: ObjectRef,
    pub state: ObjectRef,
}

pub fn state(s: &mut SnapshotSession, first: u64, values: [i64; 2]) -> StateRefs {
    let a = s.admit(observation(first, values[0])).unwrap();
    let b = s.admit(observation(first + 10, values[1])).unwrap();
    let ma = s
        .measure(MeasurementRequest {
            operator: id("read"),
            inputs: vec![a.clone()],
        })
        .unwrap();
    let mb = s
        .measure(MeasurementRequest {
            operator: id("read"),
            inputs: vec![b.clone()],
        })
        .unwrap();
    let relation = s
        .relate(RelationRequest {
            contract: id("ratio"),
            left: ma.clone(),
            right: mb.clone(),
        })
        .unwrap();
    let composition = s
        .compose(CompositionRequest {
            contract: id("composition"),
            members: vec![ma.clone(), mb.clone()],
            relations: vec![relation.clone()],
        })
        .unwrap();
    let state = s
        .derive_state(StateRequest {
            contract: id("state"),
            composition: composition.clone(),
            observation_horizon: Some(50),
        })
        .unwrap();
    StateRefs {
        observations: [a, b],
        measurements: [ma, mb],
        relation,
        composition,
        state,
    }
}

pub struct Graph {
    pub session: SnapshotSession,
    pub roots: [ObjectRef; 6],
    pub second_observation: ObjectRef,
    pub second_state: ObjectRef,
    pub witness: ObjectRef,
    pub evaluation: ObjectRef,
}

pub fn evaluation_request(p: &Pipeline, kind: &str, target: &ObjectRef) -> EvaluationRequest {
    let contract = id(&format!("check/{kind}"));
    EvaluationRequest {
        context: p.registry().conformance(&contract).unwrap().context.clone(),
        contract,
        target: target.clone(),
    }
}

pub fn graph(protocol: IdentityProtocol) -> Graph {
    let mut session = SnapshotSession::new(registry(), protocol);
    let first = state(&mut session, 10, [12, 6]);
    let second = state(&mut session, 30, [8, 4]);
    let transition = session
        .transition(TransitionRequest {
            contract: id("transition"),
            source: first.state.clone(),
            target: second.state.clone(),
            observation_horizon: Some(50),
        })
        .unwrap();
    let request = evaluation_request(session.pipeline(), "T", &transition);
    let evaluation = session.evaluate(request).unwrap();
    let witness = session
        .admit(ObservationRequest {
            source: id("confirmation"),
            timestamp: Some(90),
            payload: 1_i64.to_le_bytes().to_vec(),
            acquisition_context: "snapshot/witness/90".into(),
        })
        .unwrap();
    Graph {
        session,
        roots: [
            first.observations[0].clone(),
            first.measurements[0].clone(),
            first.relation,
            first.composition,
            first.state,
            transition,
        ],
        second_observation: second.observations[0].clone(),
        second_state: second.state,
        witness,
        evaluation,
    }
}

pub fn revocation(
    p: &Pipeline,
    name: &str,
    target: &ObjectRef,
    witness: &ObjectRef,
) -> InvalidationRequest {
    InvalidationRequest {
        request_id: name.into(),
        contract: id("lifecycle/O"),
        target: target.clone(),
        reason: InvalidationReason::EvidenceWithdrawn,
        evidence: vec![witness.clone()],
        expected_revision: p.lifecycle_revision(),
    }
}

pub struct HistoryRefs {
    pub pending_state: ObjectRef,
    pub unresolved_evaluation: ObjectRef,
    pub failed_evaluation: ObjectRef,
    pub unknown_evaluation: ObjectRef,
    pub old_missing: ObjectRef,
    pub late_input: ObjectRef,
    pub first_request: InvalidationRequest,
    pub first_event: LifecycleEventId,
    pub second_event: LifecycleEventId,
    pub born_late: ObjectRef,
}

/// All nine mutating APIs, failures/idempotents, captured late inputs, and epochs.
pub fn enrich(g: &mut Graph) -> HistoryRefs {
    let pending_state = g
        .session
        .derive_state(StateRequest {
            contract: id("pending"),
            composition: g.roots[3].clone(),
            observation_horizon: Some(50),
        })
        .unwrap();
    let req = evaluation_request(g.session.pipeline(), "S", &pending_state);
    let unresolved_evaluation = g.session.evaluate_keyed(req).unwrap();
    let context = g
        .session
        .pipeline()
        .registry()
        .conformance(&id("check/C"))
        .unwrap()
        .context
        .clone();
    let failed_evaluation = g
        .session
        .evaluate_keyed(EvaluationRequest {
            contract: id("snapshot/fail/C"),
            target: g.roots[3].clone(),
            context: context.clone(),
        })
        .unwrap();
    let unknown_evaluation = g
        .session
        .evaluate_keyed(EvaluationRequest {
            contract: id("missing/snapshot/check"),
            target: g.roots[3].clone(),
            context,
        })
        .unwrap();
    assert_eq!(
        g.session
            .pipeline()
            .resolve_evaluation(&unresolved_evaluation)
            .unwrap()
            .result(),
        Some(R::Unresolved)
    );
    assert_eq!(
        g.session
            .pipeline()
            .resolve_evaluation(&failed_evaluation)
            .unwrap()
            .result(),
        Some(R::Fail)
    );
    assert_eq!(
        g.session
            .pipeline()
            .resolve_evaluation(&unknown_evaluation)
            .unwrap()
            .result(),
        None
    );

    let missing = id("missing/snapshot/definition");
    assert!(g
        .session
        .admit(ObservationRequest {
            source: missing.clone(),
            timestamp: Some(70),
            payload: vec![0],
            acquisition_context: "missing/source".into()
        })
        .is_err());
    assert!(g
        .session
        .measure(MeasurementRequest {
            operator: missing.clone(),
            inputs: vec![g.roots[0].clone()]
        })
        .is_err());
    assert!(g
        .session
        .relate(RelationRequest {
            contract: missing.clone(),
            left: g.roots[0].clone(),
            right: g.roots[1].clone()
        })
        .is_err());
    assert!(g
        .session
        .compose(CompositionRequest {
            contract: missing.clone(),
            members: vec![g.roots[0].clone()],
            relations: vec![]
        })
        .is_err());
    assert!(g
        .session
        .derive_state(StateRequest {
            contract: missing.clone(),
            composition: g.roots[3].clone(),
            observation_horizon: Some(50)
        })
        .is_err());
    assert!(g
        .session
        .transition(TransitionRequest {
            contract: missing.clone(),
            source: g.roots[4].clone(),
            target: g.second_state.clone(),
            observation_horizon: Some(50)
        })
        .is_err());

    let future_request = observation(80, 11);
    let mut predictor = SnapshotSession::new(registry(), g.session.pipeline().identity_protocol());
    let late_input = predictor.admit(future_request.clone()).unwrap();
    assert!(g
        .session
        .measure(MeasurementRequest {
            operator: id("read"),
            inputs: vec![late_input.clone()]
        })
        .is_err());
    let old_missing = g
        .session
        .pipeline()
        .rejections()
        .last()
        .unwrap()
        .invalid_reference()
        .clone();
    let old_report = g.session.pipeline().lineage_report(&old_missing);
    assert!(old_report.unresolved.contains(&late_input));
    assert_eq!(g.session.admit(future_request).unwrap(), late_input);
    assert_eq!(
        g.session.pipeline().lineage_report(&old_missing),
        old_report
    );

    let repeated = MeasurementRequest {
        operator: missing,
        inputs: vec![g.roots[0].clone()],
    };
    let before = g.session.pipeline().invalid_count();
    let audit = g.session.pipeline().rejections().len();
    assert!(g.session.measure(repeated.clone()).is_err());
    assert!(g.session.measure(repeated).is_err());
    assert_eq!(g.session.pipeline().invalid_count(), before);
    assert_eq!(g.session.pipeline().rejections().len(), audit + 2);

    let first_request = revocation(
        g.session.pipeline(),
        "snapshot/withdraw/1",
        &g.roots[0],
        &g.witness,
    );
    let first_event = g.session.invalidate(first_request.clone()).unwrap();
    let req = evaluation_request(g.session.pipeline(), "T", &g.roots[5]);
    let reassessment = g.session.evaluate_keyed(req).unwrap();
    assert_eq!(
        g.session
            .pipeline()
            .resolve_evaluation(&reassessment)
            .unwrap()
            .result(),
        Some(R::Fail)
    );
    assert_eq!(
        g.session.invalidate(first_request.clone()).unwrap(),
        first_event
    );
    let mut conflict = first_request.clone();
    conflict.reason = InvalidationReason::SourceContractViolation;
    assert!(g.session.invalidate(conflict).is_err());
    let mut stale = revocation(
        g.session.pipeline(),
        "snapshot/stale",
        &g.second_observation,
        &g.witness,
    );
    stale.expected_revision = 0;
    assert!(g.session.invalidate(stale).is_err());
    let second = revocation(
        g.session.pipeline(),
        "snapshot/withdraw/2",
        &g.second_observation,
        &g.witness,
    );
    let second_event = g.session.invalidate(second).unwrap();
    let born_late = g.session.admit(observation(95, 99)).unwrap();
    assert_eq!(g.session.pipeline().lifecycle_revision(), 2);
    assert_eq!(
        g.session
            .pipeline()
            .resolve_evaluation(&g.evaluation)
            .unwrap()
            .result(),
        Some(R::Pass)
    );
    assert_eq!(
        g.session
            .pipeline()
            .lifecycle_view(&g.evaluation)
            .unwrap()
            .status,
        S::Invalid
    );
    HistoryRefs {
        pending_state,
        unresolved_evaluation,
        failed_evaluation,
        unknown_evaluation,
        old_missing,
        late_input,
        first_request,
        first_event,
        second_event,
        born_late,
    }
}

pub fn basis(p: &Pipeline, reference: &ObjectRef) -> Vec<u8> {
    match reference {
        ObjectRef::Observation(_) => p.resolve_observation(reference).unwrap().canonical_basis(),
        ObjectRef::Measurement(_) => p.resolve_measurement(reference).unwrap().canonical_basis(),
        ObjectRef::Relation(_) => p.resolve_relation(reference).unwrap().canonical_basis(),
        ObjectRef::Composition(_) => p.resolve_composition(reference).unwrap().canonical_basis(),
        ObjectRef::State(_) => p.resolve_state(reference).unwrap().canonical_basis(),
        ObjectRef::Transition(_) => p.resolve_transition(reference).unwrap().canonical_basis(),
        ObjectRef::Evaluation(_) | ObjectRef::EvaluationKey(_) => {
            p.resolve_evaluation(reference).unwrap().canonical_basis()
        }
        ObjectRef::Source(_) => panic!("sources are Registry definitions"),
        _ => p.resolve_invalid(reference).unwrap().canonical_basis(),
    }
}

/// Independent public oracle, including fields that are outside canonical body.
pub fn assert_equivalent(a: &SnapshotSession, b: &SnapshotSession) {
    assert_eq!(a.operation_count(), b.operation_count());
    let left = a.pipeline();
    let right = b.pipeline();
    assert_eq!(left.identity_protocol(), right.identity_protocol());
    let refs = a.inventory();
    assert_eq!(refs, b.inventory());
    assert_eq!(left.rejections(), right.rejections());
    assert_eq!(left.lifecycle_events(), right.lifecycle_events());
    assert_eq!(left.lifecycle_rejections(), right.lifecycle_rejections());
    assert_eq!(left.lifecycle_revision(), right.lifecycle_revision());
    assert_eq!(left.evaluation_references(), right.evaluation_references());
    let mut digests = BTreeSet::new();
    for reference in &refs {
        assert_eq!(basis(left, reference), basis(right, reference));
        assert_eq!(left.trace(reference), right.trace(reference));
        assert_eq!(
            left.lineage_report(reference),
            right.lineage_report(reference)
        );
        for epoch in 0..=left.lifecycle_revision() + 1 {
            assert_eq!(
                left.lifecycle_view_at(reference, epoch),
                right.lifecycle_view_at(reference, epoch)
            );
        }
        match reference {
            ObjectRef::Observation(_) => assert_eq!(
                left.resolve_observation(reference),
                right.resolve_observation(reference)
            ),
            ObjectRef::Measurement(_) => assert_eq!(
                left.resolve_measurement(reference),
                right.resolve_measurement(reference)
            ),
            ObjectRef::Relation(_) => assert_eq!(
                left.resolve_relation(reference),
                right.resolve_relation(reference)
            ),
            ObjectRef::Composition(_) => assert_eq!(
                left.resolve_composition(reference),
                right.resolve_composition(reference)
            ),
            ObjectRef::State(_) => assert_eq!(
                left.resolve_state(reference),
                right.resolve_state(reference)
            ),
            ObjectRef::Transition(_) => assert_eq!(
                left.resolve_transition(reference),
                right.resolve_transition(reference)
            ),
            ObjectRef::Evaluation(_) | ObjectRef::EvaluationKey(_) => {
                let artifact = left.resolve_evaluation(reference).unwrap();
                digests.insert(artifact.id());
                assert_eq!(Some(artifact), right.resolve_evaluation(reference));
                assert_eq!(
                    left.canonical_evaluation_reference(reference),
                    right.canonical_evaluation_reference(reference)
                );
            }
            ObjectRef::Source(_) => unreachable!(),
            _ => assert_eq!(
                left.resolve_invalid(reference),
                right.resolve_invalid(reference)
            ),
        }
    }
    for digest in digests {
        assert_eq!(
            left.evaluations_with_digest(digest),
            right.evaluations_with_digest(digest)
        );
        let alias = ObjectRef::Evaluation(digest);
        assert_eq!(
            left.resolve_evaluation(&alias),
            right.resolve_evaluation(&alias)
        );
        assert_eq!(left.lifecycle_view(&alias), right.lifecycle_view(&alias));
    }
    assert_registry_equal(left.registry(), right.registry());
}

pub fn assert_registry_equal(a: &Registry, b: &Registry) {
    macro_rules! entries {
        ($method:ident, [$($name:expr),* $(,)?]) => {
            $(assert_eq!(a.$method(&$name), b.$method(&$name), stringify!($method));)*
        }
    }
    entries!(schema, [id("schema"), version("unused/schema", 2)]);
    entries!(
        source,
        [
            id("source"),
            id("confirmation"),
            version("unused/source", 2)
        ]
    );
    entries!(
        operator,
        [
            id("read"),
            version("unused/read", 2),
            version("unused/difference", 2)
        ]
    );
    entries!(
        relation,
        [
            id("ratio"),
            id("unused/precedes"),
            id("unused/distance"),
            id("unused/directed"),
            id("unused/symmetric")
        ]
    );
    entries!(
        constraint,
        [
            id("cardinality"),
            id("connected"),
            id("unused/units"),
            id("unused/acyclic"),
            id("unused/chain"),
            id("unused/requires")
        ]
    );
    entries!(
        composition,
        [id("composition"), id("unused/ordered"), id("unused/scalar")]
    );
    entries!(
        state_criterion,
        [
            id("state/cardinality"),
            id("state/relation"),
            id("state/connected"),
            id("state/span"),
            id("confirmation/after"),
            version("unused/cardinality", 2)
        ]
    );
    entries!(
        state,
        [id("state"), id("pending"), version("unused/state", 2)]
    );
    entries!(
        transition_condition,
        [
            id("transition/time"),
            id("transition/source"),
            id("transition/target"),
            id("unused/witness")
        ]
    );
    entries!(
        transition,
        [id("transition"), version("unused/transition", 2)]
    );
    entries!(
        conformance_criterion,
        [
            id("exists"),
            id("lineage"),
            id("identity"),
            id("construction"),
            id("snapshot/three")
        ]
    );
    entries!(
        conformance,
        [id("snapshot/fail/C"), version("unused/check/C", 2)]
    );
    entries!(projection, [version("unused/project/C", 2)]);
    entries!(lifecycle, [version("unused/revoke/O", 2)]);
    for kind in ["O", "M", "R", "C", "S", "T"] {
        entries!(conformance, [id(&format!("check/{kind}"))]);
        entries!(projection, [id(&format!("project/{kind}"))]);
        entries!(lifecycle, [id(&format!("lifecycle/{kind}"))]);
    }
    // Missing version 1 must remain missing, never replaced by version 2.
    assert!(a.operator(&id("unused/read")).is_none());
    assert!(b.operator(&id("unused/read")).is_none());
}
