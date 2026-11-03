use core::hash::{Hash, Hasher};

pub trait CanonicalSemanticBasis {
    fn canonical_basis(&self) -> &[u8];
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
        let mut hasher = FnvHasher::default();
        obj.canonical_basis().hash(&mut hasher);
        hasher.finish()
    }
}