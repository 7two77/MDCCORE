use super::*;
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);

#[test]
fn owner_drop_releases_the_lock_even_with_an_inherited_file_description() {
    let t = Temp::new();
    let s = RunStore::create(t.run(), CSV, m()).unwrap();
    // try_clone shares the open file description, as an inherited descriptor
    // does between fork and exec. Keep it alive across owner drop/reopen.
    let inherited = s._lock.file.try_clone().unwrap();
    assert!(matches!(
        RunStore::open(t.run(), None),
        Err(RecoveryError::LockBusy)
    ));
    drop(s);
    let next = RunStore::open(t.run(), None).unwrap();
    drop(inherited);
    assert_eq!(next.durable_count().unwrap(), 0);
    assert!(matches!(
        RunStore::open(t.run(), None),
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
const CSV: &[u8] = include_bytes!("../../../tests/fixtures/BTCUSDT-1m-2024-01-01-first3.csv");
fn m() -> RunManifest {
    RunManifest::new(
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
        RunLimits::default(),
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
        let mut s = RunStore::create(t.run(), CSV, m()).unwrap();
        s.step(1).unwrap();
        let inject = Inject::at(point);
        assert!(matches!(s.step(2), Err(RecoveryError::Indeterminate(_))));
        drop(inject);
        assert!(s.recovery_required());
        assert!(matches!(s.step(2), Err(RecoveryError::RecoveryRequired)));
        assert!(s.lookup(1).is_err());
        drop(s);
        let mut s = RunStore::open(t.run(), None).unwrap();
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
            RunStore::create(t.run(), CSV, m()),
            Err(RecoveryError::Indeterminate(_))
        ));
        drop(inject);
        if let Ok(s) = RunStore::open(t.run(), None) {
            assert_eq!(s.durable_count().unwrap(), 0);
        }
        assert!(RunStore::create(t.run(), CSV, m()).is_err());
    }
}
#[test]
fn checksummed_but_forged_report_snapshot_and_index_are_recomputed() {
    for kind in 0..3 {
        let t = Temp::new();
        let mut s = RunStore::create(t.run(), CSV, m()).unwrap();
        let mut bundle = s.step(1).unwrap();
        let mut head = s.head.clone();
        drop(s);
        match kind {
            0 => bundle.report = b"{\"fake\":true}\n".to_vec(),
            1 => bundle.snapshot.push(0),
            _ => bundle.index = 2,
        }
        let hdr = header(head.run, head.manifest).unwrap();
        let prev = hdr[hdr.len() - 32..].try_into().unwrap();
        let record = bundle.encode(head.run, prev, RunLimits::default()).unwrap();
        let mut journal = hdr;
        journal.extend_from_slice(&record);
        head.offset = journal.len() as u64;
        head.tip = record[record.len() - 32..].try_into().unwrap();
        head.bundle = bundle.digest(head.run);
        fs::write(t.run().join("JOURNAL"), journal).unwrap();
        fs::write(t.run().join("HEAD"), head.encode().unwrap()).unwrap();
        assert!(RunStore::open(t.run(), None).is_err());
    }
}
#[test]
fn tail_repair_failures_are_indeterminate_and_never_advance_prefix() {
    for point in ["tail-truncated", "tail-synced"] {
        let t = Temp::new();
        let mut s = RunStore::create(t.run(), CSV, m()).unwrap();
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
            RunStore::open(t.run(), None),
            Err(RecoveryError::Indeterminate(_))
        ));
        drop(inject);
        assert_eq!(
            RunStore::open(t.run(), None)
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
        let mut s = RunStore::create(t.run(), CSV, m()).unwrap();
        s.step(1).unwrap();
        drop(s);
        let inject = Inject::at(point);
        assert!(matches!(
            RunStore::open(t.run(), None),
            Err(RecoveryError::Indeterminate(_))
        ));
        drop(inject);
        assert_eq!(
            RunStore::open(t.run(), None)
                .unwrap()
                .durable_count()
                .unwrap(),
            1
        );
    }
}

#[test]
fn bundle_capture_matches_legacy_exports_across_cutoffs_and_profiles() {
    use crate::{work_counts, LagAnchor, Limits, Symbol, TimestampUnit};
    for (csv, unit) in [
        (CSV, TimestampUnit::Milliseconds),
        (
            include_bytes!("../../../tests/fixtures/BTCUSDT-1m-2025-01-01-first3.csv").as_slice(),
            TimestampUnit::Microseconds,
        ),
    ] {
        for symbol in [Symbol::BtcUsdt, Symbol::EthUsdt] {
            let profile = Profile::new(symbol, unit, 100, LagAnchor::ReportedClose).unwrap();
            let dataset = Dataset::from_csv(csv, profile, Limits::default()).unwrap();
            let a = dataset.evidence()[0].available_at();
            let b = dataset.evidence()[1].available_at();
            let manifest =
                RunManifest::new(csv, profile, vec![a - 1, a, b - 1, b], RunLimits::default())
                    .unwrap();
            for index in 1..=4 {
                work_counts::take();
                let view = dataset
                    .decision(manifest.cutoffs()[index as usize - 1])
                    .unwrap();
                let report = view.report_json().unwrap() + "\n";
                let snapshot = view.snapshot_bytes().unwrap();
                assert_eq!(work_counts::take().exports, 3);
                let bundle = DecisionBundle::build(&dataset, &manifest, index).unwrap();
                let observed = work_counts::take();
                assert_eq!(observed.exports, 1);
                assert_eq!(observed.restores, 1);
                assert_eq!(bundle.report(), report.as_bytes());
                assert_eq!(bundle.snapshot(), snapshot);
                // Repeated independent requests must not consume/replace a cached view.
                assert_eq!(
                    DecisionBundle::build(&dataset, &manifest, index).unwrap(),
                    bundle
                );
            }
        }
    }
}

