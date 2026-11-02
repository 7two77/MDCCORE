//! The registered technical TEST-REP-01 projection profile.
use super::conformance::TargetType;
use super::dynamics::DynamicsContext;
use super::registry::VersionedId;

/// Declares one readonly source-to-representation mapping. The implementation
/// supports schema TEST-REP-01 version 1; it does not add market interpretation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectionProfile {
    pub reference: VersionedId,
    pub schema: VersionedId,
    pub target_type: TargetType,
    pub conformance: VersionedId,
    pub context: DynamicsContext,
}
