//! Explicit relation and composition contracts for the bounded scalar profile.

use super::registry::{DomainContext, RegistryError, VersionedId};
use alloc::vec::Vec;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemporalScope {
    start: u64,
    end: u64,
}

impl TemporalScope {
    pub fn new(start: u64, end: u64) -> Result<Self, RegistryError> {
        if start > end {
            return Err(RegistryError::InvalidTemporalScope);
        }
        Ok(Self { start, end })
    }

    pub fn start(&self) -> u64 {
        self.start
    }

    pub fn end(&self) -> u64 {
        self.end
    }

    pub fn contains(&self, min: u64, max: u64) -> bool {
        min <= max && self.start <= min && max <= self.end
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelationContext {
    domain: DomainContext,
    scope: TemporalScope,
    reference_frame: VersionedId,
    causal_context: VersionedId,
}

impl RelationContext {
    pub fn new(
        domain: DomainContext,
        scope: TemporalScope,
        reference_frame: VersionedId,
        causal_context: VersionedId,
    ) -> Self {
        Self {
            domain,
            scope,
            reference_frame,
            causal_context,
        }
    }

    pub fn domain(&self) -> &DomainContext {
        &self.domain
    }

    pub fn scope(&self) -> &TemporalScope {
        &self.scope
    }

    pub fn reference_frame(&self) -> &VersionedId {
        &self.reference_frame
    }

    pub fn causal_context(&self) -> &VersionedId {
        &self.causal_context
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RelationKind {
    Precedes,
    TemporalDistance,
    Adjacent { step: u64, symmetric: bool },
    Proportion,
}

impl RelationKind {
    pub fn is_symmetric(&self) -> bool {
        matches!(
            self,
            Self::Adjacent {
                symmetric: true,
                ..
            }
        )
    }

    /// Ordered numeric operands alone do not establish a directed graph edge.
    pub fn is_directed(&self) -> bool {
        matches!(
            self,
            Self::Precedes
                | Self::TemporalDistance
                | Self::Adjacent {
                    symmetric: false,
                    ..
                }
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelationContract {
    pub reference: VersionedId,
    pub kind: RelationKind,
    pub context: RelationContext,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemberOrder {
    Ordered,
    Unordered,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemberType {
    Observations,
    Measurements,
    ScalarInputs,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConstraintKind {
    Cardinality { min: u32, max: u32 },
    SameUnits,
    Connected,
    Acyclic,
    OrderedChain { relation: VersionedId },
    RequiresRelation { relation: VersionedId },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConstraintContract {
    pub reference: VersionedId,
    pub kind: ConstraintKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositionContract {
    pub reference: VersionedId,
    pub member_order: MemberOrder,
    pub member_type: MemberType,
    pub constraints: Vec<VersionedId>,
    pub context: RelationContext,
}