#[test]
fn open_prepares_once_but_restores_and_recomputes_every_bundle() {
    use crate::work_counts;
    let t = Temp::new();
    let manifest = m();
    let mut s = RunStore::create(t.run(), CSV, manifest.clone()).unwrap();
    let saved: Vec<_> = (1..=2).map(|i| s.step(i).unwrap()).collect();
    drop(s);
    work_counts::take();
    let mut s = RunStore::open(t.run(), Some(&manifest)).unwrap();
    assert_eq!(
        work_counts::take(),
        work_counts::Counts {
            exports: 2,
            parses: 1,
            manifests: 1,
            restores: 2
        }
    );
    for (i, bundle) in saved.iter().enumerate() {
        assert_eq!(s.lookup(i as u64 + 1).unwrap(), Some(bundle));
    }
    assert_eq!(s.step(1).unwrap(), saved[0]);
    assert_eq!(work_counts::take(), work_counts::Counts::default());
    assert_eq!(s.next_index().unwrap(), Some(3));
    drop(s);
    work_counts::take();
    let decoded = validate_prefix(
        &fs::read(t.run().join("MANIFEST")).unwrap(),
        CSV,
        &fs::read(t.run().join("HEAD")).unwrap(),
        &fs::read(t.run().join("JOURNAL")).unwrap(),
    )
    .unwrap();
    assert_eq!(decoded, saved);
    assert_eq!(
        work_counts::take(),
        work_counts::Counts {
            exports: 2,
            parses: 1,
            manifests: 1,
            restores: 2
        }
    );
}

#[test]
fn preparation_preserves_source_and_expected_manifest_error_precedence() {
    use crate::work_counts;
    let t = Temp::new();
    let manifest = m();
    let s = RunStore::create(t.run(), CSV, manifest.clone()).unwrap();
    drop(s);
    let before_head = fs::read(t.run().join("HEAD")).unwrap();
    let before_journal = fs::read(t.run().join("JOURNAL")).unwrap();
    let mut wrong_source = CSV.to_vec();
    wrong_source[0] ^= 1;
    let bytes = manifest.encode().unwrap();
    work_counts::take();
    assert!(matches!(
        validate_prefix(&bytes, &wrong_source, b"bad head", b"bad journal"),
        Err(RecoveryError::Integrity("captured source"))
    ));
    assert_eq!(
        work_counts::take(),
        work_counts::Counts {
            manifests: 1,
            ..Default::default()
        }
    );
    fs::write(t.run().join("SOURCE"), &wrong_source).unwrap();
    let other = RunManifest::new(
        CSV,
        manifest.profile(),
        vec![manifest.cutoffs()[0]],
        manifest.limits(),
    )
    .unwrap();
    work_counts::take();
    assert!(matches!(
        RunStore::open(t.run(), Some(&other)),
        Err(RecoveryError::ExpectedManifest)
    ));
    assert_eq!(
        work_counts::take(),
        work_counts::Counts {
            manifests: 1,
            ..Default::default()
        }
    );
    assert_eq!(fs::read(t.run().join("HEAD")).unwrap(), before_head);
    assert_eq!(fs::read(t.run().join("JOURNAL")).unwrap(), before_journal);
}

#[test]
fn captured_blob_limits_still_reject_before_restore_and_journal_mutation() {
    use crate::work_counts;
    for snapshot_limit in [false, true] {
        let t = Temp::new();
        let template = m();
        let mut limits = template.limits();
        if snapshot_limit {
            limits.max_snapshot_bytes = 1;
        } else {
            limits.max_report_bytes = 1;
        }
        let manifest =
            RunManifest::new(CSV, template.profile(), template.cutoffs().to_vec(), limits).unwrap();
        let mut s = RunStore::create(t.run(), CSV, manifest).unwrap();
        let before = ["HEAD", "JOURNAL"].map(|n| fs::read(t.run().join(n)).unwrap());
        work_counts::take();
        assert!(matches!(
            s.step(1),
            Err(RecoveryError::Limit("decision blobs"))
        ));
        let counts = work_counts::take();
        assert_eq!(counts.exports, 1);
        assert_eq!(counts.restores, 0);
        assert_eq!(s.durable_count().unwrap(), 0);
        assert!(!s.recovery_required());
        assert_eq!(
            ["HEAD", "JOURNAL"].map(|n| fs::read(t.run().join(n)).unwrap()),
            before
        );
    }
}
