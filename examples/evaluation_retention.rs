//! Canonical Evaluation references through real conformance and lifecycle APIs.
#[allow(dead_code)]
#[path = "lifecycle.rs"]
mod scenario;
use mdc_core::contract::evaluation::{ConformanceResult, LifecycleStatus};
use mdc_core::pipeline::ProjectionRequest;
use mdc_core::provenance::ObjectRef;
use std::fmt::Write;

fn main() {
    let mut g = scenario::graph();
    let old = scenario::capture(&g.p, &g.refs);
    let mut refs = Vec::new();
    let mut encoded = String::new();
    for (kind, target) in ["O", "M", "R", "C", "S", "T"].iter().zip(g.roots.iter()) {
        let contract = scenario::id(&format!("check/{kind}"));
        let context =
            g.p.registry()
                .conformance(&contract)
                .unwrap()
                .context
                .clone();
        let reference =
            g.p.evaluate_keyed(mdc_core::pipeline::EvaluationRequest {
                contract,
                target: target.clone(),
                context,
            })
            .unwrap();
        let ObjectRef::EvaluationKey(key) = &reference else {
            panic!("canonical API returned an alias")
        };
        let artifact = g.p.resolve_evaluation(&reference).unwrap();
        assert_eq!(artifact.id(), key.digest());
        assert_eq!(artifact.key(), *key);
        assert_eq!(artifact.result(), Some(ConformanceResult::Pass));
        assert!(g
            .p
            .project(ProjectionRequest {
                profile: scenario::id(&format!("project/{kind}")),
                target: target.clone(),
                evaluation: reference.clone()
            })
            .is_ok());
        write!(
            &mut encoded,
            "{kind}:{}:{}:",
            key.digest(),
            key.canonical_key().len()
        )
        .unwrap();
        for byte in key.canonical_key() {
            write!(&mut encoded, "{byte:02x}").unwrap();
        }
        encoded.push('|');
        refs.push(reference);
    }
    assert_eq!(g.p.evaluation_count(), 6);
    let revoke = scenario::request(&g.p, "withdraw", "O", &g.roots[0], &g.witness);
    g.p.invalidate(revoke).unwrap();
    assert_eq!(scenario::capture(&g.p, &g.refs), old);
    for reference in &refs {
        assert_eq!(
            g.p.lifecycle_view(reference).unwrap().status,
            LifecycleStatus::Invalid
        );
        assert_eq!(
            g.p.resolve_evaluation(reference).unwrap().result(),
            Some(ConformanceResult::Pass)
        );
    }
    assert_eq!(
        g.p.lifecycle_view(&g.evaluation),
        g.p.lifecycle_view(&refs[5])
    );
    println!("MDC_EVALUATION_RETENTION_VERIFY=PASS evaluations={} canonical_refs={} snapshots_preserved={}", g.p.evaluation_count(), refs.len(), old.len());
    println!("canonical_keys_snapshot={encoded}");
}
