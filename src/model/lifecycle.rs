//! Append-only lifecycle records. These do not rewrite calculus snapshots.
use crate::contract::encoding::{
    put_bytes, put_dynamics_context, put_object_ref, put_reference, put_str,
};
use crate::contract::evaluation::LifecycleStatus;
use crate::contract::lifecycle::{InvalidationReason, LifecycleContract};
use crate::contract::registry::VersionedId;
use crate::identity::{CanonicalSemanticBasis, IdentityAdapter};
use crate::provenance::ObjectRef;
use alloc::string::String;
use alloc::vec::Vec;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvalidationRequest {
    pub request_id: String,
    pub contract: VersionedId,
    pub target: ObjectRef,
    pub reason: InvalidationReason,
    /// An unordered, nonempty set of active admitted semantic witnesses.
    pub evidence: Vec<ObjectRef>,
    /// Optimistic concurrency against the lifecycle epoch, not source time.
    pub expected_revision: u64,
}

/// Full canonical key disambiguates equal u64 digests in the journal namespace.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct LifecycleEventId {
    digest: u64,
    canonical_key: Vec<u8>,
}
impl LifecycleEventId {
    pub(crate) fn new(canonical_key: Vec<u8>) -> Self {
        Self {
            digest: IdentityAdapter::derive_identity_from_basis(&canonical_key),
            canonical_key,
        }
    }
    pub fn digest(&self) -> u64 {
        self.digest
    }
    pub fn canonical_key(&self) -> &[u8] {
        &self.canonical_key
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LifecycleObjectSnapshot {
    pub(crate) reference: ObjectRef,
    pub(crate) semantic_basis: Vec<u8>,
    pub(crate) status_before: LifecycleStatus,
}
impl LifecycleObjectSnapshot {
    pub fn reference(&self) -> &ObjectRef {
        &self.reference
    }
    pub fn semantic_basis(&self) -> &[u8] {
        &self.semantic_basis
    }
    pub fn status_before(&self) -> LifecycleStatus {
        self.status_before
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LifecycleEvent {
    pub(crate) id: LifecycleEventId,
    pub(crate) request: InvalidationRequest,
    pub(crate) definition: LifecycleContract,
    pub(crate) revision: u64,
    pub(crate) affected: Vec<LifecycleObjectSnapshot>,
    pub(crate) evidence: Vec<LifecycleObjectSnapshot>,
    pub(crate) lineage: Vec<ObjectRef>,
    pub(crate) unresolved: Vec<ObjectRef>,
    pub(crate) unresolved_definitions: Vec<crate::model::invalid::UnresolvedDefinition>,
}
impl LifecycleEvent {
    pub fn id(&self) -> &LifecycleEventId {
        &self.id
    }
    pub fn request(&self) -> &InvalidationRequest {
        &self.request
    }
    pub fn definition(&self) -> &LifecycleContract {
        &self.definition
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn affected(&self) -> &[LifecycleObjectSnapshot] {
        &self.affected
    }
    pub fn evidence(&self) -> &[LifecycleObjectSnapshot] {
        &self.evidence
    }
    pub fn captured_lineage(&self) -> &[ObjectRef] {
        &self.lineage
    }
    pub fn unresolved_references(&self) -> &[ObjectRef] {
        &self.unresolved
    }
    pub fn unresolved_definitions(&self) -> &[crate::model::invalid::UnresolvedDefinition] {
        &self.unresolved_definitions
    }
    pub(crate) fn seal(&mut self, protocol: crate::identity::IdentityProtocol) {
        let canonical_key = self.canonical_basis();
        self.id = LifecycleEventId {
            digest: protocol.derive_from_basis(&canonical_key),
            canonical_key,
        };
    }
}
impl CanonicalSemanticBasis for LifecycleEvent {
    fn canonical_basis(&self) -> Vec<u8> {
        let mut out = b"MDC-LIFECYCLE-001/invalidation/v1\0".to_vec();
        put_reference(&mut out, &self.definition.reference);
        out.push(self.definition.target_type as u8);
        put_dynamics_context(&mut out, &self.definition.context);
        out.push(self.definition.propagation as u8);
        out.extend_from_slice(&(self.definition.allowed_reasons.len() as u64).to_le_bytes());
        for reason in &self.definition.allowed_reasons {
            out.push(*reason as u8);
        }
        put_str(&mut out, &self.request.request_id);
        put_object_ref(&mut out, &self.request.target);
        out.push(self.request.reason as u8);
        out.extend_from_slice(&self.request.expected_revision.to_le_bytes());
        out.extend_from_slice(&self.revision.to_le_bytes());
        out.push(LifecycleStatus::Invalid as u8);
        for snapshots in [&self.evidence, &self.affected] {
            out.extend_from_slice(&(snapshots.len() as u64).to_le_bytes());
            for snapshot in snapshots {
                put_object_ref(&mut out, &snapshot.reference);
                put_bytes(&mut out, &snapshot.semantic_basis);
                out.push(snapshot.status_before as u8);
            }
        }
        for refs in [&self.lineage, &self.unresolved] {
            out.extend_from_slice(&(refs.len() as u64).to_le_bytes());
            for reference in refs {
                put_object_ref(&mut out, reference);
            }
        }
        out.extend_from_slice(&(self.unresolved_definitions.len() as u64).to_le_bytes());
        for definition in &self.unresolved_definitions {
            out.push(definition.kind as u8);
            put_reference(&mut out, &definition.reference);
        }
        out
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LifecycleView {
    pub revision: u64,
    pub admitted_revision: u64,
    pub status: LifecycleStatus,
    pub invalidated_by: Vec<LifecycleEventId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LifecycleError {
    ContractUnresolved(VersionedId),
    TargetUnresolved(ObjectRef),
    UnsupportedTarget(ObjectRef),
    ContextMismatch,
    ReasonNotAllowed,
    EmptyRequestId,
    EmptyEvidence,
    DuplicateEvidence,
    EvidenceInvalid(ObjectRef),
    RevisionConflict { expected: u64, actual: u64 },
    RequestIdConflict,
    FutureRevision,
    NotYetAdmitted(ObjectRef),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LifecycleRejection {
    pub request: InvalidationRequest,
    pub error: LifecycleError,
    pub observed_revision: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_digest_collision_does_not_equal_two_different_journal_keys() {
        let a = LifecycleEventId {
            digest: 7,
            canonical_key: alloc::vec![1],
        };
        let b = LifecycleEventId {
            digest: 7,
            canonical_key: alloc::vec![2],
        };
        assert_ne!(a, b);
        let mut map = alloc::collections::BTreeMap::new();
        map.insert(a, 1);
        map.insert(b, 2);
        assert_eq!(map.len(), 2);
    }
}
