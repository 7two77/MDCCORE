use std::cell::Cell;
use std::hash::{Hash, Hasher};

use mdc_core::canonical::snapshot::snapshot;
use mdc_core::identity::{CanonicalSemanticBasis, FnvHasher, IdentityAdapter};
use mdc_core::model::observation::{Observation, Source};

struct TestSource(String);

impl Source for TestSource {
    fn id(&self) -> &str {
        &self.0
    }
}

#[test]
fn snapshot_captures_one_basis_even_if_successive_calls_differ() {
    struct ChangingBasis {
        calls: Cell<u8>,
    }

    impl CanonicalSemanticBasis for ChangingBasis {
        fn canonical_basis(&self) -> Vec<u8> {
            let next = self.calls.get() + 1;
            self.calls.set(next);
            vec![next]
        }
    }

    let object = ChangingBasis {
        calls: Cell::new(0),
    };
    let captured = snapshot(&object);

    assert_eq!(object.calls.get(), 1);
    assert_eq!(captured.semantic_basis, vec![1]);
    assert_eq!(
        captured.identity,
        IdentityAdapter::derive_identity_from_basis(&captured.semantic_basis),
    );
}

#[test]
fn observation_basis_remains_owned_after_observation_is_dropped() {
    let basis = {
        let observation =
            Observation::new(TestSource("BTC".to_owned()), 100, vec![1, 2, 3]).unwrap();
        observation.canonical_basis()
    };

    assert_eq!(basis.as_slice(), b"\"BTC\"|100|[1, 2, 3]");
}

#[test]
fn observation_preserves_the_existing_bytes_and_native_hash_protocol() {
    let observation = Observation::new(TestSource("BTC".to_owned()), 100, vec![1, 2, 3]).unwrap();
    let expected_bytes: &[u8] = b"\"BTC\"|100|[1, 2, 3]";

    // Reproduce the pre-repair adapter's slice Hash protocol. This intentionally
    // checks compatibility on the current toolchain/target, not portability.
    let mut legacy_hasher = FnvHasher::default();
    expected_bytes.hash(&mut legacy_hasher);
    let expected_identity = legacy_hasher.finish();

    assert_eq!(observation.canonical_basis().as_slice(), expected_bytes);
    assert_eq!(
        IdentityAdapter::derive_identity(&observation),
        expected_identity,
    );
    let captured = snapshot(&observation);
    assert_eq!(captured.semantic_basis.as_slice(), expected_bytes);
    assert_eq!(captured.identity, expected_identity);
}
