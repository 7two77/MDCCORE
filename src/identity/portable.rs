//! Versioned, platform-independent digest of captured canonical bytes.
use super::IdentityAdapter;
use alloc::vec::Vec;

/// Immutable choice for a Pipeline. Legacy is retained for existing graphs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum IdentityProtocol {
    #[default]
    LegacyRustHashFnv64,
    PortableV1,
}

impl IdentityProtocol {
    pub const fn name(self) -> &'static str {
        match self {
            Self::LegacyRustHashFnv64 => "legacy-rust-hash-fnv64",
            Self::PortableV1 => "MDC-ID-FNV1A64-LE-v1",
        }
    }

    /// Unknown versions never fall back to legacy or to the latest version.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "legacy-rust-hash-fnv64" => Some(Self::LegacyRustHashFnv64),
            "MDC-ID-FNV1A64-LE-v1" => Some(Self::PortableV1),
            _ => None,
        }
    }

    pub fn derive_from_basis(self, basis: &[u8]) -> u64 {
        match self {
            Self::LegacyRustHashFnv64 => IdentityAdapter::derive_identity_from_basis(basis),
            Self::PortableV1 => {
                // Exactly one continuous FNV-1a state, even when it becomes zero.
                let mut hash = 0xcbf29ce484222325_u64;
                let length = (basis.len() as u64).to_le_bytes();
                for byte in PORTABLE_V1_HEADER.iter().chain(length.iter()).chain(basis) {
                    hash ^= u64::from(*byte);
                    hash = hash.wrapping_mul(0x100000001b3);
                }
                hash
            }
        }
    }

    pub fn identify(self, basis: &[u8]) -> ProfiledIdentity {
        ProfiledIdentity {
            protocol: self,
            digest: self.derive_from_basis(basis),
        }
    }
}

/// Magic (7 bytes), protocol version u16 LE, hash code 1, basis codec code 1.
pub const PORTABLE_V1_HEADER: &[u8] = b"MDC-ID\0\x01\x00\x01\x01";

/// The exact hash preimage. No Rust Hash, Debug, native width or native endian.
pub fn portable_v1_preimage(basis: &[u8]) -> Vec<u8> {
    let mut bytes = PORTABLE_V1_HEADER.to_vec();
    bytes.extend_from_slice(&(basis.len() as u64).to_le_bytes());
    bytes.extend_from_slice(basis);
    bytes
}

/// Protocol is part of an external identity handle; digest alone is ambiguous.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProfiledIdentity {
    protocol: IdentityProtocol,
    digest: u64,
}
impl ProfiledIdentity {
    pub fn protocol(self) -> IdentityProtocol {
        self.protocol
    }
    pub fn digest(self) -> u64 {
        self.digest
    }
}

/// Frozen typed-reference codec for v1, interpreted within a Pipeline protocol.
pub fn canonical_reference_v1(reference: &crate::provenance::ObjectRef) -> Vec<u8> {
    let mut bytes = Vec::new();
    crate::contract::encoding::put_object_ref(&mut bytes, reference);
    bytes
}
