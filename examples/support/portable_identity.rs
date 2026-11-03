//! Calls the core operations; contains no identity or derivation algorithm.
#[allow(dead_code)]
#[path = "../lifecycle.rs"]
pub mod scenario;

use mdc_core::contract::evaluation::{ConformanceResult, LifecycleStatus};
use mdc_core::identity::{CanonicalSemanticBasis, IdentityProtocol};
use mdc_core::pipeline::*;
use mdc_core::provenance::ObjectRef;
use std::fmt::Write;

pub fn hex(bytes: &[u8]) -> String {
    let mut text = String::new();
    for byte in bytes {
        write!(&mut text, "{byte:02x}").unwrap();
    }
    text
}

fn record(out: &mut String, kind: &str, digest: u64, body: &[u8]) {
    assert_eq!(IdentityProtocol::PortableV1.derive_from_basis(body), digest);
    writeln!(out, "{kind}\t{digest:016x}\t{}", hex(body)).unwrap();
}

pub fn run() -> String {
    let protocol = IdentityProtocol::PortableV1;
    let mut g = scenario::graph_with_identity_protocol(scenario::registry(), protocol);
    assert_eq!(g.p.identity_protocol(), protocol);
    let mut out = format!("protocol={}\n", protocol.name());
    let before = scenario::capture(&g.p, &g.refs);
    for (reference, body) in &before {
        let (kind, id) = match reference {
            ObjectRef::Observation(id) => {
                assert_eq!(
                    g.p.resolve_observation(reference)
                        .unwrap()
                        .provenance()
                        .identity_profile(),
                    protocol.name()
                );
                ("observation", *id)
            }
            ObjectRef::Measurement(id) => ("measurement", *id),
            ObjectRef::Relation(id) => ("relation", *id),
            ObjectRef::Composition(id) => ("composition", *id),
            ObjectRef::State(id) => ("state", *id),
            ObjectRef::Transition(id) => ("transition", *id),
            ObjectRef::Evaluation(id) => ("evaluation", *id),
            _ => unreachable!(),
        };
        record(&mut out, kind, id, body);
        let lineage = g.p.trace(reference).unwrap();
        writeln!(
            &mut out,
            "lineage\t{}",
            lineage
                .iter()
                .map(|r| hex(&mdc_core::identity::canonical_reference_v1(r)))
                .collect::<Vec<_>>()
                .join(";")
        )
        .unwrap();
        assert!(g
            .p
            .trace(reference)
            .unwrap()
            .iter()
            .any(|r| matches!(r, ObjectRef::Source(_))));
    }
    for (kind, target) in ["O", "M", "R", "C", "S", "T"].iter().zip(&g.roots) {
        let contract = scenario::id(&format!("check/{kind}"));
        let context =
            g.p.registry()
                .conformance(&contract)
                .unwrap()
                .context
                .clone();
        let reference =
            g.p.evaluate_keyed(EvaluationRequest {
                contract,
                target: target.clone(),
                context,
            })
            .unwrap();
        let ObjectRef::EvaluationKey(key) = &reference else {
            panic!("missing full key")
        };
        let artifact = g.p.resolve_evaluation(&reference).unwrap();
        assert_eq!(artifact.result(), Some(ConformanceResult::Pass));
        assert_eq!(
            mdc_core::provenance::EvaluationId::from_canonical_basis_with_protocol(
                artifact.canonical_basis(),
                protocol
            ),
            *key
        );
        assert!(g
            .p
            .project(ProjectionRequest {
                profile: scenario::id(&format!("project/{kind}")),
                target: target.clone(),
                evaluation: reference
            })
            .is_ok());
        record(
            &mut out,
            "evaluation-key",
            artifact.id(),
            &artifact.canonical_basis(),
        );
    }
    let missing = scenario::id("absent/version");
    let failures = [
        g.p.admit(ObservationRequest {
            source: missing.clone(),
            timestamp: Some(1),
            payload: vec![],
            acquisition_context: "missing".into(),
        }),
        g.p.measure(MeasurementRequest {
            operator: missing.clone(),
            inputs: vec![g.roots[0].clone()],
        }),
        g.p.relate(RelationRequest {
            contract: missing.clone(),
            left: g.roots[0].clone(),
            right: g.roots[1].clone(),
        }),
        g.p.compose(CompositionRequest {
            contract: missing.clone(),
            members: vec![g.roots[0].clone()],
            relations: vec![],
        }),
        g.p.derive_state(StateRequest {
            contract: missing.clone(),
            composition: g.roots[3].clone(),
            observation_horizon: Some(50),
        }),
        g.p.transition(TransitionRequest {
            contract: missing,
            source: g.roots[4].clone(),
            target: g.second_state.clone(),
            observation_horizon: Some(50),
        }),
    ];
    assert!(failures.iter().all(Result::is_err));
    assert_eq!(g.p.rejections().len(), 6);
    for rejection in g.p.rejections() {
        let invalid = g.p.resolve_invalid(rejection.invalid_reference()).unwrap();
        record(
            &mut out,
            "invalid",
            invalid.id().digest(),
            &invalid.canonical_basis(),
        );
    }
    let request = scenario::request(&g.p, "portable/withdraw", "O", &g.roots[0], &g.witness);
    let id = g.p.invalidate(request).unwrap();
    let event = g.p.resolve_lifecycle_event(&id).unwrap();
    record(&mut out, "lifecycle", id.digest(), &event.canonical_basis());
    assert_eq!(scenario::capture(&g.p, &g.refs), before);
    assert_eq!(
        g.p.lifecycle_view(&g.evaluation).unwrap().status,
        LifecycleStatus::Invalid
    );
    let reevaluation = scenario::evaluate(&mut g.p, "T", &g.roots[5]);
    let artifact = g.p.resolve_evaluation(&reevaluation).unwrap();
    assert_eq!(artifact.result(), Some(ConformanceResult::Fail));
    record(
        &mut out,
        "reevaluation",
        artifact.id(),
        &artifact.canonical_basis(),
    );
    writeln!(&mut out, "MDC_PORTABLE_IDENTITY_VERIFY=PASS").unwrap();
    out
}
