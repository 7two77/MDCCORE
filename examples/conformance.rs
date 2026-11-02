#[allow(dead_code)]
#[path = "state_transition.rs"]
mod fixture;
use mdc_core::contract::conformance::*;
use mdc_core::contract::dynamics::CriterionOrder;
use mdc_core::contract::evaluation::{ConformanceResult, LifecycleStatus};
use mdc_core::contract::registry::VersionedId;
use mdc_core::pipeline::{EvaluationRequest, Pipeline, TransitionRequest};
fn id(name: &str) -> VersionedId {
    VersionedId::new(name, 1).unwrap()
}
fn main() {
    let mut registry = fixture::registry();
    let context = registry.state(&id("state")).unwrap().context.clone();
    let mut criteria = Vec::new();
    for (name, kind) in [
        ("exists", ConformanceCriterionKind::TargetExists),
        ("lineage", ConformanceCriterionKind::LineageComplete),
        ("construction", ConformanceCriterionKind::ConstructionPass),
    ] {
        registry
            .register_conformance_criterion(ConformanceCriterionContract {
                reference: id(name),
                kind,
            })
            .unwrap();
        criteria.push(id(name));
    }
    registry
        .register_conformance(ConformanceContract {
            reference: id("check"),
            target_type: TargetType::Transition,
            criteria,
            criterion_order: CriterionOrder::Unordered,
            context: context.clone(),
            missing_target: MissingTargetPolicy::Unresolved,
        })
        .unwrap();
    let mut p = Pipeline::new(registry);
    let source = fixture::state(&mut p, 10, [12, 6]);
    let target = fixture::state(&mut p, 30, [8, 4]);
    let t = p
        .transition(TransitionRequest {
            contract: id("transition"),
            source,
            target,
            observation_horizon: Some(50),
        })
        .unwrap();
    let e = p
        .evaluate(EvaluationRequest {
            contract: id("check"),
            target: t.clone(),
            context,
        })
        .unwrap();
    let evaluation = p.resolve_evaluation(&e).unwrap();
    assert_eq!(evaluation.result(), Some(ConformanceResult::Pass));
    assert_eq!(evaluation.status(), LifecycleStatus::Conformant);
    assert_eq!(
        p.resolve_transition(&t).unwrap().status(),
        LifecycleStatus::Candidate
    );
    println!(
        "evaluation={:?} status={:?} lineage_nodes={}",
        evaluation.result(),
        evaluation.status(),
        p.trace(&e).unwrap().len()
    );
}
