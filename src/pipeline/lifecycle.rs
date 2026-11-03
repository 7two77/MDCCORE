use super::Pipeline;
use super::PipelineError;
use crate::contract::evaluation::LifecycleStatus;
use crate::identity::CanonicalSemanticBasis;
use crate::model::lifecycle::*;
use crate::provenance::ObjectRef;
use alloc::collections::BTreeSet;
use alloc::vec::Vec;

impl Pipeline {
    /// Lifecycle epochs advance only on successful invalidation, not wall time.
    pub fn lifecycle_revision(&self) -> u64 {
        self.lifecycle_events.len() as u64
    }
    pub fn lifecycle_events(&self) -> &[LifecycleEvent] {
        &self.lifecycle_events
    }
    pub fn lifecycle_rejections(&self) -> &[LifecycleRejection] {
        &self.lifecycle_rejections
    }
    pub fn resolve_lifecycle_event(&self, id: &LifecycleEventId) -> Option<&LifecycleEvent> {
        self.lifecycle_events.iter().find(|event| event.id() == id)
    }
    pub fn lifecycle_view(&self, reference: &ObjectRef) -> Result<LifecycleView, LifecycleError> {
        self.lifecycle_view_at(reference, self.lifecycle_revision())
    }
    /// Replay lifecycle decisions at an epoch. Object creation within an epoch
    /// is not temporally ordered; this is not historical data-availability replay.
    pub fn lifecycle_view_at(
        &self,
        reference: &ObjectRef,
        revision: u64,
    ) -> Result<LifecycleView, LifecycleError> {
        if revision > self.lifecycle_revision() {
            return Err(LifecycleError::FutureRevision);
        }
        let born = *self
            .lifecycle_birth
            .get(reference)
            .ok_or_else(|| LifecycleError::TargetUnresolved(reference.clone()))?;
        if revision < born {
            return Err(LifecycleError::NotYetAdmitted(reference.clone()));
        }
        let mut status = self.original_status(reference)?;
        let mut causes = Vec::new();
        for event in self.lifecycle_events.iter().take(revision as usize) {
            if event
                .affected()
                .iter()
                .any(|snapshot| snapshot.reference() == reference)
            {
                status = LifecycleStatus::Invalid;
                causes.push(event.id().clone());
            }
        }
        Ok(LifecycleView {
            revision,
            admitted_revision: born,
            status,
            invalidated_by: causes,
        })
    }
    pub fn invalidate(
        &mut self,
        request: InvalidationRequest,
    ) -> Result<LifecycleEventId, LifecycleError> {
        match self.try_invalidate(&request) {
            Ok(id) => Ok(id),
            Err(error) => {
                self.lifecycle_rejections.push(LifecycleRejection {
                    request,
                    error: error.clone(),
                    observed_revision: self.lifecycle_revision(),
                });
                Err(error)
            }
        }
    }
    fn try_invalidate(
        &mut self,
        request: &InvalidationRequest,
    ) -> Result<LifecycleEventId, LifecycleError> {
        if request.request_id.trim().is_empty() {
            return Err(LifecycleError::EmptyRequestId);
        }
        let mut normalized = request.clone();
        normalized.evidence.sort();
        if let Some(previous) = self
            .lifecycle_events
            .iter()
            .find(|e| e.request().request_id == request.request_id)
        {
            return if previous.request() == &normalized {
                Ok(previous.id().clone())
            } else {
                Err(LifecycleError::RequestIdConflict)
            };
        }
        let actual = self.lifecycle_revision();
        if request.expected_revision != actual {
            return Err(LifecycleError::RevisionConflict {
                expected: request.expected_revision,
                actual,
            });
        }
        let definition = self
            .registry
            .lifecycle(&request.contract)
            .cloned()
            .ok_or_else(|| LifecycleError::ContractUnresolved(request.contract.clone()))?;
        let target = self.evaluation_subject(&request.target).ok_or_else(|| {
            if self.resolve_evaluation(&request.target).is_some()
                || self.resolve_invalid(&request.target).is_some()
                || matches!(request.target, ObjectRef::Source(_))
            {
                LifecycleError::UnsupportedTarget(request.target.clone())
            } else {
                LifecycleError::TargetUnresolved(request.target.clone())
            }
        })?;
        if target.kind != definition.target_type {
            return Err(LifecycleError::UnsupportedTarget(request.target.clone()));
        }
        if !target.matches_context(&definition.context)
            || !self.within_lifecycle_scope(&request.target, &definition.context)
        {
            return Err(LifecycleError::ContextMismatch);
        }
        if !definition.allowed_reasons.contains(&request.reason) {
            return Err(LifecycleError::ReasonNotAllowed);
        }
        if normalized.evidence.is_empty() {
            return Err(LifecycleError::EmptyEvidence);
        }
        if normalized
            .evidence
            .windows(2)
            .any(|pair| pair[0] == pair[1])
        {
            return Err(LifecycleError::DuplicateEvidence);
        }
        let mut evidence = Vec::new();
        for reference in &normalized.evidence {
            let subject = self
                .evaluation_subject(reference)
                .ok_or_else(|| LifecycleError::EvidenceInvalid(reference.clone()))?;
            if self.ensure_usable(reference).is_err() {
                return Err(LifecycleError::EvidenceInvalid(reference.clone()));
            }
            if !subject.matches_context(&definition.context)
                || !self.within_lifecycle_scope(reference, &definition.context)
            {
                return Err(LifecycleError::ContextMismatch);
            }
            evidence.push(self.lifecycle_capture(reference)?);
        }
        let mut affected = Vec::new();
        let mut lineage = BTreeSet::new();
        let mut unresolved = BTreeSet::new();
        let mut unresolved_definitions = BTreeSet::new();
        for reference in self.lifecycle_references() {
            let closure = self.lineage_report(&reference);
            if closure.resolved.contains(&request.target) {
                affected.push(self.lifecycle_capture(&reference)?);
                lineage.extend(closure.resolved);
                unresolved.extend(closure.unresolved);
                unresolved_definitions.extend(closure.unresolved_definitions);
            }
        }
        for reference in &normalized.evidence {
            let closure = self.lineage_report(reference);
            lineage.extend(closure.resolved);
            unresolved.extend(closure.unresolved);
            unresolved_definitions.extend(closure.unresolved_definitions);
        }
        let mut event = LifecycleEvent {
            id: LifecycleEventId::new(Vec::new()),
            request: normalized,
            definition,
            revision: actual + 1,
            affected,
            evidence,
            lineage: lineage.into_iter().collect(),
            unresolved: unresolved.into_iter().collect(),
            unresolved_definitions: unresolved_definitions.into_iter().collect(),
        };
        event.seal();
        let id = event.id().clone();
        for snapshot in event.affected() {
            self.lifecycle_invalid.insert(snapshot.reference().clone());
        }
        self.lifecycle_events.push(event);
        Ok(id)
    }
    fn within_lifecycle_scope(
        &self,
        reference: &ObjectRef,
        context: &crate::contract::dynamics::DynamicsContext,
    ) -> bool {
        self.observation_interval(reference).is_ok_and(|bounds| {
            context
                .frame()
                .scope()
                .contains(bounds.start(), bounds.end())
        })
    }
    fn lifecycle_capture(
        &self,
        reference: &ObjectRef,
    ) -> Result<LifecycleObjectSnapshot, LifecycleError> {
        let basis = self
            .evaluation_subject(reference)
            .map(|subject| subject.basis)
            .or_else(|| {
                self.resolve_evaluation(reference)
                    .map(|e| e.canonical_basis())
            })
            .ok_or_else(|| LifecycleError::TargetUnresolved(reference.clone()))?;
        Ok(LifecycleObjectSnapshot {
            reference: reference.clone(),
            semantic_basis: basis,
            status_before: self.lifecycle_view(reference)?.status,
        })
    }
    fn original_status(&self, reference: &ObjectRef) -> Result<LifecycleStatus, LifecycleError> {
        if let Some(evaluation) = self.resolve_evaluation(reference) {
            return Ok(evaluation.status());
        }
        if self.evaluation_subject(reference).is_some() {
            return Ok(LifecycleStatus::Candidate);
        }
        Err(LifecycleError::UnsupportedTarget(reference.clone()))
    }
    fn lifecycle_references(&self) -> Vec<ObjectRef> {
        let mut refs = Vec::new();
        refs.extend(
            self.observations
                .keys()
                .copied()
                .map(ObjectRef::Observation),
        );
        refs.extend(
            self.measurements
                .keys()
                .copied()
                .map(ObjectRef::Measurement),
        );
        refs.extend(self.relations.keys().copied().map(ObjectRef::Relation));
        refs.extend(
            self.compositions
                .keys()
                .copied()
                .map(ObjectRef::Composition),
        );
        refs.extend(self.states.keys().copied().map(ObjectRef::State));
        refs.extend(self.transitions.keys().copied().map(ObjectRef::Transition));
        refs.extend(self.evaluations.keys().copied().map(ObjectRef::Evaluation));
        refs.sort();
        refs
    }
    pub(super) fn track_lifecycle_birth(&mut self, reference: &ObjectRef) {
        let revision = self.lifecycle_revision();
        self.lifecycle_birth
            .entry(reference.clone())
            .or_insert(revision);
    }
    pub(super) fn ensure_usable(&self, reference: &ObjectRef) -> Result<(), PipelineError> {
        if self.lifecycle_invalid.contains(reference) {
            Err(PipelineError::InputInvalidated {
                reference: reference.clone(),
                revision: self.lifecycle_revision(),
                events: self
                    .lifecycle_view(reference)
                    .expect("invalidated stored object")
                    .invalidated_by,
            })
        } else {
            Ok(())
        }
    }
}
