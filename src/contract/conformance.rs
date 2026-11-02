//! Bounded, explicit conformance contracts. No caller-supplied criterion results.
use super::dynamics::{CriterionOrder, DynamicsContext};
use super::registry::VersionedId;
use alloc::vec::Vec;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetType {
    Observation,
    Measurement,
    Relation,
    Composition,
    State,
    Transition,
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
pub enum MissingTargetPolicy {
    Fail,
    Unresolved,
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
