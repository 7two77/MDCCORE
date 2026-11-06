use super::*;
const CSV: &[u8] = include_bytes!("../../../tests/fixtures/BTCUSDT-1m-2024-01-01-first3.csv");
fn vector(name: &str) -> Vec<u8> {
    read_vector(
        include_str!("../../../tests/fixtures/replay-run-wire-v2.tsv"),
        name,
    )
}
fn read_vector(table: &str, name: &str) -> Vec<u8> {
    let hex = table
        .lines()
        .filter_map(|s| s.split_once('\t'))
        .find(|(n, _)| *n == name)
        .unwrap()
        .1;
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect()
}
fn reseal(kind: V2Kind, b: &mut [u8]) {
    let n = b.len() - 32;
    let h = domain_hash(kind.domain(), &b[..n]);
    b[n..].copy_from_slice(&h);
}
#[test]
fn independent_manifest_header_head_and_decision_vectors() {
    let m = RunManifestV2::decode(&vector("manifest")).unwrap();
    assert_eq!(m.encode().unwrap(), vector("manifest"));
    assert_eq!(m.run_id().to_vec(), vector("run-id"));
    assert!(m.logical.dataset(CSV).is_ok());
    let header = header2(m.run_id(), hash(&vector("manifest"))).unwrap();
    assert_eq!(header, vector("journal-empty"));
    let head = HeadV2::decode(&vector("head-empty")).unwrap();
    assert_eq!(head.encode().unwrap(), vector("head-empty"));
    assert_eq!(head.offset, 124);
    assert_eq!(head.count, 0);
    let (b, d, total) = decode_record(
        &vector("decision"),
        m.run_id(),
        header[header.len() - 32..].try_into().unwrap(),
        m.limits,
        0,
    )
    .unwrap();
    assert_eq!(b.report(), vector("report"));
    assert_eq!(b.snapshot(), vector("snapshot"));
    assert_eq!(b.cutoff(), m.cutoffs()[0]);
    assert_eq!(b.index(), 1);
    assert_eq!(total, b.report().len() + b.snapshot().len());
    assert_eq!(
        d.to_vec(),
        vector("decision")[vector("decision").len() - 32..]
    );
}
#[test]
fn pinned_writer_and_independent_zlib_reader_vectors() {
    let table = include_str!("../../../tests/fixtures/replay-writer-v2.tsv");
    for (name, raw) in [
        ("empty", vec![]),
        ("small", b"xyz".to_vec()),
        ("noise", (0..=255).collect()),
        ("repeat", vec![b'A'; 1024]),
    ] {
        let encoded = codec::encode_blob(&raw, 16384).unwrap();
        assert_eq!(encoded, read_vector(table, name));
        assert_eq!(codec::decode_blob(&encoded, 16384, 16384).unwrap(), raw);
        assert_eq!(encoded[0], u8::from(name == "repeat"));
    }
    assert_eq!(
        codec::decode_blob(&vector("python-compressed"), 16384, 16384).unwrap(),
        vector("snapshot")
    );
}
#[test]
fn blob_truncation_stream_trailing_dictionary_and_checksum_reject() {
    let original = vector("python-compressed");
    for n in 0..original.len() {
        assert!(
            codec::decode_blob(&original[..n], 16384, 16384).is_err(),
            "truncated {n}"
        );
    }
    for suffix in [vec![0], original[49..].to_vec()] {
        let mut b = original.clone();
        b.extend(suffix);
        let n = (b.len() - 49) as u64;
        b[9..17].copy_from_slice(&n.to_le_bytes());
        assert!(
            codec::decode_blob(&b, 16384, 16384).is_err(),
            "extra stream/bytes"
        );
    }
    for offset in [0, 17, 49, 50, original.len() - 1] {
        let mut b = original.clone();
        b[offset] ^= 0x20;
        assert!(
            codec::decode_blob(&b, 16384, 16384).is_err(),
            "corrupt {offset}"
        );
    }
    let mut b = original.clone();
    b[50] |= 0x20;
    assert!(codec::decode_blob(&b, 16384, 16384).is_err());
}
#[test]
fn blob_declared_and_actual_output_bounds_are_independent() {
    let original = vector("python-compressed");
    for n in [0, 1, 2999, 3001, u64::MAX] {
        let mut b = original.clone();
        b[1..9].copy_from_slice(&n.to_le_bytes());
        assert!(
            codec::decode_blob(&b, 16384, 16384).is_err(),
            "decoded length {n}"
        );
    }
    let mut b = original.clone();
    b[9..17].copy_from_slice(&u64::MAX.to_le_bytes());
    assert!(codec::decode_blob(&b, 16384, 16384).is_err());
    assert!(codec::decode_blob(&original, 2, 16384).is_err());
    assert!(codec::decode_blob(&original, 16384, 2).is_err());
    let mut raw = codec::encode_blob(b"x", 16384).unwrap();
    raw[1..9].copy_from_slice(&2u64.to_le_bytes());
    assert!(codec::decode_blob(&raw, 16384, 16384).is_err());
}
#[test]
fn manifest_every_byte_eof_versions_limits_and_plan_are_bound() {
    let original = vector("manifest");
    for i in 0..original.len() {
        let mut b = original.clone();
        b[i] ^= 1;
        assert!(RunManifestV2::decode(&b).is_err(), "byte {i}");
    }
    for i in 0..original.len() {
        assert!(RunManifestV2::decode(&original[..i]).is_err());
    }
    let mut extra = original.clone();
    extra.push(0);
    assert!(RunManifestV2::decode(&extra).is_err());
    for (offset, n) in [
        (16, 3),
        (26, 2),
        (28, 2),
        (30, 2),
        (71, 2),
        (72, 2),
        (73, 2),
        (74, 1),
        (146, 2),
        (147, 5),
    ] {
        let mut b = original.clone();
        b[offset] = n;
        reseal(V2Kind::Manifest, &mut b);
        assert!(RunManifestV2::decode(&b).is_err(), "version/tag {offset}");
    }
    for offset in [18, 31, 82, 90, 98, 106, 114, 122, 130, 138, 148] {
        let mut b = original.clone();
        b[offset..offset + 8].copy_from_slice(&u64::MAX.to_le_bytes());
        reseal(V2Kind::Manifest, &mut b);
        assert!(RunManifestV2::decode(&b).is_err(), "length {offset}");
    }
}
#[test]
fn aggregate_and_decoded_record_limits_precede_blob_decode() {
    let m = RunManifestV2::decode(&vector("manifest")).unwrap();
    let hdr = vector("journal-empty");
    let tip = hdr[hdr.len() - 32..].try_into().unwrap();
    let l = StorageLimits {
        max_total_decoded_blob_bytes: 1,
        ..m.limits
    };
    assert!(matches!(
        decode_record(&vector("decision"), m.run_id(), tip, l, 0),
        Err(RecoveryError::Limit("v2 decoded aggregate"))
    ));
    let l = StorageLimits {
        max_decoded_record_bytes: 1,
        ..m.limits
    };
    assert!(matches!(
        decode_record(&vector("decision"), m.run_id(), tip, l, 0),
        Err(RecoveryError::Limit("v2 decoded record"))
    ));
    assert!(check_decoded(usize::MAX, 0, m.limits).is_err());
    assert!(check_decoded(1, usize::MAX, m.limits).is_err());
}
