use alloc::vec::Vec;
use crate::identity::CanonicalSemanticBasis;
use crate::identity::IdentityAdapter;

pub struct Snapshot {
    pub identity: u64,
    pub semantic_basis: Vec<u8>,
}

pub fn snapshot<T: CanonicalSemanticBasis>(obj: &T) -> Snapshot {
    Snapshot {
        identity: IdentityAdapter::derive_identity(obj),
        semantic_basis: obj.canonical_basis().to_vec(),
    }
}