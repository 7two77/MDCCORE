//! Registered source admission and measurement construction for MDC-SCALAR-001.
//!
//! The registry is frozen when moved into a pipeline. Stored objects are exposed
//! only by shared reference. Rejected requests are retained as audit attempts;
//! they are never admitted as usable semantic objects.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use alloc::vec::Vec;

use crate::contract::evaluation::LifecycleStatus;
use crate::contract::registry::{Registry, VersionedId};
use crate::identity::CanonicalSemanticBasis;
use crate::model::composition::{AdmittedComposition, CompositionError};
use crate::model::measurement::{Measurement, MeasurementError, ResolvedInput};
use crate::model::observation::{AdmissionError, AdmittedObservation};
use crate::model::relation::{AdmittedRelation, RelationError};
use crate::model::state::{AdmittedState, StateError};
use crate::model::transition::{AdmittedTransition, TransitionError};
use crate::provenance::ObjectRef;

mod projection;
pub use projection::{ProjectionError, ProjectionRequest};
mod conformance;
mod invalid;
pub use conformance::EvaluationRequest;
mod structure;
pub use structure::{CompositionRequest, RelationRequest};
mod dynamics;
pub use dynamics::{StateRequest, TransitionRequest};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservationRequest {
    pub source: VersionedId,
    pub timestamp: Option<u64>,
    pub payload: Vec<u8>,
    /// Deterministic source acquisition coordinate, not a generated runtime ID.
    pub acquisition_context: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MeasurementRequest {
    pub operator: VersionedId,
    pub inputs: Vec<ObjectRef>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PipelineError {
    SourceUnresolved(VersionedId),
    SchemaUnresolved(VersionedId),
    Admission(AdmissionError),
    OperatorUnresolved(VersionedId),
    RelationContractUnresolved(VersionedId),
    CompositionContractUnresolved(VersionedId),
    ConstraintUnresolved(VersionedId),
    StateContractUnresolved(VersionedId),
    StateCriterionUnresolved(VersionedId),
    TransitionContractUnresolved(VersionedId),
    TransitionConditionUnresolved(VersionedId),
    InputUnresolved(ObjectRef),
    InvalidInputType(ObjectRef),
    Measurement(MeasurementError),
    Relation(RelationError),
    Composition(CompositionError),
    State(StateError),
    Transition(TransitionError),
    IdentityCollision(u64),
    ProvenanceIncomplete(ObjectRef),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RejectedRequest {
    Observation(ObservationRequest),
    Measurement(MeasurementRequest),
    Relation(RelationRequest),
    Composition(CompositionRequest),
    State(StateRequest),
    Transition(TransitionRequest),
}

/// An invalid construction attempt retained separately from admitted objects.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RejectedAttempt {
    request: RejectedRequest,
    error: PipelineError,
    invalid_reference: ObjectRef,
}

impl RejectedAttempt {
    pub fn invalid_reference(&self) -> &ObjectRef {
        &self.invalid_reference
    }
    pub fn request(&self) -> &RejectedRequest {
        &self.request
    }

    pub fn error(&self) -> &PipelineError {
        &self.error
    }

    pub fn status(&self) -> LifecycleStatus {
        LifecycleStatus::Invalid
    }
}

pub struct Pipeline {
    registry: Registry,
    observations: BTreeMap<u64, AdmittedObservation>,
    measurements: BTreeMap<u64, Measurement>,
    relations: BTreeMap<u64, AdmittedRelation>,
    compositions: BTreeMap<u64, AdmittedComposition>,
    states: BTreeMap<u64, AdmittedState>,
    transitions: BTreeMap<u64, AdmittedTransition>,
    evaluations: BTreeMap<u64, crate::model::evaluation::ConformanceEvaluation>,
    failed_candidate: Option<alloc::boxed::Box<invalid::ComputedCandidate>>,
    invalid_objects: BTreeMap<ObjectRef, crate::model::invalid::InvalidSemanticObject>,
    rejections: Vec<RejectedAttempt>,
}

impl Pipeline {
    pub fn new(registry: Registry) -> Self {
        Self {
            registry,
            observations: BTreeMap::new(),
            measurements: BTreeMap::new(),
            relations: BTreeMap::new(),
            compositions: BTreeMap::new(),
            states: BTreeMap::new(),
            transitions: BTreeMap::new(),
            evaluations: BTreeMap::new(),
            failed_candidate: None,
            invalid_objects: BTreeMap::new(),
            rejections: Vec::new(),
        }
    }

    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    pub fn admit(&mut self, request: ObservationRequest) -> Result<ObjectRef, PipelineError> {
        match self.try_admit(&request) {
            Ok(reference) => Ok(reference),
            Err(error) => {
                self.record_rejection(RejectedRequest::Observation(request), error.clone());
                Err(error)
            }
        }
    }

    fn try_admit(&mut self, request: &ObservationRequest) -> Result<ObjectRef, PipelineError> {
        let source = self
            .registry
            .source(&request.source)
            .ok_or_else(|| PipelineError::SourceUnresolved(request.source.clone()))?;
        let schema = self
            .registry
            .schema(&source.schema)
            .ok_or_else(|| PipelineError::SchemaUnresolved(source.schema.clone()))?;
        let observation = AdmittedObservation::admit(
            source,
            schema,
            request.timestamp,
            request.payload.clone(),
            request.acquisition_context.clone(),
        )
        .map_err(PipelineError::Admission)?;
        let id = observation.id();
        self.check_identity(id, &observation.canonical_basis())?;
        self.observations.entry(id).or_insert(observation);
        Ok(ObjectRef::Observation(id))
    }

    pub fn measure(&mut self, request: MeasurementRequest) -> Result<ObjectRef, PipelineError> {
        match self.try_measure(&request) {
            Ok(reference) => Ok(reference),
            Err(error) => {
                self.record_rejection(RejectedRequest::Measurement(request), error.clone());
                Err(error)
            }
        }
    }

    fn try_measure(&mut self, request: &MeasurementRequest) -> Result<ObjectRef, PipelineError> {
        let operator = self
            .registry
            .operator(&request.operator)
            .ok_or_else(|| PipelineError::OperatorUnresolved(request.operator.clone()))?;
        let inputs: Vec<_> = request
            .inputs
            .iter()
            .map(|reference| self.resolve_input(reference))
            .collect::<Result<_, _>>()?;
        let measurement =
            Measurement::derive(operator, &inputs).map_err(PipelineError::Measurement)?;
        let id = measurement.id();
        if let Err(error) = self.check_identity(id, &measurement.canonical_basis()) {
            self.failed_candidate = Some(alloc::boxed::Box::new(
                invalid::ComputedCandidate::Measurement(measurement),
            ));
            return Err(error);
        }
        self.measurements.entry(id).or_insert(measurement);
        Ok(ObjectRef::Measurement(id))
    }

    fn resolve_input(&self, reference: &ObjectRef) -> Result<ResolvedInput, PipelineError> {
        match reference {
            ObjectRef::Observation(_) => {
                let observation = self
                    .resolve_observation(reference)
                    .ok_or_else(|| PipelineError::InputUnresolved(reference.clone()))?;
                self.trace(reference)?;
                Ok(ResolvedInput {
                    reference: reference.clone(),
                    value: observation.scalar_value(),
                    unit: observation.unit().into(),
                    context: observation.context().clone(),
                })
            }
            ObjectRef::Measurement(_) => {
                let measurement = self
                    .resolve_measurement(reference)
                    .ok_or_else(|| PipelineError::InputUnresolved(reference.clone()))?;
                self.trace(reference)?;
                Ok(ResolvedInput {
                    reference: reference.clone(),
                    value: measurement.value(),
                    unit: measurement.unit().into(),
                    context: measurement.context().clone(),
                })
            }
            ObjectRef::Source(_)
            | ObjectRef::Relation(_)
            | ObjectRef::Composition(_)
            | ObjectRef::State(_)
            | ObjectRef::Transition(_)
            | ObjectRef::Evaluation(_)
            | ObjectRef::InvalidObservation(_)
            | ObjectRef::InvalidMeasurement(_)
            | ObjectRef::InvalidRelation(_)
            | ObjectRef::InvalidComposition(_)
            | ObjectRef::InvalidState(_)
            | ObjectRef::InvalidTransition(_) => {
                Err(PipelineError::InvalidInputType(reference.clone()))
            }
        }
    }

    /// Refuse a colliding u64 instead of overwriting another object's content.
    /// This detects local collisions, not a proof of global hash uniqueness.
    fn check_identity(&self, id: u64, basis: &[u8]) -> Result<(), PipelineError> {
        let existing = self
            .observations
            .get(&id)
            .map(|object| object.canonical_basis())
            .or_else(|| {
                self.measurements
                    .get(&id)
                    .map(|object| object.canonical_basis())
            })
            .or_else(|| {
                self.relations
                    .get(&id)
                    .map(|object| object.canonical_basis())
            })
            .or_else(|| {
                self.compositions
                    .get(&id)
                    .map(|object| object.canonical_basis())
            })
            .or_else(|| self.states.get(&id).map(|object| object.canonical_basis()))
            .or_else(|| {
                self.transitions
                    .get(&id)
                    .map(|object| object.canonical_basis())
            });
        let existing = existing.or_else(|| {
            self.evaluations
                .get(&id)
                .map(|object| object.canonical_basis())
        });
        if existing.is_some_and(|existing| existing != basis) {
            return Err(PipelineError::IdentityCollision(id));
        }
        Ok(())
    }

    pub fn resolve_observation(&self, reference: &ObjectRef) -> Option<&AdmittedObservation> {
        match reference {
            ObjectRef::Observation(id) => self.observations.get(id),
            _ => None,
        }
    }

    pub fn resolve_measurement(&self, reference: &ObjectRef) -> Option<&Measurement> {
        match reference {
            ObjectRef::Measurement(id) => self.measurements.get(id),
            _ => None,
        }
    }

    pub fn observation_count(&self) -> usize {
        self.observations.len()
    }

    pub fn measurement_count(&self) -> usize {
        self.measurements.len()
    }

    pub fn rejections(&self) -> &[RejectedAttempt] {
        &self.rejections
    }

    /// Returns a deterministic recursive closure including the start object.
    /// A reference is resolved against stored objects; tracing never creates one.
    pub fn trace(&self, start: &ObjectRef) -> Result<Vec<ObjectRef>, PipelineError> {
        let mut pending = alloc::vec![start.clone()];
        let mut visited = BTreeSet::new();
        while let Some(reference) = pending.pop() {
            if !visited.insert(reference.clone()) {
                continue;
            }
            match &reference {
                ObjectRef::InvalidObservation(_)
                | ObjectRef::InvalidMeasurement(_)
                | ObjectRef::InvalidRelation(_)
                | ObjectRef::InvalidComposition(_)
                | ObjectRef::InvalidState(_)
                | ObjectRef::InvalidTransition(_) => {
                    let report = self.lineage_report(&reference);
                    if !report.is_complete() {
                        return Err(PipelineError::ProvenanceIncomplete(reference));
                    }
                    pending.extend(report.resolved);
                }
                ObjectRef::Source(source_ref) => {
                    let source = self
                        .registry
                        .source(source_ref)
                        .ok_or_else(|| PipelineError::ProvenanceIncomplete(reference.clone()))?;
                    if self.registry.schema(&source.schema).is_none() {
                        return Err(PipelineError::ProvenanceIncomplete(reference));
                    }
                }
                ObjectRef::Observation(_) => {
                    let observation = self
                        .resolve_observation(&reference)
                        .ok_or_else(|| PipelineError::InputUnresolved(reference.clone()))?;
                    pending.push(ObjectRef::Source(observation.provenance().source().clone()));
                }
                ObjectRef::Measurement(_) => {
                    let measurement = self
                        .resolve_measurement(&reference)
                        .ok_or_else(|| PipelineError::InputUnresolved(reference.clone()))?;
                    if self
                        .registry
                        .operator(measurement.provenance().contract())
                        .is_none()
                    {
                        return Err(PipelineError::ProvenanceIncomplete(reference));
                    }
                    pending.extend(measurement.provenance().inputs().iter().cloned());
                }
                ObjectRef::Relation(_) => {
                    let relation = self
                        .resolve_relation(&reference)
                        .ok_or_else(|| PipelineError::InputUnresolved(reference.clone()))?;
                    if self.registry.relation(relation.contract()).is_none() {
                        return Err(PipelineError::ProvenanceIncomplete(reference));
                    }
                    pending.extend(relation.provenance().inputs().iter().cloned());
                }
                ObjectRef::Composition(_) => {
                    let composition = self
                        .resolve_composition(&reference)
                        .ok_or_else(|| PipelineError::InputUnresolved(reference.clone()))?;
                    if self.registry.composition(composition.contract()).is_none()
                        || composition
                            .constraints()
                            .iter()
                            .any(|result| self.registry.constraint(result.reference()).is_none())
                    {
                        return Err(PipelineError::ProvenanceIncomplete(reference));
                    }
                    pending.extend(composition.provenance().inputs().iter().cloned());
                }
                ObjectRef::State(_) => {
                    let state = self
                        .resolve_state(&reference)
                        .ok_or_else(|| PipelineError::InputUnresolved(reference.clone()))?;
                    if self.registry.state(state.contract()).is_none()
                        || state.criteria().iter().any(|criterion| {
                            self.registry
                                .state_criterion(criterion.reference())
                                .is_none()
                        })
                    {
                        return Err(PipelineError::ProvenanceIncomplete(reference));
                    }
                    pending.extend(state.provenance().inputs().iter().cloned());
                }
                ObjectRef::Evaluation(_) => {
                    let evaluation = self
                        .resolve_evaluation(&reference)
                        .ok_or_else(|| PipelineError::InputUnresolved(reference.clone()))?;
                    if !evaluation.diagnostics().is_empty()
                        || self.registry.conformance(evaluation.contract()).is_none()
                    {
                        return Err(PipelineError::ProvenanceIncomplete(reference));
                    }
                    pending.extend(evaluation.provenance().inputs().iter().cloned());
                }
                ObjectRef::Transition(_) => {
                    let transition = self
                        .resolve_transition(&reference)
                        .ok_or_else(|| PipelineError::InputUnresolved(reference.clone()))?;
                    if self.registry.transition(transition.contract()).is_none()
                        || transition.conditions().iter().any(|condition| {
                            self.registry
                                .transition_condition(condition.reference())
                                .is_none()
                        })
                    {
                        return Err(PipelineError::ProvenanceIncomplete(reference));
                    }
                    pending.extend(transition.provenance().inputs().iter().cloned());
                }
            }
        }
        Ok(visited.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::registry::{DomainContext, ScalarSchema, SchemaContract, SourceContract};

    #[test]
    fn computed_measurement_survives_a_forced_storage_identity_collision() {
        use crate::contract::registry::{OperatorContract, OperatorKind};
        use crate::model::invalid::InvalidBody;
        let key = |name: &str| VersionedId::new(name, 1).unwrap();
        let mut registry = Registry::new();
        registry
            .register_schema(SchemaContract {
                reference: key("schema"),
                format: ScalarSchema::SignedI64Le,
            })
            .unwrap();
        registry
            .register_source(SourceContract {
                reference: key("source"),
                schema: key("schema"),
                context: DomainContext::new("fixture", key("ticks")).unwrap(),
                unit: "count".into(),
            })
            .unwrap();
        registry
            .register_operator(OperatorContract {
                reference: key("read"),
                kind: OperatorKind::Scalar,
            })
            .unwrap();
        let mut pipeline = Pipeline::new(registry);
        let mut inputs = Vec::new();
        for value in [3_i64, 7] {
            inputs.push(
                pipeline
                    .admit(ObservationRequest {
                        source: key("source"),
                        timestamp: Some(0),
                        payload: value.to_le_bytes().to_vec(),
                        acquisition_context: "fixture".into(),
                    })
                    .unwrap(),
            );
        }
        let first = pipeline
            .measure(MeasurementRequest {
                operator: key("read"),
                inputs: alloc::vec![inputs[0].clone()],
            })
            .unwrap();
        let candidate = Measurement::derive(
            pipeline.registry().operator(&key("read")).unwrap(),
            &[pipeline.resolve_input(&inputs[1]).unwrap()],
        )
        .unwrap();
        let occupied = pipeline.resolve_measurement(&first).unwrap().clone();
        // Force the storage precondition of a collision; do not replace hashing
        // in the fixture or search probabilistically for a native-hash collision.
        pipeline
            .measurements
            .insert(candidate.id(), occupied.clone());
        let error = pipeline
            .measure(MeasurementRequest {
                operator: key("read"),
                inputs: alloc::vec![inputs[1].clone()],
            })
            .unwrap_err();
        assert_eq!(error, PipelineError::IdentityCollision(candidate.id()));
        let receipt = pipeline.rejections().last().unwrap();
        assert_eq!(receipt.error(), &error);
        let invalid = pipeline
            .resolve_invalid(receipt.invalid_reference())
            .unwrap();
        assert!(matches!(
            invalid.body(),
            InvalidBody::Measurement { value: Some(7), .. }
        ));
        assert_eq!(pipeline.measurements.get(&candidate.id()), Some(&occupied));
        assert!(pipeline.failed_candidate.is_none());
    }

    #[test]
    fn collision_guard_rejects_different_content_without_replacing_the_object() {
        let reference = VersionedId::new("source", 1).unwrap();
        let schema = VersionedId::new("i64-le", 1).unwrap();
        let mut registry = Registry::new();
        registry
            .register_schema(SchemaContract {
                reference: schema.clone(),
                format: ScalarSchema::SignedI64Le,
            })
            .unwrap();
        registry
            .register_source(SourceContract {
                reference: reference.clone(),
                schema,
                context: DomainContext::new("fixture", VersionedId::new("tick", 1).unwrap())
                    .unwrap(),
                unit: "count".into(),
            })
            .unwrap();
        let mut pipeline = Pipeline::new(registry);
        let admitted = pipeline
            .admit(ObservationRequest {
                source: reference,
                timestamp: Some(0),
                payload: 7_i64.to_le_bytes().to_vec(),
                acquisition_context: "fixture-record-0".into(),
            })
            .unwrap();
        let observation = pipeline.resolve_observation(&admitted).unwrap();
        let id = observation.id();
        let original = observation.canonical_basis();
        assert_eq!(pipeline.check_identity(id, &original), Ok(()));
        assert_eq!(
            pipeline.check_identity(id, b"conflicting-content"),
            Err(PipelineError::IdentityCollision(id))
        );
        assert_eq!(
            pipeline
                .resolve_observation(&admitted)
                .unwrap()
                .canonical_basis(),
            original
        );
        assert_eq!(pipeline.observation_count(), 1);
    }
}
