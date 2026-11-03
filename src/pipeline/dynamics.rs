use alloc::vec::Vec;

use super::{Pipeline, PipelineError, RejectedRequest};
use crate::contract::registry::VersionedId;
use crate::contract::structure::TemporalScope;
use crate::identity::CanonicalSemanticBasis;
use crate::model::state::{AdmittedState, ObservationFact, ResolvedComposition};
use crate::model::transition::{AdmittedTransition, ResolvedStates};
use crate::provenance::ObjectRef;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateRequest {
    pub contract: VersionedId,
    pub composition: ObjectRef,
    pub observation_horizon: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransitionRequest {
    pub contract: VersionedId,
    pub source: ObjectRef,
    pub target: ObjectRef,
    pub observation_horizon: Option<u64>,
}

impl Pipeline {
    pub fn derive_state(&mut self, request: StateRequest) -> Result<ObjectRef, PipelineError> {
        match self.try_derive_state(&request) {
            Ok(reference) => {
                self.track_lifecycle_birth(&reference);
                Ok(reference)
            }
            Err(error) => {
                self.record_rejection(RejectedRequest::State(request), error.clone());
                Err(error)
            }
        }
    }

    fn try_derive_state(&mut self, request: &StateRequest) -> Result<ObjectRef, PipelineError> {
        let contract = self
            .registry
            .state(&request.contract)
            .ok_or_else(|| PipelineError::StateContractUnresolved(request.contract.clone()))?;
        if !matches!(request.composition, ObjectRef::Composition(_)) {
            return Err(PipelineError::InvalidInputType(request.composition.clone()));
        }
        self.ensure_usable(&request.composition)?;
        let composition = self
            .resolve_composition(&request.composition)
            .ok_or_else(|| PipelineError::InputUnresolved(request.composition.clone()))?;
        let interval = self.observation_interval(&request.composition)?;
        let relation_contracts = composition
            .relations()
            .iter()
            .map(|reference| {
                self.resolve_relation(reference)
                    .map(|relation| relation.contract().clone())
                    .ok_or_else(|| PipelineError::ProvenanceIncomplete(reference.clone()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let input = ResolvedComposition {
            composition: composition.clone(),
            interval,
            relation_contracts,
        };
        let criteria = contract
            .criteria
            .iter()
            .map(|reference| {
                self.registry
                    .state_criterion(reference)
                    .cloned()
                    .ok_or_else(|| PipelineError::StateCriterionUnresolved(reference.clone()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut state = AdmittedState::derive(
            contract,
            &input,
            request.observation_horizon,
            criteria,
            &self.observation_facts(),
        )
        .map_err(PipelineError::State)?;
        state.seal_identity(self.identity_protocol);
        let id = state.id();
        if let Err(error) = self.check_identity(id, &state.canonical_basis()) {
            self.failed_candidate = Some(alloc::boxed::Box::new(
                super::invalid::ComputedCandidate::State(state),
            ));
            return Err(error);
        }
        self.states.entry(id).or_insert(state);
        Ok(ObjectRef::State(id))
    }

    pub fn transition(&mut self, request: TransitionRequest) -> Result<ObjectRef, PipelineError> {
        match self.try_transition(&request) {
            Ok(reference) => {
                self.track_lifecycle_birth(&reference);
                Ok(reference)
            }
            Err(error) => {
                self.record_rejection(RejectedRequest::Transition(request), error.clone());
                Err(error)
            }
        }
    }

    fn try_transition(&mut self, request: &TransitionRequest) -> Result<ObjectRef, PipelineError> {
        let contract = self
            .registry
            .transition(&request.contract)
            .ok_or_else(|| PipelineError::TransitionContractUnresolved(request.contract.clone()))?;
        for reference in [&request.source, &request.target] {
            if !matches!(reference, ObjectRef::State(_)) {
                return Err(PipelineError::InvalidInputType(reference.clone()));
            }
        }
        self.ensure_usable(&request.source)?;
        self.ensure_usable(&request.target)?;
        let source = self
            .resolve_state(&request.source)
            .ok_or_else(|| PipelineError::InputUnresolved(request.source.clone()))?;
        let target = self
            .resolve_state(&request.target)
            .ok_or_else(|| PipelineError::InputUnresolved(request.target.clone()))?;
        let input = ResolvedStates {
            source: source.clone(),
            target: target.clone(),
            source_bounds: self.observation_interval(&request.source)?,
            target_bounds: self.observation_interval(&request.target)?,
        };
        let conditions = contract
            .conditions
            .iter()
            .map(|reference| {
                self.registry
                    .transition_condition(reference)
                    .cloned()
                    .ok_or_else(|| PipelineError::TransitionConditionUnresolved(reference.clone()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut transition = AdmittedTransition::derive(
            contract,
            &input,
            request.observation_horizon,
            conditions,
            &self.observation_facts(),
        )
        .map_err(PipelineError::Transition)?;
        transition.seal_identity(self.identity_protocol);
        let id = transition.id();
        if let Err(error) = self.check_identity(id, &transition.canonical_basis()) {
            self.failed_candidate = Some(alloc::boxed::Box::new(
                super::invalid::ComputedCandidate::Transition(transition),
            ));
            return Err(error);
        }
        self.transitions.entry(id).or_insert(transition);
        Ok(ObjectRef::Transition(id))
    }

    pub fn resolve_state(&self, reference: &ObjectRef) -> Option<&AdmittedState> {
        match reference {
            ObjectRef::State(id) => self.states.get(id),
            _ => None,
        }
    }

    pub fn resolve_transition(&self, reference: &ObjectRef) -> Option<&AdmittedTransition> {
        match reference {
            ObjectRef::Transition(id) => self.transitions.get(id),
            _ => None,
        }
    }

    pub fn state_count(&self) -> usize {
        self.states.len()
    }

    pub fn transition_count(&self) -> usize {
        self.transitions.len()
    }

    fn observation_facts(&self) -> Vec<ObservationFact> {
        self.observations
            .values()
            .filter(|o| self.ensure_usable(&ObjectRef::Observation(o.id())).is_ok())
            .map(|observation| ObservationFact {
                reference: ObjectRef::Observation(observation.id()),
                source: observation.source_ref().clone(),
                timestamp: observation.timestamp(),
            })
            .collect()
    }

    /// Bounds of all actual facts in the lineage, including criterion witnesses.
    /// This is intentionally distinct from a State's validity interval.
    pub(super) fn observation_interval(
        &self,
        reference: &ObjectRef,
    ) -> Result<TemporalScope, PipelineError> {
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
        TemporalScope::new(minimum, maximum)
            .map_err(|_| PipelineError::ProvenanceIncomplete(reference.clone()))
    }
}
