//! Failure retention and creation-time lineage snapshots.
use super::{Pipeline, PipelineError, RejectedAttempt, RejectedRequest};
use crate::contract::invalid_encoding::put_source_contract;
use crate::identity::CanonicalSemanticBasis;
use crate::model::composition::CompositionError;
use crate::model::invalid::*;
use crate::model::state::StateError;
use crate::model::transition::TransitionError;
use crate::provenance::ObjectRef;
use alloc::borrow::ToOwned;
use alloc::collections::BTreeSet;
use alloc::vec::Vec;

pub(super) enum ComputedCandidate {
    Measurement(crate::model::measurement::Measurement),
    Relation(crate::model::relation::AdmittedRelation),
    Composition(crate::model::composition::AdmittedComposition),
    State(crate::model::state::AdmittedState),
    Transition(crate::model::transition::AdmittedTransition),
}
struct Capture {
    inputs: Vec<CapturedInput>,
    missing: Vec<ObjectRef>,
    definitions: Vec<UnresolvedDefinition>,
}
impl Pipeline {
    pub(super) fn record_rejection(&mut self, request: RejectedRequest, error: PipelineError) {
        let mut definitions = Vec::new();
        let mut absent =
            |kind, reference| definitions.push(UnresolvedDefinition { kind, reference });
        let (mut body, mut roots) = match &request {
            RejectedRequest::Observation(r) => {
                let definition = self.registry.source(&r.source).cloned();
                let schema = definition
                    .as_ref()
                    .and_then(|s| self.registry.schema(&s.schema))
                    .cloned();
                if definition.is_none() {
                    absent(DefinitionKind::Source, r.source.clone());
                }
                if let Some(source) = &definition {
                    if schema.is_none() {
                        absent(DefinitionKind::Schema, source.schema.clone());
                    }
                }
                (
                    InvalidBody::Observation {
                        source: r.source.clone(),
                        definition,
                        schema,
                        timestamp: r.timestamp,
                        payload: r.payload.clone(),
                        acquisition_context: r.acquisition_context.clone(),
                    },
                    alloc::vec![ObjectRef::Source(r.source.clone())],
                )
            }
            RejectedRequest::Measurement(r) => {
                let definition = self.registry.operator(&r.operator).cloned();
                if definition.is_none() {
                    absent(DefinitionKind::Operator, r.operator.clone());
                }
                let metadata: Option<Vec<_>> = r
                    .inputs
                    .iter()
                    .map(|reference| {
                        if let Some(o) = self.resolve_observation(reference) {
                            Some((o.unit().to_owned(), o.context().clone()))
                        } else {
                            self.resolve_measurement(reference)
                                .map(|m| (m.unit().to_owned(), m.context().clone()))
                        }
                    })
                    .collect();
                let (unit, context) = if definition.is_some() {
                    if let Some(values) = metadata.filter(|v| !v.is_empty()) {
                        let unit = values
                            .iter()
                            .all(|v| v.0 == values[0].0)
                            .then(|| values[0].0.clone());
                        let context = values
                            .iter()
                            .all(|v| v.1 == values[0].1)
                            .then(|| values[0].1.clone());
                        (unit, context)
                    } else {
                        (None, None)
                    }
                } else {
                    (None, None)
                };
                (
                    InvalidBody::Measurement {
                        operator: r.operator.clone(),
                        definition,
                        inputs: r.inputs.clone(),
                        value: None,
                        unit,
                        context,
                    },
                    r.inputs.clone(),
                )
            }
            RejectedRequest::Relation(r) => {
                let definition = self.registry.relation(&r.contract).cloned();
                if definition.is_none() {
                    absent(DefinitionKind::Relation, r.contract.clone());
                }
                (
                    InvalidBody::Relation {
                        contract: r.contract.clone(),
                        definition,
                        left: r.left.clone(),
                        right: r.right.clone(),
                        value: None,
                    },
                    alloc::vec![r.left.clone(), r.right.clone()],
                )
            }
            RejectedRequest::Composition(r) => {
                let definition = self.registry.composition(&r.contract).cloned();
                if definition.is_none() {
                    absent(DefinitionKind::Composition, r.contract.clone());
                }
                let constraints = if let PipelineError::Composition(
                    CompositionError::FailedConstraints(results),
                ) = &error
                {
                    Some(results.clone())
                } else {
                    None
                };
                let mut roots = r.members.clone();
                roots.extend(r.relations.clone());
                (
                    InvalidBody::Composition {
                        contract: r.contract.clone(),
                        definition,
                        members: r.members.clone(),
                        relations: r.relations.clone(),
                        constraints,
                    },
                    roots,
                )
            }
            RejectedRequest::State(r) => {
                let definition = self.registry.state(&r.contract).cloned();
                if definition.is_none() {
                    absent(DefinitionKind::State, r.contract.clone());
                }
                let criteria =
                    if let PipelineError::State(StateError::FailedCriteria(results)) = &error {
                        Some(results.clone())
                    } else {
                        None
                    };
                let interval = if self.resolve_composition(&r.composition).is_some() {
                    self.observation_interval(&r.composition).ok()
                } else {
                    None
                };
                let mut roots = alloc::vec![r.composition.clone()];
                if let Some(results) = &criteria {
                    roots.extend(results.iter().flat_map(|c| c.evidence().iter().cloned()));
                }
                (
                    InvalidBody::State {
                        contract: r.contract.clone(),
                        definition,
                        source_composition: r.composition.clone(),
                        observation_horizon: r.observation_horizon,
                        interval,
                        criteria,
                    },
                    roots,
                )
            }
            RejectedRequest::Transition(r) => {
                let definition = self.registry.transition(&r.contract).cloned();
                if definition.is_none() {
                    absent(DefinitionKind::Transition, r.contract.clone());
                }
                let conditions =
                    if let PipelineError::Transition(TransitionError::FailedConditions(results)) =
                        &error
                    {
                        Some(results.clone())
                    } else {
                        None
                    };
                let mut roots = alloc::vec![r.source.clone(), r.target.clone()];
                if let Some(results) = &conditions {
                    roots.extend(results.iter().flat_map(|c| c.evidence().iter().cloned()));
                }
                (
                    InvalidBody::Transition {
                        contract: r.contract.clone(),
                        definition,
                        source_state: r.source.clone(),
                        target_state: r.target.clone(),
                        observation_horizon: r.observation_horizon,
                        conditions,
                    },
                    roots,
                )
            }
        };
        if let Some(candidate) = self.failed_candidate.take() {
            debug_assert!(matches!(
                error,
                PipelineError::IdentityCollision(_) | PipelineError::InputInvalidated { .. }
            ));
            match (*candidate, &mut body) {
                (
                    ComputedCandidate::Measurement(candidate),
                    InvalidBody::Measurement {
                        value,
                        unit,
                        context,
                        ..
                    },
                ) => {
                    *value = Some(candidate.value());
                    *unit = Some(candidate.unit().into());
                    *context = Some(candidate.context().clone());
                    roots.extend(candidate.provenance().inputs().iter().cloned());
                }
                (ComputedCandidate::Relation(candidate), InvalidBody::Relation { value, .. }) => {
                    *value = Some(candidate.value().clone());
                    roots.extend(candidate.provenance().inputs().iter().cloned());
                }
                (
                    ComputedCandidate::Composition(candidate),
                    InvalidBody::Composition { constraints, .. },
                ) => {
                    *constraints = Some(candidate.constraints().to_vec());
                    roots.extend(candidate.provenance().inputs().iter().cloned());
                }
                (
                    ComputedCandidate::State(candidate),
                    InvalidBody::State {
                        interval, criteria, ..
                    },
                ) => {
                    *interval = Some(candidate.validity_interval().clone());
                    *criteria = Some(candidate.criteria().to_vec());
                    roots.extend(candidate.provenance().inputs().iter().cloned());
                }
                (
                    ComputedCandidate::Transition(candidate),
                    InvalidBody::Transition { conditions, .. },
                ) => {
                    *conditions = Some(candidate.conditions().to_vec());
                    roots.extend(candidate.provenance().inputs().iter().cloned());
                }
                _ => unreachable!("collision candidate matches the rejected operation"),
            }
        }
        let capture = self.capture_closure(&roots);
        definitions.extend(capture.definitions);
        let artifact = InvalidSemanticObject::new(InvalidArtifactData {
            body,
            error: error.clone(),
            captured_inputs: capture.inputs,
            unresolved: capture.missing,
            unresolved_definitions: definitions,
        });
        let reference = artifact.reference();
        // Full canonical keys distinguish digest collisions without losing either
        // failure or replacing its original error with a retention error.
        self.invalid_objects
            .entry(reference.clone())
            .or_insert(artifact);
        self.rejections.push(RejectedAttempt {
            request,
            error,
            invalid_reference: reference,
        });
    }
    pub fn resolve_invalid(&self, reference: &ObjectRef) -> Option<&InvalidSemanticObject> {
        self.invalid_objects.get(reference)
    }
    pub fn invalid_count(&self) -> usize {
        self.invalid_objects.len()
    }
    /// Captured closure for invalid objects; current immutable closure otherwise.
    /// Completeness is not a claim of admissibility or external authenticity.
    pub fn lineage_report(&self, reference: &ObjectRef) -> LineageReport {
        let capture = self.capture_closure(core::slice::from_ref(reference));
        let mut resolved: Vec<_> = capture
            .inputs
            .iter()
            .map(|i| i.reference().clone())
            .collect();
        resolved.sort();
        resolved.dedup();
        let mut unresolved = capture.missing;
        unresolved.sort();
        unresolved.dedup();
        let mut unresolved_definitions = capture.definitions;
        unresolved_definitions.sort();
        unresolved_definitions.dedup();
        LineageReport {
            resolved,
            unresolved,
            unresolved_definitions,
        }
    }
    fn capture_closure(&self, roots: &[ObjectRef]) -> Capture {
        let mut pending = roots.to_vec();
        let mut visited = BTreeSet::new();
        let mut result = Capture {
            inputs: Vec::new(),
            missing: Vec::new(),
            definitions: Vec::new(),
        };
        while let Some(reference) = pending.pop() {
            if !visited.insert(reference.clone()) {
                continue;
            }
            if let Some(invalid) = self.resolve_invalid(&reference) {
                result
                    .inputs
                    .push(CapturedInput::new(reference, invalid.canonical_basis()));
                result
                    .inputs
                    .extend(invalid.captured_inputs().iter().cloned());
                result
                    .missing
                    .extend(invalid.unresolved_references().iter().cloned());
                result
                    .definitions
                    .extend(invalid.unresolved_definitions().iter().cloned());
                continue;
            }
            let resolved = match &reference {
                ObjectRef::Source(source) => self.registry.source(source).map(|definition| {
                    let mut basis = b"MDC-SCALAR-001/source-snapshot/v1\0".to_vec();
                    put_source_contract(&mut basis, definition);
                    if self.registry.schema(&definition.schema).is_none() {
                        result.definitions.push(UnresolvedDefinition {
                            kind: DefinitionKind::Schema,
                            reference: definition.schema.clone(),
                        });
                    }
                    (basis, Vec::new())
                }),
                ObjectRef::Observation(_) => self.resolve_observation(&reference).map(|o| {
                    (
                        o.canonical_basis(),
                        alloc::vec![ObjectRef::Source(o.source_ref().clone())],
                    )
                }),
                ObjectRef::Measurement(_) => self
                    .resolve_measurement(&reference)
                    .map(|o| (o.canonical_basis(), o.provenance().inputs().to_vec())),
                ObjectRef::Relation(_) => self
                    .resolve_relation(&reference)
                    .map(|o| (o.canonical_basis(), o.provenance().inputs().to_vec())),
                ObjectRef::Composition(_) => self
                    .resolve_composition(&reference)
                    .map(|o| (o.canonical_basis(), o.provenance().inputs().to_vec())),
                ObjectRef::State(_) => self
                    .resolve_state(&reference)
                    .map(|o| (o.canonical_basis(), o.provenance().inputs().to_vec())),
                ObjectRef::Transition(_) => self
                    .resolve_transition(&reference)
                    .map(|o| (o.canonical_basis(), o.provenance().inputs().to_vec())),
                ObjectRef::Evaluation(_) | ObjectRef::EvaluationKey(_) => {
                    self.resolve_evaluation(&reference).map(|o| {
                        if self.registry.conformance(o.contract()).is_none() {
                            result.definitions.push(UnresolvedDefinition {
                                kind: DefinitionKind::Conformance,
                                reference: o.contract().clone(),
                            });
                        }
                        (o.canonical_basis(), o.provenance().inputs().to_vec())
                    })
                }
                _ => None,
            };
            if let Some((basis, inputs)) = resolved {
                result.inputs.push(CapturedInput::new(reference, basis));
                pending.extend(inputs);
            } else {
                result.missing.push(reference);
            }
        }
        result
    }
}
