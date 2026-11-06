use super::*;
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);

#[test]
fn owner_drop_releases_the_lock_even_with_an_inherited_file_description() {
    let t = Temp::new();
    let s = RunStoreV2::create(t.run(), CSV, m()).unwrap();
    // try_clone shares the open file description, as an inherited descriptor
    // does between fork and exec. Keep it alive across owner drop/reopen.
    let inherited = s.guard.file.try_clone().unwrap();
    assert!(matches!(
        RunStoreV2::open(t.run(), None),
        Err(RecoveryError::LockBusy)
    ));
    drop(s);
    let next = RunStoreV2::open(t.run(), None).unwrap();
    drop(inherited);
    assert_eq!(next.durable_count().unwrap(), 0);
    assert!(matches!(
        RunStoreV2::open(t.run(), None),
        Err(RecoveryError::LockBusy)
    ));
}
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "mdc-replay-unit-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn run(&self) -> PathBuf {
        self.0.join("run")
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
struct Inject;
impl Inject {
    fn at(point: &'static str) -> Self {
        FAULT.with(|v| v.set(Some(point)));
        Self
    }
}
impl Drop for Inject {
    fn drop(&mut self) {
        FAULT.with(|v| v.set(None));
    }
}
const CSV: &[u8] = include_bytes!("../../../../tests/fixtures/BTCUSDT-1m-2024-01-01-first3.csv");
fn m() -> RunManifestV2 {
    RunManifestV2::new(
        CSV,
        Profile::new(
            Symbol::BtcUsdt,
            TimestampUnit::Milliseconds,
            100,
            LagAnchor::ReportedClose,
        )
        .unwrap(),
        vec![
            1704067260098999,
            1704067260099000,
            1704067320049000,
            1704067320099000,
        ],
        StorageLimits::default(),
    )
    .unwrap()
}
#[test]
fn every_post_mutation_commit_error_fences_until_reopen() {
    for point in [
        "record-half",
        "record-written",
        "record-synced",
        "head-half",
        "head-written",
        "head-synced",
        "head-renamed",
        "head-dir-synced",
        "before-reply",
    ] {
        let t = Temp::new();
        let mut s = RunStoreV2::create(t.run(), CSV, m()).unwrap();
        s.step(1).unwrap();
        let inject = Inject::at(point);
        assert!(matches!(s.step(2), Err(RecoveryError::Indeterminate(_))));
        drop(inject);
        assert!(s.recovery_required());
        assert!(matches!(s.step(2), Err(RecoveryError::RecoveryRequired)));
        assert!(s.lookup_owned(1).is_err());
        drop(s);
        let mut s = RunStoreV2::open(t.run(), None).unwrap();
        let n = s.durable_count().unwrap();
        assert!((1..=2).contains(&n));
        s.step(2).unwrap();
        assert_eq!(s.durable_count().unwrap(), 2);
    }
}
#[test]
fn interrupted_create_never_becomes_an_implicit_new_run() {
    for point in [
        "create-parent-synced",
        "create-half",
        "create-written",
        "create-synced",
        "source-synced",
        "manifest-synced",
        "journal-created",
        "create-files-synced",
        "head-half",
        "head-written",
        "head-synced",
        "head-renamed",
        "head-dir-synced",
    ] {
        let t = Temp::new();
        let inject = Inject::at(point);
        assert!(matches!(
            RunStoreV2::create(t.run(), CSV, m()),
            Err(RecoveryError::Indeterminate(_))
        ));
        drop(inject);
        if let Ok(s) = RunStoreV2::open(t.run(), None) {
            assert_eq!(s.durable_count().unwrap(), 0);
        }
        assert!(RunStoreV2::create(t.run(), CSV, m()).is_err());
    }
}
#[test]
fn checksummed_but_forged_report_snapshot_and_index_are_recomputed() {
    for kind in 0..3 {
        let t = Temp::new();
        let mut s = RunStoreV2::create(t.run(), CSV, m()).unwrap();
        let mut bundle = s.step(1).unwrap();
        let mut head = s.head.clone();
        drop(s);
        match kind {
            0 => bundle.report = b"{\"fake\":true}\n".to_vec(),
            1 => bundle.snapshot.push(0),
            _ => bundle.index = 2,
        }
        let hdr = header2(head.run, head.manifest).unwrap();
        let prev = hdr[hdr.len() - 32..].try_into().unwrap();
        let record = encode_record(&bundle, head.run, prev, StorageLimits::default()).unwrap();
        let mut journal = hdr;
        journal.extend_from_slice(&record);
        head.offset = journal.len() as u64;
        head.tip = record[record.len() - 32..].try_into().unwrap();
        head.bundle = bundle_digest(head.run, &bundle);
        fs::write(t.run().join("JOURNAL"), journal).unwrap();
        fs::write(t.run().join("HEAD"), head.encode().unwrap()).unwrap();
        assert!(RunStoreV2::open(t.run(), None).is_err());
    }
}
#[test]
fn tail_repair_failures_are_indeterminate_and_never_advance_prefix() {
    for point in ["tail-truncated", "tail-synced"] {
        let t = Temp::new();
        let mut s = RunStoreV2::create(t.run(), CSV, m()).unwrap();
        s.step(1).unwrap();
        drop(s);
        OpenOptions::new()
            .append(true)
            .open(t.run().join("JOURNAL"))
            .unwrap()
            .write_all(b"uncommitted")
            .unwrap();
        let inject = Inject::at(point);
        assert!(matches!(
            RunStoreV2::open(t.run(), None),
            Err(RecoveryError::Indeterminate(_))
        ));
        drop(inject);
        assert_eq!(
            RunStoreV2::open(t.run(), None)
                .unwrap()
                .durable_count()
                .unwrap(),
            1
        );
    }
}

