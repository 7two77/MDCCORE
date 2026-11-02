//! External observer fixture: all transformations are calls to the actual kernel.
#[allow(dead_code)]
#[path = "../state_transition.rs"]
mod base;
use mdc_core::canonical::snapshot::snapshot;
use mdc_core::contract::conformance::*;
use mdc_core::contract::dynamics::*;
use mdc_core::contract::evaluation::{
    aggregate_results, ConformanceResult as R, LifecycleStatus as S,
};
use mdc_core::contract::projection::ProjectionProfile;
use mdc_core::contract::registry::{Registry, RegistryError, VersionedId};
use mdc_core::identity::CanonicalSemanticBasis;
use mdc_core::model::invalid::{InvalidBody, InvalidKind};
use mdc_core::pipeline::*;
use mdc_core::provenance::ObjectRef;
use std::collections::BTreeSet;
use std::panic::{catch_unwind, AssertUnwindSafe};

pub const IDS: [&str; 23] = [
    "E2E-A01", "E2E-A02", "E2E-A03", "E2E-A04", "E2E-A05", "E2E-A06", "E2E-A07", "E2E-A08",
    "E2E-A09", "E2E-A10", "E2E-A11", "E2E-A12", "N01", "N02", "N03", "N04", "N05", "N06", "N07",
    "N08", "N09", "N10", "N11",
];
fn id(name: &str) -> VersionedId {
    VersionedId::new(name, 1).unwrap()
}
fn q(value: &str) -> String {
    let mut s = String::from("\"");
    for c in value.chars() {
        match c {
            '"' => s.push_str("\\\""),
            '\\' => s.push_str("\\\\"),
            '\n' => s.push_str("\\n"),
            '\r' => s.push_str("\\r"),
            '\t' => s.push_str("\\t"),
            c if c < ' ' => s.push_str(&format!("\\u{:04x}", c as u32)),
            c => s.push(c),
        }
    }
    s.push('"');
    s
}
fn array(items: impl IntoIterator<Item = String>) -> String {
    format!("[{}]", items.into_iter().collect::<Vec<_>>().join(","))
}
fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(HEX[(byte >> 4) as usize] as char);
        result.push(HEX[(byte & 15) as usize] as char);
    }
    result
}
fn semantic(value: Option<R>) -> String {
    value.map_or_else(
        || "null".into(),
        |r| {
            q(match r {
                R::Pass => "PASS",
                R::Fail => "FAIL",
                R::Unresolved => "UNRESOLVED",
            })
        },
    )
}
fn ref_json(reference: &ObjectRef) -> String {
    let (kind, id) = match reference {
        ObjectRef::InvalidObservation(id) => ("InvalidObservation", id),
        ObjectRef::InvalidMeasurement(id) => ("InvalidMeasurement", id),
        ObjectRef::InvalidRelation(id) => ("InvalidRelation", id),
        ObjectRef::InvalidComposition(id) => ("InvalidComposition", id),
        ObjectRef::InvalidState(id) => ("InvalidState", id),
        ObjectRef::InvalidTransition(id) => ("InvalidTransition", id),
        _ => return q(&format!("{reference:?}")),
    };
    format!(
        "{{\"kind\":{},\"digest\":{},\"canonical_key_hex\":{}}}",
        q(kind),
        id.digest(),
        q(&hex(id.canonical_key()))
    )
}
fn context(registry: &Registry) -> DynamicsContext {
    registry.state(&id("state")).unwrap().context.clone()
}

