//! Immutable evaluation artifacts, including retained invalid diagnostics.
use crate::contract::conformance::*;
use crate::contract::dynamics::DynamicsContext;
use crate::contract::encoding::{put_bytes, put_dynamics_context, put_object_ref, put_reference};
use crate::contract::evaluation::{ConformanceResult, LifecycleStatus};
use crate::contract::registry::VersionedId;
use crate::identity::{CanonicalSemanticBasis, IdentityAdapter};
use crate::provenance::{DerivationProvenance, EvaluationId, ObjectRef};
use alloc::vec::Vec;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvaluationRequest {
    pub contract: VersionedId,
    pub target: ObjectRef,
    pub context: DynamicsContext,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EvaluationDiagnostic {
    ContractUnresolved,
    TargetUnresolved,
    InvalidTargetType,
    ContextMismatch,
    ProvenanceIncomplete,
    CriterionUnresolved,
    TargetInvalid,
    TargetInvalidated,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CriterionAssessment {
    pub(crate) definition: ConformanceCriterionContract,
    pub(crate) result: ConformanceResult,
}
impl CriterionAssessment {
    pub fn reference(&self) -> &VersionedId {
        &self.definition.reference
    }
    pub fn kind(&self) -> &ConformanceCriterionKind {
        &self.definition.kind
    }
    pub fn result(&self) -> ConformanceResult {
        self.result
    }
}
pub(crate) struct EvaluationData {
    pub request: EvaluationRequest,
    pub definition: Option<ConformanceContract>,
    pub criteria: Vec<CriterionAssessment>,
    pub target_basis: Option<Vec<u8>>,
    pub result: Option<ConformanceResult>,
    pub status: LifecycleStatus,
    pub diagnostics: Vec<EvaluationDiagnostic>,
    pub lifecycle: Option<crate::model::lifecycle::LifecycleView>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConformanceEvaluation {
    id: u64,
    request: EvaluationRequest,
    definition: Option<ConformanceContract>,
    criteria: Vec<CriterionAssessment>,
    target_basis: Option<Vec<u8>>,
    result: Option<ConformanceResult>,
    status: LifecycleStatus,
    diagnostics: Vec<EvaluationDiagnostic>,
    lifecycle: Option<crate::model::lifecycle::LifecycleView>,
    provenance: DerivationProvenance,
}
impl ConformanceEvaluation {
    pub(crate) fn new(data: EvaluationData) -> Self {
        let provenance = DerivationProvenance::new(
            data.request.contract.clone(),
            alloc::vec![data.request.target.clone()],
        );
        let mut artifact = Self {
            id: 0,
            request: data.request,
            definition: data.definition,
            criteria: data.criteria,
            target_basis: data.target_basis,
            result: data.result,
            status: data.status,
            diagnostics: data.diagnostics,
            lifecycle: data.lifecycle,
            provenance,
        };
        artifact.id = IdentityAdapter::derive_identity(&artifact);
        artifact
    }
    pub fn id(&self) -> u64 {
        self.id
    }
    pub fn key(&self) -> EvaluationId {
        EvaluationId::for_artifact(self.id, self.canonical_basis())
    }
    #[cfg(test)]
    pub(crate) fn force_digest_for_test(&mut self, digest: u64) {
        self.id = digest;
    }
    pub fn request(&self) -> &EvaluationRequest {
        &self.request
    }
    pub fn contract(&self) -> &VersionedId {
        &self.request.contract
    }
    pub fn target(&self) -> &ObjectRef {
        &self.request.target
    }
    pub fn context(&self) -> &DynamicsContext {
        &self.request.context
    }
    pub fn criteria(&self) -> &[CriterionAssessment] {
        &self.criteria
    }
    pub fn result(&self) -> Option<ConformanceResult> {
        self.result
    }
    pub fn status(&self) -> LifecycleStatus {
        self.status
    }
    pub fn lifecycle_context(&self) -> Option<&crate::model::lifecycle::LifecycleView> {
        self.lifecycle.as_ref()
    }
    pub fn diagnostics(&self) -> &[EvaluationDiagnostic] {
        &self.diagnostics
    }
    pub fn target_basis(&self) -> Option<&[u8]> {
        self.target_basis.as_deref()
    }
    pub fn provenance(&self) -> &DerivationProvenance {
        &self.provenance
    }
}
impl CanonicalSemanticBasis for ConformanceEvaluation {
    fn canonical_basis(&self) -> Vec<u8> {
        let mut bytes = b"MDC-SCALAR-001/evaluation/v1\0".to_vec();
        put_reference(&mut bytes, self.contract());
        put_object_ref(&mut bytes, self.target());
        put_dynamics_context(&mut bytes, self.context());
        if let Some(definition) = &self.definition {
            bytes.push(1);
            bytes.push(definition.target_type as u8);
            bytes.push(definition.criterion_order as u8);
            bytes.push(definition.missing_target as u8);
            put_dynamics_context(&mut bytes, &definition.context);
            bytes.extend_from_slice(&(definition.criteria.len() as u64).to_le_bytes());
            for reference in &definition.criteria {
                put_reference(&mut bytes, reference);
            }
        } else {
            bytes.push(0);
        }
        bytes.extend_from_slice(&(self.criteria.len() as u64).to_le_bytes());
        for criterion in &self.criteria {
            put_reference(&mut bytes, criterion.reference());
            match criterion.kind() {
                ConformanceCriterionKind::TargetExists => bytes.push(0),
                ConformanceCriterionKind::LineageComplete => bytes.push(1),
                ConformanceCriterionKind::IdentityCoherent => bytes.push(2),
                ConformanceCriterionKind::ConstructionPass => bytes.push(3),
                ConformanceCriterionKind::MemberCount { min, max } => {
                    bytes.push(4);
                    bytes.extend_from_slice(&min.to_le_bytes());
                    bytes.extend_from_slice(&max.to_le_bytes());
                }
            }
            bytes.push(criterion.result() as u8);
        }
        match &self.target_basis {
            Some(basis) => {
                bytes.push(1);
                put_bytes(&mut bytes, basis);
            }
            None => bytes.push(0),
        }
        bytes.push(self.result.map_or(3, |result| result as u8));
        bytes.push(self.status as u8);
        bytes.extend_from_slice(&(self.diagnostics.len() as u64).to_le_bytes());
        for diagnostic in &self.diagnostics {
            bytes.push(match diagnostic {
                EvaluationDiagnostic::ContractUnresolved => 0,
                EvaluationDiagnostic::TargetUnresolved => 1,
                EvaluationDiagnostic::InvalidTargetType => 2,
                EvaluationDiagnostic::ContextMismatch => 3,
                EvaluationDiagnostic::ProvenanceIncomplete => 4,
                EvaluationDiagnostic::CriterionUnresolved => 5,
                EvaluationDiagnostic::TargetInvalid => 6,
                EvaluationDiagnostic::TargetInvalidated => 7,
            });
        }
        if let Some(context) = &self.lifecycle {
            bytes.extend_from_slice(b"MDC-LIFECYCLE-001/evaluation-context/v1\0");
            bytes.extend_from_slice(&context.revision.to_le_bytes());
            bytes.extend_from_slice(&context.admitted_revision.to_le_bytes());
            bytes.push(context.status as u8);
            bytes.extend_from_slice(&(context.invalidated_by.len() as u64).to_le_bytes());
            for event in &context.invalidated_by {
                put_bytes(&mut bytes, event.canonical_key());
            }
        }
        put_reference(&mut bytes, self.provenance.contract());
        for input in self.provenance.inputs() {
            put_object_ref(&mut bytes, input);
        }
        bytes
    }
}
