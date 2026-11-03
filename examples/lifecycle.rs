//! Real API fixture shared by lifecycle regression tests and this hosted example.
#[allow(dead_code)]
#[path = "state_transition.rs"]
mod core_fixture;
use mdc_core::contract::conformance::*;
use mdc_core::contract::dynamics::{CriterionOrder, StateCriterionContract, StateCriterionKind};
use mdc_core::contract::evaluation::{ConformanceResult, LifecycleStatus};
use mdc_core::contract::lifecycle::*;
use mdc_core::contract::projection::ProjectionProfile;
use mdc_core::contract::registry::{Registry, VersionedId};
use mdc_core::identity::CanonicalSemanticBasis;
use mdc_core::model::lifecycle::*;
use mdc_core::pipeline::*;
use mdc_core::provenance::ObjectRef;
use std::collections::BTreeMap;

pub fn id(name: &str) -> VersionedId {
    VersionedId::new(name, 1).unwrap()
}
pub fn registry() -> Registry {
    let mut r = core_fixture::registry();
    let context = r.state(&id("state")).unwrap().context.clone();
    let mut confirmation = r.source(&id("source")).unwrap().clone();
    confirmation.reference = id("confirmation");
    r.register_source(confirmation).unwrap();
    r.register_state_criterion(StateCriterionContract {
        reference: id("confirmation/after"),
        kind: StateCriterionKind::ObservationAfter {
            source: id("confirmation"),
        },
    })
    .unwrap();
    let mut pending = r.state(&id("state")).unwrap().clone();
    pending.reference = id("pending");
    pending.criteria.push(id("confirmation/after"));
    r.register_state(pending).unwrap();
    let mut criteria = Vec::new();
    for (name, kind) in [
        ("exists", ConformanceCriterionKind::TargetExists),
        ("lineage", ConformanceCriterionKind::LineageComplete),
        ("identity", ConformanceCriterionKind::IdentityCoherent),
        ("construction", ConformanceCriterionKind::ConstructionPass),
    ] {
        r.register_conformance_criterion(ConformanceCriterionContract {
            reference: id(name),
            kind,
        })
        .unwrap();
        criteria.push(id(name));
    }
    for (name, target_type) in [
        ("O", TargetType::Observation),
        ("M", TargetType::Measurement),
        ("R", TargetType::Relation),
        ("C", TargetType::Composition),
        ("S", TargetType::State),
        ("T", TargetType::Transition),
    ] {
        r.register_conformance(ConformanceContract {
            reference: id(&format!("check/{name}")),
            target_type,
            criteria: criteria.clone(),
            criterion_order: CriterionOrder::Unordered,
            context: context.clone(),
            missing_target: MissingTargetPolicy::Unresolved,
        })
        .unwrap();
        r.register_projection(ProjectionProfile {
            reference: id(&format!("project/{name}")),
            schema: id("TEST-REP-01"),
            target_type,
            conformance: id(&format!("check/{name}")),
            context: context.clone(),
        })
        .unwrap();
        r.register_lifecycle(LifecycleContract {
            reference: id(&format!("lifecycle/{name}")),
            target_type,
            context: context.clone(),
            allowed_reasons: vec![
                InvalidationReason::EvidenceWithdrawn,
                InvalidationReason::SourceContractViolation,
                InvalidationReason::ContradictoryTemporalIdentity,
                InvalidationReason::DerivationContractViolation,
            ],
            propagation: InvalidationPropagation::RequiredDeclaredInputs,
        })
        .unwrap();
    }
    r
}
pub struct Graph {
    pub p: Pipeline,
    pub roots: [ObjectRef; 6],
    pub second_state: ObjectRef,
    pub evaluation: ObjectRef,
    pub witness: ObjectRef,
    pub refs: Vec<ObjectRef>,
}
pub fn graph() -> Graph {
    graph_with_registry(registry())
}
pub fn graph_with_registry(registry: Registry) -> Graph {
    let mut p = Pipeline::new(registry);
    let s1 = core_fixture::state(&mut p, 10, [12, 6]);
    let s2 = core_fixture::state(&mut p, 30, [8, 4]);
    let t = p
        .transition(TransitionRequest {
            contract: id("transition"),
            source: s1.clone(),
            target: s2.clone(),
            observation_horizon: Some(50),
        })
        .unwrap();
    let e = evaluate(&mut p, "T", &t);
    let mut refs: Vec<_> = p
        .trace(&e)
        .unwrap()
        .into_iter()
        .filter(|r| !matches!(r, ObjectRef::Source(_)))
        .collect();
    let o = refs
        .iter()
        .find(|r| {
            p.resolve_observation(r)
                .is_some_and(|o| o.timestamp() == 10)
        })
        .unwrap()
        .clone();
    let m = refs
        .iter()
        .find(|r| {
            p.resolve_measurement(r)
                .is_some_and(|m| m.provenance().inputs().contains(&o))
        })
        .unwrap()
        .clone();
    let c = p.resolve_state(&s1).unwrap().source_composition().clone();
    let relation = p.resolve_composition(&c).unwrap().relations()[0].clone();
    let witness = p
        .admit(ObservationRequest {
            source: id("confirmation"),
            timestamp: Some(90),
            payload: 1_i64.to_le_bytes().to_vec(),
            acquisition_context: "fixture/confirmation/90".into(),
        })
        .unwrap();
    refs.push(witness.clone());
    Graph {
        p,
        roots: [o, m, relation, c, s1, t],
        second_state: s2,
        evaluation: e,
        witness,
        refs,
    }
}
pub fn evaluate(p: &mut Pipeline, kind: &str, target: &ObjectRef) -> ObjectRef {
    let contract = id(&format!("check/{kind}"));
    let context = p.registry().conformance(&contract).unwrap().context.clone();
    p.evaluate(EvaluationRequest {
        contract,
        target: target.clone(),
        context,
    })
    .unwrap()
}
pub fn request(
    p: &Pipeline,
    name: &str,
    kind: &str,
    target: &ObjectRef,
    witness: &ObjectRef,
) -> InvalidationRequest {
    InvalidationRequest {
        request_id: name.into(),
        contract: id(&format!("lifecycle/{kind}")),
        target: target.clone(),
        reason: InvalidationReason::EvidenceWithdrawn,
        evidence: vec![witness.clone()],
        expected_revision: p.lifecycle_revision(),
    }
}
pub fn basis(p: &Pipeline, r: &ObjectRef) -> Vec<u8> {
    match r {
        ObjectRef::Observation(_) => p.resolve_observation(r).unwrap().canonical_basis(),
        ObjectRef::Measurement(_) => p.resolve_measurement(r).unwrap().canonical_basis(),
        ObjectRef::Relation(_) => p.resolve_relation(r).unwrap().canonical_basis(),
        ObjectRef::Composition(_) => p.resolve_composition(r).unwrap().canonical_basis(),
        ObjectRef::State(_) => p.resolve_state(r).unwrap().canonical_basis(),
        ObjectRef::Transition(_) => p.resolve_transition(r).unwrap().canonical_basis(),
        ObjectRef::Evaluation(_) | ObjectRef::EvaluationKey(_) => {
            p.resolve_evaluation(r).unwrap().canonical_basis()
        }
        _ => panic!("only retained admitted objects in this fixture"),
    }
}
pub fn capture(p: &Pipeline, refs: &[ObjectRef]) -> BTreeMap<ObjectRef, Vec<u8>> {
    refs.iter().map(|r| (r.clone(), basis(p, r))).collect()
}
fn main() {
    let mut g = graph();
    let before = capture(&g.p, &g.refs);
    let invalidation = request(&g.p, "withdraw/1", "O", &g.roots[0], &g.witness);
    let event = g.p.invalidate(invalidation).unwrap();
    assert_eq!(before, capture(&g.p, &g.refs));
    assert_eq!(
        g.p.lifecycle_view(&g.evaluation).unwrap().status,
        LifecycleStatus::Invalid
    );
    assert_eq!(
        g.p.resolve_evaluation(&g.evaluation).unwrap().result(),
        Some(ConformanceResult::Pass)
    );
    let assessment = evaluate(&mut g.p, "T", &g.roots[5]);
    assert_eq!(
        g.p.resolve_evaluation(&assessment).unwrap().result(),
        Some(ConformanceResult::Fail)
    );
    println!(
        "MDC_LIFECYCLE_VERIFY=PASS revision={} affected={} retained_snapshots={} reevaluation={:?}",
        g.p.lifecycle_revision(),
        g.p.resolve_lifecycle_event(&event)
            .unwrap()
            .affected()
            .len(),
        before.len(),
        g.p.resolve_evaluation(&assessment).unwrap().result()
    );
    let bytes =
        g.p.resolve_lifecycle_event(&event)
            .unwrap()
            .canonical_basis();
    use std::fmt::Write;
    let mut hex = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut hex, "{byte:02x}").unwrap();
    }
    println!("event_canonical_hex={hex}");
}