fn registry() -> Registry {
    let mut r = base::registry();
    let ctx = context(&r);
    let mut source = r.source(&id("source")).unwrap().clone();
    source.reference = id("confirmation");
    r.register_source(source).unwrap();
    for (name, kind) in [
        (
            "state/after",
            StateCriterionKind::ObservationAfter {
                source: id("confirmation"),
            },
        ),
        (
            "state/three",
            StateCriterionKind::Cardinality { min: 3, max: 3 },
        ),
    ] {
        r.register_state_criterion(StateCriterionContract {
            reference: id(name),
            kind,
        })
        .unwrap();
    }
    let mut pending = r.state(&id("state")).unwrap().clone();
    pending.reference = id("pending");
    pending.criteria.push(id("state/after"));
    r.register_state(pending.clone()).unwrap();
    pending.reference = id("failed-state");
    pending.criteria.push(id("state/three"));
    r.register_state(pending).unwrap();
    let mut transition = r.transition(&id("transition")).unwrap().clone();
    transition.reference = id("pending-transition");
    transition.source_state = id("pending");
    transition.target_state = id("pending");
    r.register_transition(transition).unwrap();
    let mut criteria = Vec::new();
    for (name, kind) in [
        ("cf/exists", ConformanceCriterionKind::TargetExists),
        ("cf/lineage", ConformanceCriterionKind::LineageComplete),
        ("cf/identity", ConformanceCriterionKind::IdentityCoherent),
        (
            "cf/construction",
            ConformanceCriterionKind::ConstructionPass,
        ),
    ] {
        r.register_conformance_criterion(ConformanceCriterionContract {
            reference: id(name),
            kind,
        })
        .unwrap();
        criteria.push(id(name));
    }
    for (name, target_type) in [
        ("check/O", TargetType::Observation),
        ("check/M", TargetType::Measurement),
        ("check/R", TargetType::Relation),
        ("check/C", TargetType::Composition),
        ("check/S", TargetType::State),
        ("check/T", TargetType::Transition),
    ] {
        r.register_conformance(ConformanceContract {
            reference: id(name),
            target_type,
            criteria: criteria.clone(),
            criterion_order: CriterionOrder::Unordered,
            context: ctx.clone(),
            missing_target: MissingTargetPolicy::Unresolved,
        })
        .unwrap();
    }
    r.register_projection(ProjectionProfile {
        reference: id("projection"),
        schema: id("TEST-REP-01"),
        target_type: TargetType::Transition,
        conformance: id("check/T"),
        context: ctx,
    })
    .unwrap();
    r
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Node {
    reference: ObjectRef,
    identity: u64,
    basis: Vec<u8>,
}
impl Node {
    fn json(&self) -> String {
        format!(
            "{{\"reference\":{},\"identity\":{},\"basis_hex\":{}}}",
            ref_json(&self.reference),
            self.identity,
            q(&hex(&self.basis))
        )
    }
}
fn node(p: &Pipeline, r: &ObjectRef) -> Node {
    let stored = match r {
        ObjectRef::Observation(id) => {
            let x = p.resolve_observation(r).unwrap();
            assert_eq!(x.id(), *id);
            x.id()
        }
        ObjectRef::Measurement(id) => {
            let x = p.resolve_measurement(r).unwrap();
            assert_eq!(x.id(), *id);
            x.id()
        }
        ObjectRef::Relation(id) => {
            let x = p.resolve_relation(r).unwrap();
            assert_eq!(x.id(), *id);
            x.id()
        }
        ObjectRef::Composition(id) => {
            let x = p.resolve_composition(r).unwrap();
            assert_eq!(x.id(), *id);
            x.id()
        }
        ObjectRef::State(id) => {
            let x = p.resolve_state(r).unwrap();
            assert_eq!(x.id(), *id);
            x.id()
        }
        ObjectRef::Transition(id) => {
            let x = p.resolve_transition(r).unwrap();
            assert_eq!(x.id(), *id);
            x.id()
        }
        ObjectRef::Evaluation(id) => {
            let x = p.resolve_evaluation(r).unwrap();
            assert_eq!(x.id(), *id);
            x.id()
        }
        _ => panic!("not a positive semantic ref"),
    };
    let s = match r {
        ObjectRef::Observation(_) => snapshot(p.resolve_observation(r).unwrap()),
        ObjectRef::Measurement(_) => snapshot(p.resolve_measurement(r).unwrap()),
        ObjectRef::Relation(_) => snapshot(p.resolve_relation(r).unwrap()),
        ObjectRef::Composition(_) => snapshot(p.resolve_composition(r).unwrap()),
        ObjectRef::State(_) => snapshot(p.resolve_state(r).unwrap()),
        ObjectRef::Transition(_) => snapshot(p.resolve_transition(r).unwrap()),
        ObjectRef::Evaluation(_) => snapshot(p.resolve_evaluation(r).unwrap()),
        _ => panic!("only admitted semantic objects enter positive graph"),
    };
    assert_eq!(
        stored, s.identity,
        "stored identity must agree with captured semantic content"
    );
    Node {
        reference: r.clone(),
        identity: stored,
        basis: s.semantic_basis,
    }
}
struct Run {
    p: Pipeline,
    refs: Vec<ObjectRef>,
    operations: Vec<String>,
    source_comp: Option<ObjectRef>,
    target_comp: Option<ObjectRef>,
    source: Option<ObjectRef>,
    target: Option<ObjectRef>,
    transition: Option<ObjectRef>,
    evaluation: Option<ObjectRef>,
}
impl Run {
    fn new() -> Self {
        Self {
            p: Pipeline::new(registry()),
            refs: Vec::new(),
            operations: Vec::new(),
            source_comp: None,
            target_comp: None,
            source: None,
            target: None,
            transition: None,
            evaluation: None,
        }
    }
    fn graph(&self) -> Vec<Node> {
        let mut refs = self.refs.clone();
        refs.sort();
        refs.dedup();
        refs.iter().map(|r| node(&self.p, r)).collect()
    }
    fn counts(&self) -> [usize; 7] {
        [
            self.p.observation_count(),
            self.p.measurement_count(),
            self.p.relation_count(),
            self.p.composition_count(),
            self.p.state_count(),
            self.p.transition_count(),
            self.p.evaluation_count(),
        ]
    }
    fn op(
        &mut self,
        name: &str,
        f: impl FnOnce(&mut Pipeline) -> Result<ObjectRef, PipelineError>,
    ) -> Result<ObjectRef, PipelineError> {
        let before = self.graph();
        let result = f(&mut self.p);
        let after: Vec<_> = before.iter().map(|n| node(&self.p, &n.reference)).collect();
        assert_eq!(before, after, "upstream mutation during {name}");
        let output = match &result {
            Ok(r) => {
                self.refs.push(r.clone());
                format!("{{\"reference\":{}}}", ref_json(r))
            }
            Err(e) => q(&format!("{e:?}")),
        };
        self.operations.push(format!("{{\"operation\":{},\"before\":{},\"after\":{},\"upstream_unchanged\":true,\"output\":{}}}",q(name),array(before.iter().map(Node::json)),array(after.iter().map(Node::json)),output));
        result
    }
    fn pair(&mut self, first: u64, values: [i64; 2], reverse: bool) -> ObjectRef {
        let indices = if reverse { [1, 0] } else { [0, 1] };
        let mut observations = [None, None];
        for i in indices {
            let t = first + i as u64 * 10;
            observations[i] = Some(
                self.op("admit", |p| {
                    p.admit(ObservationRequest {
                        source: id("source"),
                        timestamp: Some(t),
                        payload: values[i].to_le_bytes().to_vec(),
                        acquisition_context: format!("fixture/record/{t}"),
                    })
                })
                .unwrap(),
            );
        }
        let mut measurements = [None, None];
        for i in indices {
            let o = observations[i].clone().unwrap();
            measurements[i] = Some(
                self.op("measure", |p| {
                    p.measure(MeasurementRequest {
                        operator: id("read"),
                        inputs: vec![o],
                    })
                })
                .unwrap(),
            );
        }
        let m = [
            measurements[0].clone().unwrap(),
            measurements[1].clone().unwrap(),
        ];
        for i in 0..2 {
            assert_eq!(
                self.p.resolve_measurement(&m[i]).unwrap().value(),
                i128::from(values[i])
            );
        }
        let edge = self
            .op("relate", |p| {
                p.relate(RelationRequest {
                    contract: id("ratio"),
                    left: m[0].clone(),
                    right: m[1].clone(),
                })
            })
            .unwrap();
        assert_eq!(
            self.p.resolve_relation(&edge).unwrap().value(),
            &mdc_core::model::relation::RelationValue::Proportion {
                numerator: i128::from(values[0]),
                denominator: i128::from(values[1])
            }
        );
        self.op("compose", |p| {
            p.compose(CompositionRequest {
                contract: id("composition"),
                members: if reverse {
                    vec![m[1].clone(), m[0].clone()]
                } else {
                    m.to_vec()
                },
                relations: vec![edge],
            })
        })
        .unwrap()
    }
    fn state(&mut self, contract: &str, c: ObjectRef) -> ObjectRef {
        self.op("derive_state", |p| {
            p.derive_state(StateRequest {
                contract: id(contract),
                composition: c,
                observation_horizon: Some(60),
            })
        })
        .unwrap()
    }
    fn eval(&mut self, contract: &str, target: ObjectRef) -> ObjectRef {
        let ctx = self
            .p
            .registry()
            .conformance(&id(contract))
            .unwrap()
            .context
            .clone();
        self.op("evaluate", |p| {
            p.evaluate(EvaluationRequest {
                contract: id(contract),
                target,
                context: ctx,
            })
        })
        .unwrap()
    }
    fn build(reverse: bool, pending: bool) -> Self {
        let mut r = Self::new();
        let (a, b) = if reverse {
            let b = r.pair(30, [8, 4], true);
            let a = r.pair(10, [12, 6], true);
            (a, b)
        } else {
            let a = r.pair(10, [12, 6], false);
            let b = r.pair(30, [8, 4], false);
            (a, b)
        };
        r.source_comp = Some(a.clone());
        r.target_comp = Some(b.clone());
        let contract = if pending { "pending" } else { "state" };
        let (source, target) = if reverse {
            let target = r.state(contract, b);
            let source = r.state(contract, a);
            (source, target)
        } else {
            let source = r.state(contract, a);
            let target = r.state(contract, b);
            (source, target)
        };
        r.source = Some(source.clone());
        r.target = Some(target.clone());
        let transition = r
            .op("transition", |p| {
                p.transition(TransitionRequest {
                    contract: id(if pending {
                        "pending-transition"
                    } else {
                        "transition"
                    }),
                    source,
                    target,
                    observation_horizon: Some(60),
                })
            })
            .unwrap();
        r.transition = Some(transition.clone());
        let evaluation = r.eval("check/T", transition);
        r.evaluation = Some(evaluation);
        assert_eq!(r.counts(), [4, 4, 2, 2, 2, 1, 1]);
        assert_eq!(r.p.invalid_count(), 0);
        assert!(r.p.rejections().is_empty());
        r
    }
    fn invalid_evidence(&self) -> String {
        let refs: BTreeSet<_> = self
            .p
            .rejections()
            .iter()
            .map(|receipt| receipt.invalid_reference().clone())
            .collect();
        array(refs.iter().map(|reference|{
   let artifact=self.p.resolve_invalid(reference).unwrap();let closure=self.p.lineage_report(reference);
   let results=match artifact.body(){
    InvalidBody::Composition{constraints:Some(values),..}=>array(values.iter().map(|c|format!("{{\"reference\":{},\"kind\":{},\"result\":{}}}",q(&format!("{:?}",c.reference())),q(&format!("{:?}",c.kind())),semantic(Some(c.result()))))),
    InvalidBody::State{criteria:Some(values),..}=>array(values.iter().map(|c|format!("{{\"reference\":{},\"kind\":{},\"result\":{},\"evidence\":{}}}",q(&format!("{:?}",c.reference())),q(&format!("{:?}",c.kind())),semantic(Some(c.result())),array(c.evidence().iter().map(ref_json))))),
    InvalidBody::Transition{conditions:Some(values),..}=>array(values.iter().map(|c|format!("{{\"reference\":{},\"kind\":{},\"result\":{},\"evidence\":{}}}",q(&format!("{:?}",c.reference())),q(&format!("{:?}",c.kind())),semantic(Some(c.result())),array(c.evidence().iter().map(ref_json))))),_=>"[]".into()};
   format!("{{\"reference\":{},\"status\":\"INVALID\",\"kind\":{},\"basis_hex\":{},\"error\":{},\"local_results\":{},\"captured_inputs\":{},\"resolved\":{},\"unresolved\":{},\"unresolved_definitions\":{},\"complete\":{}}}",ref_json(reference),q(&format!("{:?}",artifact.kind())),q(&hex(&artifact.canonical_basis())),q(&format!("{:?}",artifact.error())),results,array(artifact.captured_inputs().iter().map(|input|format!("{{\"reference\":{},\"basis_hex\":{}}}",ref_json(input.reference()),q(&hex(input.basis()))))),array(closure.resolved.iter().map(ref_json)),array(closure.unresolved.iter().map(ref_json)),array(closure.unresolved_definitions.iter().map(|d|format!("{{\"kind\":{},\"reference\":{{\"id\":{},\"version\":{}}}}}",q(&format!("{:?}",d.kind)),q(d.reference.id()),d.reference.version()))),closure.is_complete())
  }))
    }
    fn json(&self) -> String {
        let evaluations=self.refs.iter().filter_map(|r|self.p.resolve_evaluation(r)).map(|e|format!("{{\"reference\":{},\"target\":{},\"result\":{},\"status\":{},\"criteria\":{},\"diagnostics\":{}}}",ref_json(&ObjectRef::Evaluation(e.id())),ref_json(e.target()),semantic(e.result()),q(&format!("{:?}",e.status())),array(e.criteria().iter().map(|c|format!("{{\"id\":{},\"result\":{}}}",q(&format!("{:?}",c.reference())),semantic(Some(c.result()))))),q(&format!("{:?}",e.diagnostics()))));
        let lineage = self.evaluation.as_ref().map_or("[]".into(), |r| {
            array(self.p.trace(r).unwrap().iter().map(ref_json))
        });
        format!("{{\"canonical_graph\":{},\"operations\":{},\"lineage\":{},\"evaluations\":{},\"invalid_artifacts\":{}}}",array(self.graph().iter().map(Node::json)),array(self.operations.clone()),lineage,array(evaluations),self.invalid_evidence())
    }
    fn reject(
        &mut self,
        name: &str,
        f: impl FnOnce(&mut Pipeline) -> Result<ObjectRef, PipelineError>,
    ) -> (ObjectRef, PipelineError) {
        let before = self.counts();
        let receipts = self.p.rejections().len();
        let error = self.op(name, f).unwrap_err();
        assert_eq!(self.counts(), before);
        assert_eq!(self.p.rejections().len(), receipts + 1);
        let receipt = self.p.rejections().last().unwrap();
        assert_eq!(receipt.error(), &error);
        let r = receipt.invalid_reference().clone();
        assert_eq!(self.p.resolve_invalid(&r).unwrap().status(), S::Invalid);
        (r, error)
    }
}

pub struct Case {
    pub id: String,
    pub executed: bool,
    pub passed: bool,
    pub expected_semantic: Option<R>,
    pub actual_semantic: Option<R>,
    pub diagnostics: String,
    pub evidence: Vec<String>,
}
pub struct Report {
    pub cases: Vec<Case>,
    runs: Vec<String>,
    run_graphs: Vec<Vec<Node>>,
    run_lineages: Vec<Vec<ObjectRef>>,
}
fn declared_semantic_result(id: &str) -> Option<R> {
    match id {
        "E2E-A05" => Some(R::Pass),
        "E2E-A06" | "N04" => Some(R::Unresolved),
        "E2E-A07" | "N05" | "N07" => Some(R::Fail),
        _ => None,
    }
}
impl Report {
    pub fn accepted(&self) -> bool {
        self.runs.len() == 3
            && self.run_graphs.len() == 3
            && self.run_lineages.len() == 3
            && self.run_graphs.windows(2).all(|pair| pair[0] == pair[1])
            && self.run_lineages.windows(2).all(|pair| pair[0] == pair[1])
            && self.cases.len() == IDS.len()
            && IDS
                .iter()
                .all(|id| self.cases.iter().filter(|c| c.id == *id).count() == 1)
            && self.cases.iter().all(|c| {
                c.executed
                    && c.passed
                    && !c.evidence.is_empty()
                    && c.expected_semantic == declared_semantic_result(&c.id)
                    && c.expected_semantic == c.actual_semantic
            })
    }
    pub fn json(&self) -> String {
        format!("{{\"schema_version\":1,\"process_id\":{},\"fixture_id\":\"MDC-E2E-CONFORMANCE-001\",\"fixture_version\":1,\"profile\":\"MDC-SCALAR-001\",\"acceptance\":{},\"harness_status\":{},\"scope\":\"Declared deterministic fixture and contracts on this implementation; not universal conformance\",\"input\":{{\"source\":\"source/version1; signed-i64-le schema/version1; count; fixture/counter; fixture/ticks/version1\",\"records\":[[10,12],[20,6],[30,8],[40,4]],\"acquisition\":\"fixture/record/<source_timestamp>\",\"horizon\":60,\"calculus\":\"STP-MDC-0.1/version1\",\"domain\":\"MDC-SCALAR-001/version1\",\"contracts\":[\"schema\",\"source\",\"confirmation\",\"read\",\"ratio\",\"cardinality\",\"connected\",\"composition\",\"state\",\"pending\",\"failed-state\",\"transition\",\"pending-transition\",\"check/T\",\"projection\"],\"contract_version\":1,\"representation_schema\":\"TEST-REP-01/version1\"}},\"blockers\":[],\"runs\":{},\"cases\":{}}}",std::process::id(),q(if self.accepted(){"PASS"}else{"FAIL"}),q(if self.accepted(){"PASS"}else{"FAIL"}),array(self.runs.clone()),array(self.cases.iter().map(|c|format!("{{\"id\":{},\"mandatory\":true,\"executed\":{},\"harness_status\":{},\"expected\":{{\"harness_status\":\"PASS\",\"semantic_result\":{}}},\"actual\":{{\"semantic_result\":{}}},\"diagnostics\":{},\"evidence_refs\":[{}],\"evidence\":{}}}",q(&c.id),c.executed,q(if c.executed&&c.passed{"PASS"}else if c.executed{"FAIL"}else{"NOT_EXECUTED"}),semantic(c.expected_semantic),semantic(c.actual_semantic),if c.diagnostics.is_empty(){"[]".into()}else{array([q(&c.diagnostics)])},q(&format!("cases/{}/evidence",c.id)),array(c.evidence.clone())))))
    }
}
fn case(
    report: &mut Report,
    id: &str,
    expected: Option<R>,
    operation: impl FnOnce() -> (Option<R>, Vec<String>),
) {
    let result = catch_unwind(AssertUnwindSafe(operation));
    let (passed, actual, evidence, diagnostics) = match result {
        Ok((actual, evidence)) => (actual == expected, actual, evidence, String::new()),
        Err(error) => {
            let reason = error
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| error.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "fixture panicked".into());
            (false, None, Vec::new(), reason)
        }
    };
    report.cases.push(Case {
        id: id.into(),
        executed: true,
        passed,
        expected_semantic: expected,
        actual_semantic: actual,
        diagnostics,
        evidence,
    });
}
fn contracts_resolve(run: &Run) {
    for reference in &run.refs {
        match reference {
            ObjectRef::Observation(_) => {
                let o = run.p.resolve_observation(reference).unwrap();
                assert!(run.p.registry().source(o.source_ref()).is_some());
                assert!(run.p.registry().schema(o.schema()).is_some());
            }
            ObjectRef::Measurement(_) => assert!(run
                .p
                .registry()
                .operator(run.p.resolve_measurement(reference).unwrap().operator())
                .is_some()),
            ObjectRef::Relation(_) => assert!(run
                .p
                .registry()
                .relation(run.p.resolve_relation(reference).unwrap().contract())
                .is_some()),
            ObjectRef::Composition(_) => {
                let c = run.p.resolve_composition(reference).unwrap();
                assert!(run.p.registry().composition(c.contract()).is_some());
                for criterion in c.constraints() {
                    assert!(run.p.registry().constraint(criterion.reference()).is_some());
                }
            }
            ObjectRef::State(_) => {
                let s = run.p.resolve_state(reference).unwrap();
                assert!(run.p.registry().state(s.contract()).is_some());
                for criterion in s.criteria() {
                    assert!(run
                        .p
                        .registry()
                        .state_criterion(criterion.reference())
                        .is_some());
                }
            }
            ObjectRef::Transition(_) => {
                let t = run.p.resolve_transition(reference).unwrap();
                assert!(run.p.registry().transition(t.contract()).is_some());
                for criterion in t.conditions() {
                    assert!(run
                        .p
                        .registry()
                        .transition_condition(criterion.reference())
                        .is_some());
                }
            }
            ObjectRef::Evaluation(_) => {
                let e = run.p.resolve_evaluation(reference).unwrap();
                assert!(run.p.registry().conformance(e.contract()).is_some());
                for criterion in e.criteria() {
                    assert!(run
                        .p
                        .registry()
                        .conformance_criterion(criterion.reference())
                        .is_some());
                }
            }
            _ => panic!("unexpected reference in positive graph"),
        }
    }
}
fn missing_target() -> Run {
    let mut r = Run::build(false, false);
    let source = r.source.clone().unwrap();
    let missing = ObjectRef::State(0);
    let (invalid, error) = r.reject("missing_target", |p| {
        p.transition(TransitionRequest {
            contract: id("transition"),
            source,
            target: missing.clone(),
            observation_horizon: Some(60),
        })
    });
    assert_eq!(error, PipelineError::InputUnresolved(missing.clone()));
    assert!(r.p.resolve_state(&missing).is_none());
    assert!(r.p.lineage_report(&invalid).unresolved.contains(&missing));
    r
}
fn projection(backflow: bool) -> (Run, String) {
    let r = Run::build(false, false);
    let before = r.graph();
    let counts = r.counts();
    let request = ProjectionRequest {
        profile: id("projection"),
        target: r.transition.clone().unwrap(),
        evaluation: r.evaluation.clone().unwrap(),
    };
    let mut d = r.p.project(request.clone()).unwrap();
    let repeated = r.p.project(request).unwrap();
    assert_eq!(d.canonical_bytes(), repeated.canonical_bytes());
    assert_eq!(d.source_basis(), basis_of_transition(&r));
    let original = d.canonical_bytes();
    if backflow {
        d.label = "WAVE_3 BUY".into();
        assert_eq!(
            r.p.apply_representation(&d),
            Err(ProjectionError::BackflowForbidden)
        );
        assert_ne!(original, d.canonical_bytes());
    }
    assert_eq!(r.graph(), before);
    assert_eq!(r.counts(), counts);
    let evidence=format!("{{\"external_representation\":true,\"schema\":{},\"before_hex\":{},\"after_hex\":{},\"source_basis_hex\":{},\"reverse_attempt\":{},\"core_unchanged\":true}}",q(&format!("{:?}",d.schema())),q(&hex(&original)),q(&hex(&d.canonical_bytes())),q(&hex(d.source_basis())),backflow);
    (r, evidence)
}
fn basis_of_transition(r: &Run) -> Vec<u8> {
    r.p.resolve_transition(r.transition.as_ref().unwrap())
        .unwrap()
        .canonical_basis()
}
fn fail_with_unresolved() -> (Run, R) {
    let mut r = Run::build(false, true);
    let source = r.target.clone().unwrap();
    let target = r.source.clone().unwrap();
    let (invalid, error) = r.reject("reversed_pending_transition", |p| {
        p.transition(TransitionRequest {
            contract: id("pending-transition"),
            source,
            target,
            observation_horizon: Some(60),
        })
    });
    let values = match error {
        PipelineError::Transition(
            mdc_core::model::transition::TransitionError::FailedConditions(results),
        ) => results.iter().map(|c| c.result()).collect::<Vec<_>>(),
        other => panic!("wrong failure {other:?}"),
    };
    assert!(values.contains(&R::Fail));
    assert!(values.contains(&R::Unresolved));
    assert!(matches!(
        r.p.resolve_invalid(&invalid).unwrap().body(),
        InvalidBody::Transition {
            conditions: Some(_),
            ..
        }
    ));
    let result = aggregate_results(&values).unwrap();
    (r, result)
}
fn semantic_namespace(run: &Run) -> Vec<String> {
    let mut names = Vec::new();
    for r in &run.refs {
        names.push(
            match r {
                ObjectRef::Observation(_) => "OBSERVATION",
                ObjectRef::Measurement(_) => "MEASUREMENT",
                ObjectRef::Relation(_) => "RELATION",
                ObjectRef::Composition(_) => "COMPOSITION",
                ObjectRef::State(_) => "STATE",
                ObjectRef::Transition(_) => "TRANSITION",
                ObjectRef::Evaluation(_) => "EVALUATION",
                _ => panic!("unexpected semantic type"),
            }
            .into(),
        );
        if let Some(state) = run.p.resolve_state(r) {
            assert_eq!(state.definition(), StateDefinition::CompositionAbstraction);
            names.push(format!("{:?}", state.definition()));
        }
        if let Some(edge) = run.p.resolve_relation(r) {
            names.push(format!("{:?}", edge.kind()));
        }
    }
    names
}

