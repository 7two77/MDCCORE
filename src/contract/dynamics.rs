//! Explicit State derivation and Transition contracts for the bounded profile.

use super::registry::VersionedId;
use super::structure::RelationContext;
use alloc::vec::Vec;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DynamicsContext {
    frame: RelationContext,
    calculus_version: VersionedId,
    domain_version: VersionedId,
}

impl DynamicsContext {
    pub fn new(
        frame: RelationContext,
        calculus_version: VersionedId,
        domain_version: VersionedId,
    ) -> Self {
        Self {
            frame,
            calculus_version,
            domain_version,
        }
    }

    pub fn frame(&self) -> &RelationContext {
        &self.frame
    }
    pub fn calculus_version(&self) -> &VersionedId {
        &self.calculus_version
    }
    pub fn domain_version(&self) -> &VersionedId {
        &self.domain_version
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CriterionOrder {
    Ordered,
    Unordered,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StateDefinition {
    CompositionAbstraction,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StateCriterionKind {
    Cardinality { min: u32, max: u32 },
    HasRelation { relation: VersionedId },
    ConstraintPassed { constraint: VersionedId },
    MaxTemporalSpan { ticks: u64 },
    ObservationAfter { source: VersionedId },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateCriterionContract {
    pub reference: VersionedId,
    pub kind: StateCriterionKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateContract {
    pub reference: VersionedId,
    pub composition: VersionedId,
    pub definition: StateDefinition,
    pub criterion_order: CriterionOrder,
    pub criteria: Vec<VersionedId>,
    pub context: DynamicsContext,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransitionConditionKind {
    TemporalOrder { strict: bool },
    SourceCriteriaPass,
    TargetCriteriaPass,
    ObservationAfterTarget { source: VersionedId },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransitionConditionContract {
    pub reference: VersionedId,
    pub kind: TransitionConditionKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransitionContract {
    pub reference: VersionedId,
    pub source_state: VersionedId,
    pub target_state: VersionedId,
    pub condition_order: CriterionOrder,
    pub conditions: Vec<VersionedId>,
    pub context: DynamicsContext,
}
