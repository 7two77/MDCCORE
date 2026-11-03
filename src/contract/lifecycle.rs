//! Explicit revocation policy for retained immutable semantic objects.
use super::conformance::TargetType;
use super::dynamics::DynamicsContext;
use super::registry::VersionedId;
use alloc::vec::Vec;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum InvalidationReason {
    SourceContractViolation,
    ContradictoryTemporalIdentity,
    DerivationContractViolation,
    EvidenceWithdrawn,
}

/// Every declared semantic input, including a criterion witness, is required.
/// Recomputing with alternative evidence creates a new semantic artifact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvalidationPropagation {
    RequiredDeclaredInputs,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LifecycleContract {
    pub reference: VersionedId,
    pub target_type: TargetType,
    pub context: DynamicsContext,
    pub allowed_reasons: Vec<InvalidationReason>,
    pub propagation: InvalidationPropagation,
}