pub fn execute() -> Report {
    let mut report = Report {
        cases: Vec::new(),
        runs: Vec::new(),
        run_graphs: Vec::new(),
        run_lineages: Vec::new(),
    };
    case(&mut report, "E2E-A01", None, || {
        let r = Run::build(false, false);
        contracts_resolve(&r);
        assert!(r.p.registry().projection(&id("projection")).is_some());
        (None, vec![r.json()])
    });
    // A02 also establishes the three actual run records used by final acceptance.
    let runs = catch_unwind(AssertUnwindSafe(|| {
        [
            Run::build(false, false),
            Run::build(false, false),
            Run::build(true, false),
        ]
    }));
    match runs {
        Ok(runs) => {
            let graphs: Vec<_> = runs.iter().map(Run::graph).collect();
            let lineages: Vec<_> = runs
                .iter()
                .map(|r| r.p.trace(r.evaluation.as_ref().unwrap()).unwrap())
                .collect();
            let passed = graphs.windows(2).all(|p| p[0] == p[1])
                && lineages.windows(2).all(|p| p[0] == p[1]);
            report.run_graphs = graphs;
            report.run_lineages = lineages;
            report.runs = runs
                .iter()
                .enumerate()
                .map(|(i, r)| {
                    format!(
                        "{{\"id\":{},{}",
                        q(["run-1", "run-2", "run-permuted"][i]),
                        &r.json()[1..]
                    )
                })
                .collect();
            report.cases.push(Case {
                id: "E2E-A02".into(),
                executed: true,
                passed,
                expected_semantic: None,
                actual_semantic: None,
                diagnostics: if passed {
                    String::new()
                } else {
                    "canonical graph or lineage changed".into()
                },
                evidence: report.runs.clone(),
            });
        }
        Err(_) => report.cases.push(Case {
            id: "E2E-A02".into(),
            executed: true,
            passed: false,
            expected_semantic: None,
            actual_semantic: None,
            diagnostics: "independent run construction panicked".into(),
            evidence: Vec::new(),
        }),
    }
    case(&mut report, "E2E-A03", None, || {
        let r = Run::build(false, false);
        let closure = r.p.trace(r.evaluation.as_ref().unwrap()).unwrap();
        let mut expected = r.refs.clone();
        expected.push(ObjectRef::Source(id("source")));
        expected.sort();
        expected.dedup();
        assert_eq!(closure, expected);
        assert_eq!(closure.len(), 17);
        (None, vec![r.json()])
    });
    case(&mut report, "E2E-A04", None, || {
        let r = Run::build(false, false);
        assert_eq!(r.operations.len(), 16);
        for name in [
            "admit",
            "measure",
            "relate",
            "compose",
            "derive_state",
            "transition",
            "evaluate",
        ] {
            assert!(r
                .operations
                .iter()
                .any(|op| op.contains(&format!("\"operation\":\"{name}\""))));
        }
        (None, vec![r.json()])
    });
    case(&mut report, "E2E-A05", Some(R::Pass), || {
        let r = Run::build(false, false);
        let actual =
            r.p.resolve_evaluation(r.evaluation.as_ref().unwrap())
                .unwrap()
                .result();
        assert_eq!(aggregate_results(&[R::Pass, R::Pass]), Ok(R::Pass));
        (actual, vec![r.json()])
    });
    case(&mut report, "E2E-A06", Some(R::Unresolved), || {
        let r = Run::build(false, true);
        assert_eq!(
            r.p.resolve_state(r.source.as_ref().unwrap())
                .unwrap()
                .criteria_result(),
            R::Unresolved
        );
        assert_eq!(
            r.p.resolve_transition(r.transition.as_ref().unwrap())
                .unwrap()
                .conditions_result(),
            R::Unresolved
        );
        assert_eq!(
            aggregate_results(&[R::Pass, R::Unresolved]),
            Ok(R::Unresolved)
        );
        (
            r.p.resolve_evaluation(r.evaluation.as_ref().unwrap())
                .unwrap()
                .result(),
            vec![r.json()],
        )
    });
    case(&mut report, "E2E-A07", Some(R::Fail), || {
        let (r, result) = fail_with_unresolved();
        assert_eq!(aggregate_results(&[R::Pass, R::Fail]), Ok(R::Fail));
        assert_eq!(aggregate_results(&[R::Unresolved, R::Fail]), Ok(R::Fail));
        (Some(result), vec![r.json()])
    });
    case(&mut report, "E2E-A08", None, || {
        let mut r = Run::build(false, false);
        let c = r.source_comp.clone().unwrap();
        let members = r.p.resolve_composition(&c).unwrap().members().to_vec();
        let observation = r.p.resolve_measurement(&members[0]).unwrap().inputs()[0].clone();
        let mut invalids = Vec::new();
        invalids.push(
            r.reject("invalid_observation", |p| {
                p.admit(ObservationRequest {
                    source: id("source"),
                    timestamp: Some(50),
                    payload: vec![1],
                    acquisition_context: "fixture/malformed".into(),
                })
            })
            .0,
        );
        let invalid_m = r
            .reject("invalid_measurement", |p| {
                p.measure(MeasurementRequest {
                    operator: id("read"),
                    inputs: vec![ObjectRef::Observation(0)],
                })
            })
            .0;
        invalids.push(invalid_m.clone());
        invalids.push(
            r.reject("invalid_relation", |p| {
                p.relate(RelationRequest {
                    contract: id("ratio"),
                    left: observation.clone(),
                    right: observation,
                })
            })
            .0,
        );
        invalids.push(
            r.reject("invalid_composition", |p| {
                p.compose(CompositionRequest {
                    contract: id("composition"),
                    members: vec![members[0].clone()],
                    relations: vec![],
                })
            })
            .0,
        );
        invalids.push(
            r.reject("invalid_state", |p| {
                p.derive_state(StateRequest {
                    contract: id("failed-state"),
                    composition: c,
                    observation_horizon: Some(60),
                })
            })
            .0,
        );
        let source = r.target.clone().unwrap();
        let target = r.source.clone().unwrap();
        invalids.push(
            r.reject("invalid_transition", |p| {
                p.transition(TransitionRequest {
                    contract: id("transition"),
                    source,
                    target,
                    observation_horizon: Some(60),
                })
            })
            .0,
        );
        let kinds: BTreeSet<_> = invalids
            .iter()
            .map(|reference| {
                let object = r.p.resolve_invalid(reference).unwrap();
                assert_eq!(object.status(), S::Invalid);
                assert_eq!(object.reference(), *reference);
                assert_eq!(object.id().canonical_key(), object.canonical_basis());
                object.kind()
            })
            .collect();
        assert_eq!(
            kinds,
            BTreeSet::from([
                InvalidKind::Observation,
                InvalidKind::Measurement,
                InvalidKind::Relation,
                InvalidKind::Composition,
                InvalidKind::State,
                InvalidKind::Transition
            ])
        );
        let (_, error) = r.reject("reject_invalid_input", |p| {
            p.measure(MeasurementRequest {
                operator: id("read"),
                inputs: vec![invalid_m.clone()],
            })
        });
        assert_eq!(error, PipelineError::InvalidInputType(invalid_m));
        (None, vec![r.json()])
    });
    case(&mut report, "E2E-A09", None, || {
        let r = missing_target();
        (None, vec![r.json()])
    });
    case(&mut report, "E2E-A10", None, || {
        let r = Run::build(false, false);
        let names = semantic_namespace(&r);
        let forbidden = [
            "WAVE",
            "TREND",
            "IMPULSE",
            "FIBONACCI",
            "ELLIOTT",
            "SIGNAL",
            "BUY",
            "SELL",
            "CAUSE",
            "PREDICTION",
        ];
        for name in &names {
            for token in name
                .to_uppercase()
                .split(|c: char| !c.is_ascii_alphanumeric())
            {
                assert!(
                    !forbidden.contains(&token),
                    "forbidden semantic token {token}"
                );
            }
        }
        let observed = format!(
            "{{\"semantic_namespace\":{},\"forbidden_intersection\":[]}}",
            array(names.iter().map(|s| q(s)))
        );
        (None, vec![r.json(), observed])
    });
    case(&mut report, "E2E-A11", None, || {
        let (r, evidence) = projection(true);
        (None, vec![r.json(), evidence])
    });
    case(&mut report, "E2E-A12", None, || {
        let (r, evidence) = projection(false);
        (None, vec![r.json(), evidence])
    });
    case(&mut report, "N01", None, || {
        let mut r = Run::build(false, false);
        let target = r.transition.clone().unwrap();
        let context = context(r.p.registry());
        let unknown = VersionedId::new("check/T", 2).unwrap();
        let e = r
            .op("unknown_evaluation_contract", |p| {
                p.evaluate(EvaluationRequest {
                    contract: unknown,
                    target,
                    context,
                })
            })
            .unwrap();
        let a = r.p.resolve_evaluation(&e).unwrap();
        assert_eq!(a.result(), None);
        assert_eq!(a.status(), S::Invalid);
        assert!(a.criteria().is_empty());
        let c = r.source_comp.clone().unwrap();
        let (_, error) = r.reject("unknown_state_contract", |p| {
            p.derive_state(StateRequest {
                contract: VersionedId::new("state", 2).unwrap(),
                composition: c,
                observation_horizon: Some(60),
            })
        });
        assert_eq!(
            error,
            PipelineError::StateContractUnresolved(VersionedId::new("state", 2).unwrap())
        );
        (None, vec![r.json()])
    });
    case(&mut report, "N02", None, || {
        let mut r = Run::build(false, false);
        let missing = ObjectRef::Composition(0);
        let (invalid, error) = r.reject("missing_composition", |p| {
            p.derive_state(StateRequest {
                contract: id("state"),
                composition: missing.clone(),
                observation_horizon: Some(60),
            })
        });
        assert_eq!(error, PipelineError::InputUnresolved(missing.clone()));
        assert!(r.p.lineage_report(&invalid).unresolved.contains(&missing));
        assert!(r.p.resolve_composition(&missing).is_none());
        (None, vec![r.json()])
    });
    case(&mut report, "N03", None, || {
        let mut registry = registry();
        let mut incomplete = registry.conformance(&id("check/T")).unwrap().clone();
        incomplete.reference = id("incomplete-check");
        incomplete.criteria.push(id("undefined-criterion"));
        let error = registry.register_conformance(incomplete).unwrap_err();
        assert_eq!(
            error,
            RegistryError::ConformanceCriterionUnresolved(id("undefined-criterion"))
        );
        assert!(registry.conformance(&id("incomplete-check")).is_none());
        (
            None,
            vec![format!(
                "{{\"operation\":\"register_conformance\",\"error\":{},\"contract_absent\":true}}",
                q(&format!("{error:?}"))
            )],
        )
    });
    case(&mut report, "N04", Some(R::Unresolved), || {
        let r = Run::build(false, true);
        let state = r.p.resolve_state(r.target.as_ref().unwrap()).unwrap();
        let criterion = state
            .criteria()
            .iter()
            .find(|c| c.reference() == &id("state/after"))
            .unwrap();
        assert!(criterion.evidence().is_empty());
        (Some(criterion.result()), vec![r.json()])
    });
    case(&mut report, "N05", Some(R::Fail), || {
        let mut r = Run::build(false, false);
        let composition = r.source_comp.clone().unwrap();
        let (invalid, error) = r.reject("failed_state_criterion", |p| {
            p.derive_state(StateRequest {
                contract: id("failed-state"),
                composition,
                observation_horizon: Some(60),
            })
        });
        let values = match error {
            PipelineError::State(mdc_core::model::state::StateError::FailedCriteria(results)) => {
                results.iter().map(|c| c.result()).collect::<Vec<_>>()
            }
            other => panic!("unexpected {other:?}"),
        };
        assert!(values.contains(&R::Pass));
        assert!(values.contains(&R::Fail));
        assert!(values.contains(&R::Unresolved));
        assert!(matches!(
            r.p.resolve_invalid(&invalid).unwrap().body(),
            InvalidBody::State {
                criteria: Some(_),
                ..
            }
        ));
        (Some(aggregate_results(&values).unwrap()), vec![r.json()])
    });
    case(&mut report, "N06", None, || {
        let r = missing_target();
        (None, vec![r.json()])
    });
    case(&mut report, "N07", Some(R::Fail), || {
        let mut r = Run::build(false, false);
        let source = r.target.clone().unwrap();
        let target = r.source.clone().unwrap();
        let (_, error) = r.reject("reversed_temporal_order", |p| {
            p.transition(TransitionRequest {
                contract: id("transition"),
                source,
                target,
                observation_horizon: Some(60),
            })
        });
        let conditions = match error {
            PipelineError::Transition(
                mdc_core::model::transition::TransitionError::FailedConditions(results),
            ) => results,
            other => panic!("unexpected {other:?}"),
        };
        let temporal = conditions
            .iter()
            .find(|c| matches!(c.kind(), TransitionConditionKind::TemporalOrder { .. }))
            .unwrap();
        (Some(temporal.result()), vec![r.json()])
    });
    case(&mut report, "N08", None, || {
        let mut r = Run::build(false, false);
        let missing = ObjectRef::Observation(0);
        let (invalid, error) = r.reject("incomplete_provenance", |p| {
            p.measure(MeasurementRequest {
                operator: id("read"),
                inputs: vec![missing.clone()],
            })
        });
        assert_eq!(error, PipelineError::InputUnresolved(missing.clone()));
        let closure = r.p.lineage_report(&invalid);
        assert!(!closure.is_complete());
        assert!(closure.unresolved.contains(&missing));
        assert!(closure.unresolved_definitions.is_empty());
        assert!(r.p.trace(&invalid).is_err());
        (None, vec![r.json()])
    });
    case(&mut report, "N09", None, || {
        let normal = Run::build(false, false);
        let permuted = Run::build(true, false);
        assert_eq!(normal.graph(), permuted.graph());
        assert_eq!(
            normal.p.trace(normal.evaluation.as_ref().unwrap()).unwrap(),
            permuted
                .p
                .trace(permuted.evaluation.as_ref().unwrap())
                .unwrap()
        );
        (None, vec![normal.json(), permuted.json()])
    });
    case(&mut report, "N10", None, || {
        let (r, evidence) = projection(true);
        (None, vec![r.json(), evidence])
    });
    case(&mut report, "N11", None, || {
        let mut r = Run::build(false, false);
        let (invalid, error) = r.reject("absent_future_source", |p| {
            p.admit(ObservationRequest {
                source: id("absent-future-source"),
                timestamp: Some(80),
                payload: 99_i64.to_le_bytes().to_vec(),
                acquisition_context: "fixture/future".into(),
            })
        });
        assert_eq!(
            error,
            PipelineError::SourceUnresolved(id("absent-future-source"))
        );
        let closure = r.p.lineage_report(&invalid);
        assert!(!closure.is_complete());
        assert!(closure
            .unresolved
            .contains(&ObjectRef::Source(id("absent-future-source"))));
        let source = r.source.clone().unwrap();
        let missing = ObjectRef::State(0);
        let (_, error) = r.reject("no_future_state_synthesis", |p| {
            p.transition(TransitionRequest {
                contract: id("transition"),
                source,
                target: missing.clone(),
                observation_horizon: Some(90),
            })
        });
        assert_eq!(error, PipelineError::InputUnresolved(missing.clone()));
        assert!(r.p.resolve_state(&missing).is_none());
        (None, vec![r.json()])
    });
    report
}
