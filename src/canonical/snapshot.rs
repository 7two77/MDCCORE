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