#[test]
fn recovery_does_not_expose_a_prefix_before_successful_sealing() {
    for point in ["recovery-journal-synced", "recovery-dir-synced"] {
        let t = Temp::new();
        let mut s = RunStoreV2::create(t.run(), CSV, m()).unwrap();
        s.step(1).unwrap();
        drop(s);
        let inject = Inject::at(point);
        assert!(matches!(
            RunStoreV2::open(t.run(), None),
            Err(RecoveryError::Indeterminate(_))
        ));
        drop(inject);
        assert_eq!(
            RunStoreV2::open(t.run(), None)
                .unwrap()
                .durable_count()
                .unwrap(),
            1
        );
    }
}
#[test]
fn owned_read_corruption_fences_all_further_operations() {
    let t = Temp::new();
    let mut s = RunStoreV2::create(t.run(), CSV, m()).unwrap();
    let original = s.step(1).unwrap();
    s.step(2).unwrap();
    let file = t.run().join("JOURNAL");
    let mut bytes = fs::read(&file).unwrap();
    bytes[160] ^= 1;
    fs::write(file, bytes).unwrap();
    assert!(s.lookup_owned(1).is_err());
    assert!(s.recovery_required());
    assert!(s.step(3).is_err());
    assert!(s.lookup_owned(2).is_err());
    assert!(s.durable_count().is_err());
    assert!(!original.report().is_empty());
    drop(s);
    assert!(RunStoreV2::open(t.run(), None).is_err());
}
#[test]
fn streaming_open_has_only_an_index_and_no_repeated_semantic_lookup() {
    use crate::work_counts;
    let t = Temp::new();
    let mut s = RunStoreV2::create(t.run(), CSV, m()).unwrap();
    let want: Vec<_> = (1..=4).map(|n| s.step(n).unwrap()).collect();
    drop(s);
    work_counts::take();
    let mut s = RunStoreV2::open(t.run(), None).unwrap();
    let counts = work_counts::take();
    assert_eq!(counts.parses, 1);
    assert_eq!(counts.restores, 4);
    assert_eq!(counts.exports, 4);
    assert_eq!(s.index.len(), 4);
    assert!(std::mem::size_of::<Entry>() < 160);
    for n in [4, 1, 3, 2] {
        assert_eq!(
            s.lookup_owned(n).unwrap().as_ref(),
            Some(&want[n as usize - 1])
        );
    }
    assert_eq!(work_counts::take(), work_counts::Counts::default());
    assert_eq!(
        s.decoded_blob_bytes().unwrap(),
        want.iter()
            .map(|b| b.report().len() + b.snapshot().len())
            .sum::<usize>()
    );
}
#[test]
fn v1_and_v2_are_explicit_and_decoded_bundles_match() {
    let t = Temp::new();
    let new = m();
    let old = super::super::super::RunManifest::new(
        CSV,
        new.profile(),
        new.cutoffs().to_vec(),
        super::super::super::RunLimits::default(),
    )
    .unwrap();
    assert_ne!(new.run_id(), old.run_id());
    let mut a = super::super::super::RunStore::create(t.0.join("old"), CSV, old).unwrap();
    let mut b = RunStoreV2::create(t.run(), CSV, new).unwrap();
    for n in 1..=4 {
        assert_eq!(a.step(n).unwrap(), b.step(n).unwrap());
    }
    drop(a);
    drop(b);
    assert!(RunStoreV2::open(t.0.join("old"), None).is_err());
    assert!(super::super::super::RunStore::open(t.run(), None).is_err());
}
#[test]
fn independent_physical_decoded_and_aggregate_limits_fail_before_io() {
    for l in [
        StorageLimits {
            max_encoded_record_bytes: 1,
            ..StorageLimits::default()
        },
        StorageLimits {
            max_physical_journal_bytes: 124,
            ..StorageLimits::default()
        },
        StorageLimits {
            max_decoded_record_bytes: 1,
            ..StorageLimits::default()
        },
        StorageLimits {
            max_total_decoded_blob_bytes: 1,
            ..StorageLimits::default()
        },
    ] {
        let t = Temp::new();
        let template = m();
        let m =
            RunManifestV2::new(CSV, template.profile(), template.cutoffs().to_vec(), l).unwrap();
        let mut s = RunStoreV2::create(t.run(), CSV, m).unwrap();
        let before = ["HEAD", "JOURNAL"].map(|n| fs::read(t.run().join(n)).unwrap());
        assert!(matches!(s.step(1), Err(RecoveryError::Limit(_))));
        assert_eq!(s.durable_count().unwrap(), 0);
        assert!(!s.recovery_required());
        assert_eq!(
            before,
            ["HEAD", "JOURNAL"].map(|n| fs::read(t.run().join(n)).unwrap())
        );
    }
}
#[test]
fn inherited_process_cannot_read_or_mutate_the_owners_store() {
    let t = Temp::new();
    let mut s = RunStoreV2::create(t.run(), CSV, m()).unwrap();
    let owner = s.guard.owner;
    s.guard.owner = owner.wrapping_add(1);
    assert!(matches!(
        s.lookup_owned(0),
        Err(RecoveryError::WrongProcess)
    ));
    assert!(matches!(s.step(1), Err(RecoveryError::WrongProcess)));
    assert!(matches!(
        s.durable_count(),
        Err(RecoveryError::WrongProcess)
    ));
    s.guard.owner = owner;
}
#[test]
fn owned_lookup_rejects_a_resealed_replacement_against_the_certified_index() {
    let t = Temp::new();
    let mut s = RunStoreV2::create(t.run(), CSV, m()).unwrap();
    let mut b = s.step(1).unwrap();
    let entry = s.index[0].clone();
    b.decision_id[0] ^= 1;
    let record = encode_record(&b, s.head.run, entry.previous, s.manifest.limits).unwrap();
    assert_eq!(record.len(), entry.length);
    let mut f = OpenOptions::new()
        .write(true)
        .open(t.run().join("JOURNAL"))
        .unwrap();
    f.seek(SeekFrom::Start(entry.offset)).unwrap();
    f.write_all(&record).unwrap();
    assert!(matches!(
        s.lookup_owned(1),
        Err(RecoveryError::Integrity("v2 owned frame changed"))
    ));
    assert!(s.recovery_required());
}
#[test]
fn decoded_aggregate_failure_after_a_commit_preserves_the_prior_prefix() {
    let t = Temp::new();
    let template = m();
    let a = DecisionBundle::build(
        &template.logical.dataset(CSV).unwrap(),
        &template.logical,
        1,
    )
    .unwrap();
    let l = StorageLimits {
        max_total_decoded_blob_bytes: a.report().len() + a.snapshot().len(),
        ..StorageLimits::default()
    };
    let manifest =
        RunManifestV2::new(CSV, template.profile(), template.cutoffs().to_vec(), l).unwrap();
    let mut s = RunStoreV2::create(t.run(), CSV, manifest.clone()).unwrap();
    s.step(1).unwrap();
    let before = ["HEAD", "JOURNAL"].map(|n| fs::read(t.run().join(n)).unwrap());
    assert!(matches!(
        s.step(2),
        Err(RecoveryError::Limit("v2 decoded aggregate"))
    ));
    assert_eq!(s.lookup_owned(1).unwrap().unwrap(), a);
    assert!(!s.recovery_required());
    assert_eq!(
        before,
        ["HEAD", "JOURNAL"].map(|n| fs::read(t.run().join(n)).unwrap())
    );
    drop(s);
    assert_eq!(
        RunStoreV2::open(t.run(), Some(&manifest))
            .unwrap()
            .durable_count()
            .unwrap(),
        1
    );
}
