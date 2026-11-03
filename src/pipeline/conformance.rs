use super::{Pipeline, PipelineError};
use crate::contract::conformance::*;
use crate::contract::dynamics::DynamicsContext;
use crate::contract::evaluation::{
    aggregate_results, ConformanceResult as R, LifecycleStatus as S,
};
use crate::contract::registry::DomainContext;
use crate::contract::structure::RelationContext;
use crate::identity::{CanonicalSemanticBasis, IdentityAdapter};
pub use crate::model::evaluation::EvaluationRequest;
use crate::model::evaluation::{
    ConformanceEvaluation, CriterionAssessment, EvaluationData, EvaluationDiagnostic as D,
};
use crate::provenance::{EvaluationId, ObjectRef};
use alloc::vec::Vec;

pub(super) struct Subject {
    pub(super) kind: TargetType,
    id: u64,
    pub(super) basis: Vec<u8>,
    domain: DomainContext,
    frame: Option<RelationContext>,
    dynamics: Option<DynamicsContext>,
    result: R,
    count: Option<usize>,
}
impl Subject {
    pub(super) fn matches_context(&self, context: &DynamicsContext) -> bool {
        &self.domain == context.frame().domain()
            && self
                .frame
                .as_ref()
                .is_none_or(|frame| frame == context.frame())
            && self
                .dynamics
                .as_ref()
                .is_none_or(|versions| versions == context)
    }
}
impl Pipeline {
    /// Preserve existing numeric aliases when available; collisions use a full key.
    pub fn evaluate(&mut self, request: EvaluationRequest) -> Result<ObjectRef, PipelineError> {
        self.evaluate_with_reference_policy(request, false)
    }
    /// Use canonical full-key references from the first insertion, independently
    /// of numeric alias availability or insertion order of colliding evaluations.
    pub fn evaluate_keyed(
        &mut self,
        request: EvaluationRequest,
    ) -> Result<ObjectRef, PipelineError> {
        self.evaluate_with_reference_policy(request, true)
    }
    fn evaluate_with_reference_policy(
        &mut self,
        request: EvaluationRequest,
        keyed: bool,
    ) -> Result<ObjectRef, PipelineError> {
        let subject = self.evaluation_subject(&request.target);
        let definition = self.registry.conformance(&request.contract).cloned();
        let lifecycle = self
            .lifecycle_view(&request.target)
            .ok()
            .filter(|v| !v.invalidated_by.is_empty());
        let mut data = EvaluationData {
            target_basis: subject.as_ref().map(|s| s.basis.clone()).or_else(|| {
                self.resolve_invalid(&request.target)
                    .map(|o| o.canonical_basis())
            }),
            request,
            definition: definition.clone(),
            criteria: Vec::new(),
            result: None,
            status: S::Invalid,
            diagnostics: Vec::new(),
            lifecycle,
        };
        if let Some(contract) = definition {
            let mut precondition_result = None;
            let requested_type = match &data.request.target {
                ObjectRef::Observation(_) | ObjectRef::InvalidObservation(_) => {
                    Some(TargetType::Observation)
                }
                ObjectRef::Measurement(_) | ObjectRef::InvalidMeasurement(_) => {
                    Some(TargetType::Measurement)
                }
                ObjectRef::Relation(_) | ObjectRef::InvalidRelation(_) => {
                    Some(TargetType::Relation)
                }
                ObjectRef::Composition(_) | ObjectRef::InvalidComposition(_) => {
                    Some(TargetType::Composition)
                }
                ObjectRef::State(_) | ObjectRef::InvalidState(_) => Some(TargetType::State),
                ObjectRef::Transition(_) | ObjectRef::InvalidTransition(_) => {
                    Some(TargetType::Transition)
                }
                _ => None,
            };
            if requested_type != Some(contract.target_type) {
                data.diagnostics.push(D::InvalidTargetType);
                precondition_result = Some(R::Fail);
            }
            if data.request.context != contract.context {
                data.diagnostics.push(D::ContextMismatch);
                precondition_result = Some(R::Fail);
            }
            if subject.is_some() && self.ensure_usable(&data.request.target).is_err() {
                data.diagnostics.push(D::TargetInvalidated);
                precondition_result = Some(R::Fail);
            }
            if let Some(subject) = &subject {
                if subject.kind != contract.target_type
                    && !data.diagnostics.contains(&D::InvalidTargetType)
                {
                    data.diagnostics.push(D::InvalidTargetType);
                    precondition_result = Some(R::Fail);
                }
                let context = &contract.context;
                if &subject.domain != context.frame().domain()
                    || subject
                        .frame
                        .as_ref()
                        .is_some_and(|frame| frame != context.frame())
                    || subject
                        .dynamics
                        .as_ref()
                        .is_some_and(|versions| versions != context)
                {
                    data.diagnostics.push(D::ContextMismatch);
                    precondition_result = Some(R::Fail);
                }
                match self.observation_interval(&data.request.target) {
                    Ok(bounds)
                        if context
                            .frame()
                            .scope()
                            .contains(bounds.start(), bounds.end()) => {}
                    Ok(_) => {
                        data.diagnostics.push(D::ContextMismatch);
                        precondition_result = Some(R::Fail);
                    }
                    Err(_) => {
                        data.diagnostics.push(D::ProvenanceIncomplete);
                        precondition_result = Some(R::Fail);
                    }
                }
            } else if self.resolve_invalid(&data.request.target).is_some() {
                data.diagnostics.push(D::TargetInvalid);
                precondition_result = Some(R::Fail);
            } else {
                let supported = matches!(
                    data.request.target,
                    ObjectRef::Observation(_)
                        | ObjectRef::Measurement(_)
                        | ObjectRef::Relation(_)
                        | ObjectRef::Composition(_)
                        | ObjectRef::State(_)
                        | ObjectRef::Transition(_)
                        | ObjectRef::InvalidObservation(_)
                        | ObjectRef::InvalidMeasurement(_)
                        | ObjectRef::InvalidRelation(_)
                        | ObjectRef::InvalidComposition(_)
                        | ObjectRef::InvalidState(_)
                        | ObjectRef::InvalidTransition(_)
                );
                if supported {
                    data.diagnostics.push(D::TargetUnresolved);
                    if precondition_result.is_none() {
                        precondition_result = Some(match contract.missing_target {
                            MissingTargetPolicy::Fail => R::Fail,
                            MissingTargetPolicy::Unresolved => R::Unresolved,
                        });
                    }
                } else {
                    data.diagnostics.push(D::InvalidTargetType);
                    precondition_result = Some(R::Fail);
                }
            }
            for reference in &contract.criteria {
                let Some(criterion) = self.registry.conformance_criterion(reference).cloned()
                else {
                    data.diagnostics.push(D::CriterionUnresolved);
                    continue;
                };
                let result = if let Some(result) = precondition_result {
                    result
                } else {
                    let subject = subject
                        .as_ref()
                        .expect("resolved target after precondition checks");
                    match criterion.kind {
                        ConformanceCriterionKind::TargetExists => R::Pass,
                        ConformanceCriterionKind::LineageComplete => {
                            if self.trace(&data.request.target).is_ok() {
                                R::Pass
                            } else {
                                R::Fail
                            }
                        }
                        ConformanceCriterionKind::IdentityCoherent => {
                            if IdentityAdapter::derive_identity_from_basis(&subject.basis)
                                == subject.id
                            {
                                R::Pass
                            } else {
                                R::Fail
                            }
                        }
                        ConformanceCriterionKind::ConstructionPass => subject.result,
                        ConformanceCriterionKind::MemberCount { min, max } => {
                            if subject.count.is_some_and(|count| {
                                u128::from(min) <= count as u128 && count as u128 <= u128::from(max)
                            }) {
                                R::Pass
                            } else {
                                R::Fail
                            }
                        }
                    }
                };
                data.criteria.push(CriterionAssessment {
                    definition: criterion,
                    result,
                });
            }
            if !data.diagnostics.contains(&D::CriterionUnresolved) {
                data.result = aggregate_results(
                    &data.criteria.iter().map(|c| c.result()).collect::<Vec<_>>(),
                )
                .ok();
            }
            data.status = if !data.diagnostics.is_empty() {
                S::Invalid
            } else if data.result == Some(R::Pass) {
                S::Conformant
            } else {
                S::Candidate
            };
        } else {
            data.diagnostics.push(D::ContractUnresolved);
        }
        let artifact = ConformanceEvaluation::new(data);
        #[cfg(test)]
        let artifact = {
            let mut artifact = artifact;
            if let Some(digest) = self.evaluation_digest_override {
                artifact.force_digest_for_test(digest);
            }
            artifact
        };
        self.retain_evaluation(artifact, keyed)
    }
    fn retain_evaluation(
        &mut self,
        artifact: ConformanceEvaluation,
        keyed: bool,
    ) -> Result<ObjectRef, PipelineError> {
        let key = artifact.key();
        let canonical = ObjectRef::EvaluationKey(key.clone());
        if self.evaluations.contains_key(&key) {
            let primary = self.preferred_evaluation_reference(&key);
            self.ensure_usable(&primary)?;
            return Ok(if keyed { canonical } else { primary });
        }
        let alias_available = if keyed {
            false
        } else {
            match self.check_identity(artifact.id(), key.canonical_key()) {
                Ok(()) => true,
                Err(
                    PipelineError::IdentityCollision(_) | PipelineError::InputInvalidated { .. },
                ) => false,
                Err(error) => return Err(error),
            }
        };
        if alias_available {
            // This reservation is never reassigned, including after invalidation.
            self.evaluation_aliases.insert(artifact.id(), key.clone());
        }
        self.evaluations.insert(key.clone(), artifact);
        let primary = self.preferred_evaluation_reference(&key);
        self.track_lifecycle_birth(&primary);
        Ok(if keyed { canonical } else { primary })
    }
    pub fn resolve_evaluation(&self, reference: &ObjectRef) -> Option<&ConformanceEvaluation> {
        self.evaluation_key(reference)
            .and_then(|key| self.evaluations.get(key))
    }
    pub fn evaluation_key(&self, reference: &ObjectRef) -> Option<&EvaluationId> {
        match reference {
            ObjectRef::Evaluation(id) => self.evaluation_aliases.get(id),
            ObjectRef::EvaluationKey(key) => {
                self.evaluations.get_key_value(key).map(|(key, _)| key)
            }
            _ => None,
        }
    }
    pub fn canonical_evaluation_reference(&self, reference: &ObjectRef) -> Option<ObjectRef> {
        self.evaluation_key(reference)
            .cloned()
            .map(ObjectRef::EvaluationKey)
    }
    /// One primary reference per retained Evaluation, including diagnostics-only results.
    pub fn evaluation_references(&self) -> Vec<ObjectRef> {
        let mut refs: Vec<_> = self
            .evaluations
            .keys()
            .map(|key| self.preferred_evaluation_reference(key))
            .collect();
        refs.sort();
        refs
    }
    /// Return every member, always with its canonical full key. No arbitrary
    /// choice of a bucket member is exposed as digest-only resolution.
    pub fn evaluations_with_digest(&self, digest: u64) -> Vec<ObjectRef> {
        self.evaluations
            .keys()
            .filter(|key| key.digest() == digest)
            .cloned()
            .map(ObjectRef::EvaluationKey)
            .collect()
    }
    pub fn evaluation_count(&self) -> usize {
        self.evaluations.len()
    }
    pub(super) fn preferred_evaluation_reference(&self, key: &EvaluationId) -> ObjectRef {
        if self.evaluation_aliases.get(&key.digest()) == Some(key) {
            ObjectRef::Evaluation(key.digest())
        } else {
            ObjectRef::EvaluationKey(key.clone())
        }
    }
    pub(super) fn normalize_evaluation_reference(&self, reference: &ObjectRef) -> ObjectRef {
        self.evaluation_key(reference)
            .map(|key| self.preferred_evaluation_reference(key))
            .unwrap_or_else(|| reference.clone())
    }
    pub(super) fn evaluation_subject(&self, reference: &ObjectRef) -> Option<Subject> {
        let (kind, id, basis, domain, frame, dynamics, result, count) = match reference {
            ObjectRef::Observation(_) => {
                let x = self.resolve_observation(reference)?;
                (
                    TargetType::Observation,
                    x.id(),
                    x.canonical_basis(),
                    x.context().clone(),
                    None,
                    None,
                    R::Pass,
                    None,
                )
            }
            ObjectRef::Measurement(_) => {
                let x = self.resolve_measurement(reference)?;
                (
                    TargetType::Measurement,
                    x.id(),
                    x.canonical_basis(),
                    x.context().clone(),
                    None,
                    None,
                    R::Pass,
                    None,
                )
            }
            ObjectRef::Relation(_) => {
                let x = self.resolve_relation(reference)?;
                (
                    TargetType::Relation,
                    x.id(),
                    x.canonical_basis(),
                    x.context().domain().clone(),
                    Some(x.context().clone()),
                    None,
                    R::Pass,
                    None,
                )
            }
            ObjectRef::Composition(_) => {
                let x = self.resolve_composition(reference)?;
                (
                    TargetType::Composition,
                    x.id(),
                    x.canonical_basis(),
                    x.context().domain().clone(),
                    Some(x.context().clone()),
                    None,
                    x.constraints_result(),
                    Some(x.members().len()),
                )
            }
            ObjectRef::State(_) => {
                let x = self.resolve_state(reference)?;
                (
                    TargetType::State,
                    x.id(),
                    x.canonical_basis(),
                    x.context().frame().domain().clone(),
                    Some(x.context().frame().clone()),
                    Some(x.context().clone()),
                    x.criteria_result(),
                    None,
                )
            }
            ObjectRef::Transition(_) => {
                let x = self.resolve_transition(reference)?;
                (
                    TargetType::Transition,
                    x.id(),
                    x.canonical_basis(),
                    x.context().frame().domain().clone(),
                    Some(x.context().frame().clone()),
                    Some(x.context().clone()),
                    x.conditions_result(),
                    None,
                )
            }
            _ => return None,
        };
        Some(Subject {
            kind,
            id,
            basis,
            domain,
            frame,
            dynamics,
            result,
            count,
        })
    }
}
