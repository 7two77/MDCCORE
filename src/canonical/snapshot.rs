use crate::identity::CanonicalSemanticBasis;
use crate::identity::IdentityAdapter;
use alloc::vec::Vec;

pub struct Snapshot {
    pub identity: u64,
    pub semantic_basis: Vec<u8>,
}

pub fn snapshot<T: CanonicalSemanticBasis>(obj: &T) -> Snapshot {
    let semantic_basis = obj.canonical_basis();
    Snapshot {
        identity: IdentityAdapter::derive_identity_from_basis(&semantic_basis),
        semantic_basis,
    }
}

/// Explicit protocol tagging for transport; this does not admit or rekey objects.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfiledSnapshot {
    pub identity: crate::identity::ProfiledIdentity,
    pub semantic_basis: Vec<u8>,
}

pub fn snapshot_with_protocol<T: CanonicalSemanticBasis>(
    obj: &T,
    protocol: crate::identity::IdentityProtocol,
) -> ProfiledSnapshot {
    let semantic_basis = obj.canonical_basis();
    ProfiledSnapshot {
        identity: protocol.identify(&semantic_basis),
        semantic_basis,
    }
}

/// Verified rebinding of a captured byte artifact. Embedded legacy graph
/// references remain legacy; a portable semantic graph requires explicit replay.
pub fn migrate_legacy_snapshot(
    legacy: &Snapshot,
) -> Result<ProfiledSnapshot, LegacySnapshotMismatch> {
    if IdentityAdapter::derive_identity_from_basis(&legacy.semantic_basis) != legacy.identity {
        return Err(LegacySnapshotMismatch);
    }
    Ok(ProfiledSnapshot {
        identity: crate::identity::IdentityProtocol::PortableV1.identify(&legacy.semantic_basis),
        semantic_basis: legacy.semantic_basis.clone(),
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LegacySnapshotMismatch;
