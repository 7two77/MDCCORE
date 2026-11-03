use alloc::vec::Vec;

use super::{Pipeline, PipelineError, RejectedRequest};
use crate::contract::registry::VersionedId;
use crate::identity::CanonicalSemanticBasis;
use crate::model::composition::AdmittedComposition;
use crate::model::relation::{AdmittedRelation, ResolvedRelationInput};
use crate::provenance::ObjectRef;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelationRequest {
    pub contract: VersionedId,
    pub left: ObjectRef,
    pub right: ObjectRef,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositionRequest {
    pub contract: VersionedId,
    pub members: Vec<ObjectRef>,
    pub relations: Vec<ObjectRef>,
}

impl Pipeline {
    pub fn relate(&mut self, request: RelationRequest) -> Result<ObjectRef, PipelineError> {
        match self.try_relate(&request) {
            Ok(reference) => {
                self.track_lifecycle_birth(&reference);
                Ok(reference)
            }
            Err(error) => {
                self.record_rejection(RejectedRequest::Relation(request), error.clone());
                Err(error)
            }
        }
    }

    fn try_relate(&mut self, request: &RelationRequest) -> Result<ObjectRef, PipelineError> {
        let contract = self
            .registry
            .relation(&request.contract)
            .ok_or_else(|| PipelineError::RelationContractUnresolved(request.contract.clone()))?;
        let left = self.resolve_structure_input(&request.left)?;
        let right = self.resolve_structure_input(&request.right)?;
        let mut relation =
            AdmittedRelation::derive(contract, left, right).map_err(PipelineError::Relation)?;
        relation.seal_identity(self.identity_protocol);
        let id = relation.id();
        if let Err(error) = self.check_identity(id, &relation.canonical_basis()) {
            self.failed_candidate = Some(alloc::boxed::Box::new(
                super::invalid::ComputedCandidate::Relation(relation),
            ));
            return Err(error);
        }
        self.relations.entry(id).or_insert(relation);
        Ok(ObjectRef::Relation(id))
    }

    pub fn compose(&mut self, request: CompositionRequest) -> Result<ObjectRef, PipelineError> {
        match self.try_compose(&request) {
            Ok(reference) => {
                self.track_lifecycle_birth(&reference);
                Ok(reference)
            }
            Err(error) => {
                self.record_rejection(RejectedRequest::Composition(request), error.clone());
                Err(error)
            }
        }
    }

    fn try_compose(&mut self, request: &CompositionRequest) -> Result<ObjectRef, PipelineError> {
        let contract = self
            .registry
            .composition(&request.contract)
            .ok_or_else(|| {
                PipelineError::CompositionContractUnresolved(request.contract.clone())
            })?;
        let constraints = contract
            .constraints
            .iter()
            .map(|reference| {
                self.registry
                    .constraint(reference)
                    .cloned()
                    .ok_or_else(|| PipelineError::ConstraintUnresolved(reference.clone()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let members = request
            .members
            .iter()
            .map(|reference| self.resolve_structure_input(reference))
            .collect::<Result<Vec<_>, _>>()?;
        let relations = request
            .relations
            .iter()
            .map(|reference| {
                if !matches!(reference, ObjectRef::Relation(_)) {
                    return Err(PipelineError::InvalidInputType(reference.clone()));
                }
                self.ensure_usable(reference)?;
                let relation = self
                    .resolve_relation(reference)
                    .ok_or_else(|| PipelineError::InputUnresolved(reference.clone()))?;
                self.trace(reference)?;
                Ok(relation.clone())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut composition =
            AdmittedComposition::derive(contract, members, relations, constraints)
                .map_err(PipelineError::Composition)?;
        composition.seal_identity(self.identity_protocol);
        let id = composition.id();
        if let Err(error) = self.check_identity(id, &composition.canonical_basis()) {
            self.failed_candidate = Some(alloc::boxed::Box::new(
                super::invalid::ComputedCandidate::Composition(composition),
            ));
            return Err(error);
        }
        self.compositions.entry(id).or_insert(composition);
        Ok(ObjectRef::Composition(id))
    }

    pub fn resolve_relation(&self, reference: &ObjectRef) -> Option<&AdmittedRelation> {
        match reference {
            ObjectRef::Relation(id) => self.relations.get(id),
            _ => None,
        }
    }

    pub fn resolve_composition(&self, reference: &ObjectRef) -> Option<&AdmittedComposition> {
        match reference {
            ObjectRef::Composition(id) => self.compositions.get(id),
            _ => None,
        }
    }

    pub fn relation_count(&self) -> usize {
        self.relations.len()
    }

    pub fn composition_count(&self) -> usize {
        self.compositions.len()
    }

    fn resolve_structure_input(
        &self,
        reference: &ObjectRef,
    ) -> Result<ResolvedRelationInput, PipelineError> {
        self.ensure_usable(reference)?;
        match reference {
            ObjectRef::Observation(_) => {
                let object = self
                    .resolve_observation(reference)
                    .ok_or_else(|| PipelineError::InputUnresolved(reference.clone()))?;
                self.trace(reference)?;
                Ok(ResolvedRelationInput {
                    reference: reference.clone(),
                    context: object.context().clone(),
                    unit: object.unit().into(),
                    timestamp: Some(object.timestamp()),
                    value: Some(object.scalar_value()),
                    temporal_bounds: (object.timestamp(), object.timestamp()),
                })
            }
            ObjectRef::Measurement(_) => {
                let object = self
                    .resolve_measurement(reference)
                    .ok_or_else(|| PipelineError::InputUnresolved(reference.clone()))?;
                let trace = self.trace(reference)?;
                let mut minimum = u64::MAX;
                let mut maximum = 0;
                let mut found = false;
                for input in &trace {
                    if let Some(observation) = self.resolve_observation(input) {
                        minimum = minimum.min(observation.timestamp());
                        maximum = maximum.max(observation.timestamp());
                        found = true;
                    }
                }
                if !found {
                    return Err(PipelineError::ProvenanceIncomplete(reference.clone()));
                }
                Ok(ResolvedRelationInput {
                    reference: reference.clone(),
                    context: object.context().clone(),
                    unit: object.unit().into(),
                    timestamp: None,
                    value: Some(object.value()),
                    temporal_bounds: (minimum, maximum),
                })
            }
            _ => Err(PipelineError::InvalidInputType(reference.clone())),
        }
    }
}
