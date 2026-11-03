//! Explicit revocation policy for retained immutable semantic objects.
use super::conformance::TargetType;
use super::dynamics::DynamicsContext;
use super::registry::VersionedId;
use alloc::vec::Vec;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
#[repr(u8)]
pub enum InvalidationReason {
    SourceContractViolation = 0,
    ContradictoryTemporalIdentity = 1,
    DerivationContractViolation = 2,
    EvidenceWithdrawn = 3,
}

/// Every declared semantic input, including a criterion witness, is required.
/// Recomputing with alternative evidence creates a new semantic artifact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum InvalidationPropagation {
    RequiredDeclaredInputs = 0,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LifecycleContract {
    pub reference: VersionedId,
    pub target_type: TargetType,
    pub context: DynamicsContext,
    pub allowed_reasons: Vec<InvalidationReason>,
    pub propagation: InvalidationPropagation,
}
