use mdc_replay::{recovery::*, LagAnchor, Profile, Symbol, TimestampUnit};
use sha2::{Digest, Sha256};
const CSV: &[u8] = include_bytes!("fixtures/BTCUSDT-1m-2024-01-01-first3.csv");
fn profile() -> Profile {
    Profile::new(
        Symbol::BtcUsdt,
        TimestampUnit::Milliseconds,
        100,
        LagAnchor::ReportedClose,
    )
    .unwrap()
}
fn plan() -> Vec<u64> {
    vec![
        1704067260098999,
        1704067260099000,
        1704067320049000,
        1704067320099000,
    ]
}
fn manifest() -> RunManifest {
    RunManifest::new(CSV, profile(), plan(), RunLimits::default()).unwrap()
}
fn vector(name: &str) -> Vec<u8> {
    let (_, value) = include_str!("fixtures/replay-run-wire-v1.tsv")
        .lines()
        .filter_map(|l| l.split_once('\t'))
        .find(|(n, _)| *n == name)
        .unwrap();
    (0..value.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&value[i..i + 2], 16).unwrap())
        .collect()
}
fn reseal(b: &mut [u8], domain: &[u8]) {
    let n = b.len() - 32;
    let mut h = Sha256::new();
    h.update(domain);
    h.update(&b[..n]);
    b[n..].copy_from_slice(&h.finalize());
}
#[test]
fn independent_python_manifest_run_header_and_empty_head_vectors_match() {
    let m = manifest();
    assert_eq!(m.encode().unwrap(), vector("manifest"));
    assert_eq!(m.run_id().to_vec(), vector("run-id"));
    assert_eq!(RunManifest::decode(&vector("manifest")).unwrap(), m);
    assert!(validate_prefix(
        &vector("manifest"),
        CSV,
        &vector("head-empty"),
        &vector("journal-empty")
    )
    .unwrap()
    .is_empty());
}
#[test]
fn every_manifest_byte_is_integrity_bound_and_exact_eof_is_required() {
    let original = vector("manifest");
    for i in 0..original.len() {
        let mut b = original.clone();
        b[i] ^= 1;
        assert!(RunManifest::decode(&b).is_err(), "byte {i}");
    }
    for n in 0..original.len() {
        assert!(RunManifest::decode(&original[..n]).is_err());
    }
    let mut b = original;
    b.push(0);
    assert!(RunManifest::decode(&b).is_err());
}
#[test]
fn recomputed_checksums_do_not_authorize_unknown_versions_tags_or_noncanonical_lag() {
    for (offset, value) in [
        (16, 2),
        (26, 2),
        (28, 2),
        (30, 2),
        (71, 2),
        (72, 2),
        (73, 2),
        (74, 1),
    ] {
        let mut b = vector("manifest");
        b[offset] = value;
        reseal(&mut b, b"MDC-REPLAY-RUN-001/manifest/v1\0");
        assert!(RunManifest::decode(&b).is_err(), "offset {offset}");
    }
}
#[test]
fn malformed_length_and_limit_bombs_are_bounded_before_allocation() {
    for offset in [18, 31, 82, 130] {
        let mut b = vector("manifest");
        b[offset..offset + 8].copy_from_slice(&u64::MAX.to_le_bytes());
        reseal(&mut b, b"MDC-REPLAY-RUN-001/manifest/v1\0");
        assert!(RunManifest::decode(&b).is_err());
    }
    assert!(RunManifest::decode(&vec![0; 16385]).is_err());
}
#[test]
fn plans_and_persisted_limits_must_be_nonempty_strict_and_representable() {
    for p in [vec![], vec![1, 1], vec![2, 1], vec![0; 257]] {
        assert!(RunManifest::new(CSV, profile(), p, RunLimits::default()).is_err());
    }
    for l in [
        RunLimits {
            max_decisions: 0,
            ..RunLimits::default()
        },
        RunLimits {
            max_journal_bytes: usize::MAX,
            ..RunLimits::default()
        },
        RunLimits {
            max_manifest_bytes: 1,
            ..RunLimits::default()
        },
    ] {
        assert!(RunManifest::new(CSV, profile(), plan(), l).is_err());
    }
}
#[test]
fn source_and_model_are_audit_namespace_but_not_a_silent_continuation() {
    let a = manifest();
    let mut source = CSV.to_vec();
    source.push(b'\n');
    assert!(RunManifest::new(&source, profile(), plan(), RunLimits::default()).is_err());
    let permutation = std::str::from_utf8(CSV)
        .unwrap()
        .lines()
        .rev()
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let b = RunManifest::new(
        permutation.as_bytes(),
        profile(),
        plan(),
        RunLimits::default(),
    )
    .unwrap();
    assert_ne!(a.run_id(), b.run_id());
    assert!(validate_prefix(
        &a.encode().unwrap(),
        permutation.as_bytes(),
        &vector("head-empty"),
        &vector("journal-empty")
    )
    .is_err());
}
#[test]
fn head_prefix_and_manifest_bindings_fail_closed() {
    for which in ["head-empty", "journal-empty"] {
        let mut b = vector(which);
        b[30] ^= 1;
        let (head, prefix) = if which == "head-empty" {
            (b, vector("journal-empty"))
        } else {
            (vector("head-empty"), b)
        };
        assert!(validate_prefix(&vector("manifest"), CSV, &head, &prefix).is_err());
    }
    let mut h = vector("head-empty");
    h[98] = 1;
    reseal(&mut h, b"MDC-REPLAY-RUN-001/head/v1\0");
    assert!(validate_prefix(&vector("manifest"), CSV, &h, &vector("journal-empty")).is_err());
    let mut tail = vector("journal-empty");
    tail.push(0);
    assert!(validate_prefix(&vector("manifest"), CSV, &vector("head-empty"), &tail).is_err());
}
