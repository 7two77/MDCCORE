//! Projection reads calculus objects; no representation-to-calculus mapping is registered.
use super::Pipeline;
use crate::contract::conformance::TargetType;
use crate::contract::evaluation::{ConformanceResult, LifecycleStatus};
use crate::contract::registry::VersionedId;
use crate::identity::CanonicalSemanticBasis;
use crate::model::representation::Representation;
use crate::provenance::ObjectRef;
use alloc::vec::Vec;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectionRequest {
    pub profile: VersionedId,
    pub target: ObjectRef,
    pub evaluation: ObjectRef,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectionError {
    ProfileUnresolved,
    TargetUnresolved,
    InvalidTargetType,
    EvaluationUnresolved,
    ConformanceRequired,
    EvaluationMismatch,
    BackflowForbidden,
    InvalidatedInput(ObjectRef),
}
impl Pipeline {
    pub fn project(&self, request: ProjectionRequest) -> Result<Representation, ProjectionError> {
        let profile = self
            .registry()
            .projection(&request.profile)
            .ok_or(ProjectionError::ProfileUnresolved)?;
        for reference in [&request.target, &request.evaluation] {
            self.ensure_usable(reference)
                .map_err(|_| ProjectionError::InvalidatedInput(reference.clone()))?;
        }
        let (target_type, basis) = self.projection_source(&request.target)?;
        if target_type != profile.target_type {
            return Err(ProjectionError::InvalidTargetType);
        }
        let evaluation = self
            .resolve_evaluation(&request.evaluation)
            .ok_or(ProjectionError::EvaluationUnresolved)?;
        if evaluation.result() != Some(ConformanceResult::Pass)
            || evaluation.status() != LifecycleStatus::Conformant
            || !evaluation.diagnostics().is_empty()
            || self.registry().conformance(evaluation.contract()).is_none()
        {
            return Err(ProjectionError::ConformanceRequired);
        }
        if evaluation.contract() != &profile.conformance
            || evaluation.target() != &request.target
            || evaluation.context() != &profile.context
            || evaluation.target_basis() != Some(basis.as_slice())
        {
            return Err(ProjectionError::EvaluationMismatch);
        }
        Ok(Representation::new(
            profile,
            request.target,
            request.evaluation,
            basis,
            evaluation.canonical_basis(),
        ))
    }
    /// This bounded profile has no admissible reverse transformation contract.
    /// The rejection is explicit and cannot import an external label or snapshot.
    pub fn apply_representation(
        &self,
        _representation: &Representation,
    ) -> Result<(), ProjectionError> {
        Err(ProjectionError::BackflowForbidden)
    }
    fn projection_source(
        &self,
        reference: &ObjectRef,
    ) -> Result<(TargetType, Vec<u8>), ProjectionError> {
        let source = match reference {
            ObjectRef::Observation(_) => self
                .resolve_observation(reference)
                .map(|x| (TargetType::Observation, x.canonical_basis())),
            ObjectRef::Measurement(_) => self
                .resolve_measurement(reference)
                .map(|x| (TargetType::Measurement, x.canonical_basis())),
            ObjectRef::Relation(_) => self
                .resolve_relation(reference)
                .map(|x| (TargetType::Relation, x.canonical_basis())),
            ObjectRef::Composition(_) => self
                .resolve_composition(reference)
                .map(|x| (TargetType::Composition, x.canonical_basis())),
            ObjectRef::State(_) => self
                .resolve_state(reference)
                .map(|x| (TargetType::State, x.canonical_basis())),
            ObjectRef::Transition(_) => self
                .resolve_transition(reference)
                .map(|x| (TargetType::Transition, x.canonical_basis())),
            _ => return Err(ProjectionError::InvalidTargetType),
        };
        source.ok_or(ProjectionError::TargetUnresolved)
    }
}
