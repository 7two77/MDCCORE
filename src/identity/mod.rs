use alloc::vec::Vec;
use core::hash::{Hash, Hasher};

mod portable;
pub use portable::{
    canonical_reference_v1, portable_v1_preimage, IdentityProtocol, ProfiledIdentity,
    PORTABLE_V1_HEADER,
};

/// Supplies an owned copy of the object's current semantic identity basis.
///
/// Implementations must return equivalent bytes for equivalent semantic input.
/// Ownership allows computed representations to be freed without leaking memory.
pub trait CanonicalSemanticBasis {
    fn canonical_basis(&self) -> Vec<u8>;
}

#[derive(Default)]
pub struct FnvHasher {
    state: u64,
}

impl Hasher for FnvHasher {
    fn finish(&self) -> u64 {
        self.state
    }

    fn write(&mut self, bytes: &[u8]) {
        let mut hash = if self.state == 0 {
            0xcbf29ce484222325
        } else {
            self.state
        };
        for &b in bytes {
            hash ^= b as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        self.state = hash;
    }
}

pub struct IdentityAdapter;

impl IdentityAdapter {
    pub fn derive_identity<T: CanonicalSemanticBasis>(obj: &T) -> u64 {
        Self::derive_identity_from_basis(&obj.canonical_basis())
    }

    /// Hashes captured basis bytes using the existing identity protocol.
    ///
    /// This deliberately retains Rust's native `Hash` encoding for compatibility.
    /// The result is not a portable encoding or a guarantee of unique identity.
    pub fn derive_identity_from_basis(basis: &[u8]) -> u64 {
        let mut hasher = FnvHasher::default();
        basis.hash(&mut hasher);
        hasher.finish()
    }
}
