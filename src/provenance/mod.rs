use alloc::vec::Vec;

use crate::contract::registry::VersionedId;

pub struct Provenance {
    pub derived_by: &'static str,
    pub inputs: Vec<u64>,
}

/// A typed reference into the registered source and admitted object graph.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ObjectRef {
    Source(VersionedId),
    Observation(u64),
    Measurement(u64),
    Relation(u64),
    Composition(u64),
    State(u64),
    Transition(u64),
    Evaluation(u64),
    InvalidObservation(InvalidId),
    InvalidMeasurement(InvalidId),
    InvalidRelation(InvalidId),
    InvalidComposition(InvalidId),
    InvalidState(InvalidId),
    InvalidTransition(InvalidId),
}

/// Immutable lineage recorded by a production construction operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DerivationProvenance {
    contract: VersionedId,
    inputs: Vec<ObjectRef>,
}

impl DerivationProvenance {
    pub(crate) fn new(contract: VersionedId, inputs: Vec<ObjectRef>) -> Self {
        Self { contract, inputs }
    }

    pub fn contract(&self) -> &VersionedId {
        &self.contract
    }

    pub fn inputs(&self) -> &[ObjectRef] {
        &self.inputs
    }
}

/// An invalid identity retains its complete canonical key, so digest collisions
/// cannot overwrite different failure results. Valid-object IDs are unchanged.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct InvalidId {
    digest: u64,
    canonical_key: Vec<u8>,
}
impl InvalidId {
    pub(crate) fn new(canonical_key: Vec<u8>) -> Self {
        let digest = crate::identity::IdentityAdapter::derive_identity_from_basis(&canonical_key);
        Self {
            digest,
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
#[cfg(test)]
mod invalid_identity_tests {
    use super::*;
    use alloc::collections::BTreeMap;
    #[test]
    fn full_keys_preserve_two_results_under_a_forced_digest_collision() {
        let a = InvalidId {
            digest: 7,
            canonical_key: alloc::vec![1],
        };
        let b = InvalidId {
            digest: 7,
            canonical_key: alloc::vec![2],
        };
        let mut stored = BTreeMap::new();
        stored.insert(ObjectRef::InvalidMeasurement(a), "first");
        stored.insert(ObjectRef::InvalidMeasurement(b), "second");
        assert_eq!(stored.len(), 2);
    }
}
