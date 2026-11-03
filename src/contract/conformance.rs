//! Bounded, explicit conformance contracts. No caller-supplied criterion results.
use super::dynamics::{CriterionOrder, DynamicsContext};
use super::registry::VersionedId;
use alloc::vec::Vec;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum TargetType {
    Observation = 0,
    Measurement = 1,
    Relation = 2,
    Composition = 3,
    State = 4,
    Transition = 5,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConformanceCriterionKind {
    TargetExists,
    LineageComplete,
    IdentityCoherent,
    ConstructionPass,
    MemberCount { min: u32, max: u32 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConformanceCriterionContract {
    pub reference: VersionedId,
    pub kind: ConformanceCriterionKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum MissingTargetPolicy {
    Fail = 0,
    Unresolved = 1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConformanceContract {
    pub reference: VersionedId,
    pub target_type: TargetType,
    pub criteria: Vec<VersionedId>,
    pub criterion_order: CriterionOrder,
    pub context: DynamicsContext,
    pub missing_target: MissingTargetPolicy,
}
