use mdc_core::canonical::snapshot::{
    migrate_legacy_snapshot, snapshot, snapshot_with_protocol, Snapshot,
};
use mdc_core::identity::{portable_v1_preimage, CanonicalSemanticBasis, IdentityProtocol};
use std::cell::Cell;

#[allow(dead_code)]
#[path = "../examples/support/portable_identity.rs"]
mod fixture;

fn unhex(text: &str) -> Vec<u8> {
    if text == "-" {
        return vec![];
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

#[test]
fn portable_protocol_matches_independent_preimage_and_hash_vectors() {
    let mut count = 0;
    for line in include_str!("fixtures/portable-identity-v1.tsv")
        .lines()
        .filter(|l| !l.starts_with('#'))
    {
        let fields: Vec<_> = line.split('\t').collect();
        let body = unhex(fields[1]);
        assert_eq!(
            portable_v1_preimage(&body),
            unhex(fields[2]),
            "{}",
            fields[0]
        );
        assert_eq!(
            IdentityProtocol::PortableV1.derive_from_basis(&body),
            u64::from_str_radix(fields[3], 16).unwrap(),
            "{}",
            fields[0]
        );
        count += 1;
    }
    assert_eq!(count, 6);
}

#[test]
fn unknown_protocol_never_falls_back_and_handles_keep_the_protocol() {
    assert_eq!(IdentityProtocol::from_name("MDC-ID-FNV1A64-LE-v2"), None);
    assert_eq!(IdentityProtocol::from_name(""), None);
    for protocol in [
        IdentityProtocol::LegacyRustHashFnv64,
        IdentityProtocol::PortableV1,
    ] {
        assert_eq!(IdentityProtocol::from_name(protocol.name()), Some(protocol));
        assert_eq!(protocol.identify(b"x").protocol(), protocol);
    }
    assert_ne!(
        IdentityProtocol::LegacyRustHashFnv64.identify(b"x"),
        IdentityProtocol::PortableV1.identify(b"x")
    );
}

struct ChangingBasis(Cell<u8>);
impl CanonicalSemanticBasis for ChangingBasis {
    fn canonical_basis(&self) -> Vec<u8> {
        let n = self.0.get();
        self.0.set(n + 1);
        vec![n]
    }
}
#[test]
fn explicit_snapshot_captures_one_basis_and_migration_checks_legacy_bytes() {
    let input = ChangingBasis(Cell::new(0));
    let portable = snapshot_with_protocol(&input, IdentityProtocol::PortableV1);
    assert_eq!(input.0.get(), 1);
    assert_eq!(
        portable.identity,
        IdentityProtocol::PortableV1.identify(&portable.semantic_basis)
    );
    let old = snapshot(&input);
    let migrated = migrate_legacy_snapshot(&old).unwrap();
    assert_eq!(old.semantic_basis, migrated.semantic_basis);
    assert_eq!(migrated.identity.protocol(), IdentityProtocol::PortableV1);
    assert_eq!(input.0.get(), 2);
    assert!(migrate_legacy_snapshot(&Snapshot {
        identity: old.identity ^ 1,
        semantic_basis: old.semantic_basis.clone()
    })
    .is_err());
}

#[test]
fn portable_graph_includes_all_levels_invalids_retention_and_lifecycle() {
    let first = fixture::run();
    assert_eq!(first, fixture::run());
    for kind in [
        "observation",
        "measurement",
        "relation",
        "composition",
        "state",
        "transition",
        "evaluation",
        "evaluation-key",
        "invalid",
        "lifecycle",
        "reevaluation",
    ] {
        assert!(
            first.lines().any(|l| l.starts_with(&format!("{kind}\t"))),
            "{kind}"
        );
    }
}

#[test]
fn legacy_graph_stays_immutable_when_replayed_under_the_new_protocol() {
    let old = fixture::scenario::graph();
    let before = fixture::scenario::capture(&old.p, &old.refs);
    let new = fixture::scenario::graph_with_identity_protocol(
        fixture::scenario::registry(),
        IdentityProtocol::PortableV1,
    );
    assert_eq!(
        old.p.identity_protocol(),
        IdentityProtocol::LegacyRustHashFnv64
    );
    assert_eq!(
        old.p
            .resolve_observation(&old.roots[0])
            .unwrap()
            .canonical_basis(),
        new.p
            .resolve_observation(&new.roots[0])
            .unwrap()
            .canonical_basis()
    );
    assert_ne!(old.roots[0], new.roots[0]);
    assert_ne!(old.roots[1], new.roots[1]);
    assert_eq!(
        old.p.resolve_measurement(&old.roots[1]).unwrap().value(),
        new.p.resolve_measurement(&new.roots[1]).unwrap().value()
    );
    assert_eq!(
        old.p.resolve_relation(&old.roots[2]).unwrap().value(),
        new.p.resolve_relation(&new.roots[2]).unwrap().value()
    );
    assert_eq!(
        old.p
            .resolve_composition(&old.roots[3])
            .unwrap()
            .constraints_result(),
        new.p
            .resolve_composition(&new.roots[3])
            .unwrap()
            .constraints_result()
    );
    assert_eq!(
        old.p
            .resolve_state(&old.roots[4])
            .unwrap()
            .criteria_result(),
        new.p
            .resolve_state(&new.roots[4])
            .unwrap()
            .criteria_result()
    );
    assert_eq!(
        old.p
            .resolve_state(&old.roots[4])
            .unwrap()
            .validity_interval(),
        new.p
            .resolve_state(&new.roots[4])
            .unwrap()
            .validity_interval()
    );
    assert_eq!(
        old.p
            .resolve_transition(&old.roots[5])
            .unwrap()
            .conditions_result(),
        new.p
            .resolve_transition(&new.roots[5])
            .unwrap()
            .conditions_result()
    );
    assert_eq!(before, fixture::scenario::capture(&old.p, &old.refs));
    assert_eq!(
        old.p.resolve_evaluation(&old.evaluation).unwrap().result(),
        new.p.resolve_evaluation(&new.evaluation).unwrap().result()
    );
}

#[test]
fn typed_reference_wire_values_are_fixed_and_length_delimited() {
    use mdc_core::contract::registry::VersionedId;
    use mdc_core::identity::canonical_reference_v1;
    use mdc_core::provenance::ObjectRef;
    assert_eq!(
        canonical_reference_v1(&ObjectRef::Source(VersionedId::new("a", 1).unwrap())),
        unhex("0001000000000000006101000000")
    );
    assert_eq!(
        canonical_reference_v1(&ObjectRef::Observation(0x0102030405060708)),
        unhex("010807060504030201")
    );
    assert_eq!(
        canonical_reference_v1(&ObjectRef::Transition(0x0102030405060708)),
        unhex("060807060504030201")
    );
    assert_ne!(
        canonical_reference_v1(&ObjectRef::Measurement(1)),
        canonical_reference_v1(&ObjectRef::Observation(1))
    );
}

#[test]
fn a_legacy_evaluation_lookup_never_falls_back_to_a_portable_artifact() {
    use mdc_core::provenance::{EvaluationId, ObjectRef};
    let g = fixture::scenario::graph_with_identity_protocol(
        fixture::scenario::registry(),
        IdentityProtocol::PortableV1,
    );
    let artifact = g.p.resolve_evaluation(&g.evaluation).unwrap();
    let wrong = ObjectRef::EvaluationKey(EvaluationId::from_canonical_basis(
        artifact.canonical_basis(),
    ));
    let correct = ObjectRef::EvaluationKey(EvaluationId::from_canonical_basis_with_protocol(
        artifact.canonical_basis(),
        IdentityProtocol::PortableV1,
    ));
    assert_ne!(wrong, correct);
    assert!(g.p.resolve_evaluation(&wrong).is_none());
    assert_eq!(g.p.resolve_evaluation(&correct), Some(artifact));
}
