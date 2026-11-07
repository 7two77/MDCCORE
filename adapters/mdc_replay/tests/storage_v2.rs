#![cfg(target_os = "linux")]
use mdc_replay::{
    recovery::{DecisionBundle, Hash, RecoveryError},
    recovery_v2::{
        RunManifestV2 as RunManifest, RunStoreV2 as RunStore, StorageLimits as RunLimits,
    },
    Dataset, LagAnchor, Limits, Profile, Symbol, TimestampUnit,
};
use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
const CSV: &[u8] = include_bytes!("fixtures/BTCUSDT-1m-2024-01-01-first3.csv");
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "mdc-replay-v2-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn path(&self, n: &str) -> PathBuf {
        self.0.join(n)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn p() -> Profile {
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
fn m() -> RunManifest {
    RunManifest::new(CSV, p(), plan(), RunLimits::default()).unwrap()
}
fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mdc-replay-v2"))
}
fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/BTCUSDT-1m-2024-01-01-first3.csv")
}
fn create_args(c: &mut Command, dir: &std::path::Path) {
    c.arg("create").arg(dir).arg(fixture()).args([
        "BTCUSDT",
        "ms",
        "100",
        "close",
        "1704067260098999,1704067260099000,1704067320049000,1704067320099000",
    ]);
}
fn disk(dir: &std::path::Path) -> Vec<Vec<u8>> {
    ["HEAD", "MANIFEST", "SOURCE", "JOURNAL"]
        .iter()
        .map(|n| fs::read(dir.join(n)).unwrap())
        .collect()
}

#[test]
fn resumed_every_prefix_matches_uninterrupted_full_bytes_and_cursor() {
    let t = Temp::new();
    let mut a = RunStore::create(t.path("a"), CSV, m()).unwrap();
    let mut expected = Vec::new();
    for n in 1..=4 {
        expected.push(a.step(n).unwrap());
    }
    drop(a);
    let mut s = RunStore::create(t.path("b"), CSV, m()).unwrap();
    assert_eq!(s.next_index().unwrap(), Some(1));
    for (i, want) in expected.iter().enumerate() {
        drop(s);
        s = RunStore::open(t.path("b"), Some(&m())).unwrap();
        let got = s.step(i as u64 + 1).unwrap();
        assert_eq!(&got, want);
        assert_eq!(s.durable_count().unwrap(), i as u64 + 1);
    }
    assert_eq!(s.next_index().unwrap(), None);
    drop(s);
    assert_eq!(disk(&t.path("a")), disk(&t.path("b")));
    let mut s = RunStore::open(t.path("b"), None).unwrap();
    assert_eq!(
        s.lookup_owned(3).unwrap().unwrap().report(),
        expected[2].report()
    );
    assert!(std::str::from_utf8(expected[0].report())
        .unwrap()
        .contains("NO_INPUT"));
    assert!(std::str::from_utf8(expected[2].report())
        .unwrap()
        .contains("UNRESOLVED"));
    assert!(std::str::from_utf8(expected[3].report())
        .unwrap()
        .contains("PASS"));
}
#[test]
fn committed_retry_lost_reply_and_invalid_indices_never_append() {
    let t = Temp::new();
    let mut s = RunStore::create(t.path("run"), CSV, m()).unwrap();
    s.step(1).unwrap();
    drop(s);
    let mut s = RunStore::open(t.path("run"), None).unwrap();
    let before = disk(&t.path("run"));
    let saved = s.lookup_owned(1).unwrap().unwrap().clone();
    assert_eq!(s.step(1).unwrap(), saved);
    for n in [0, 3, 5, u64::MAX] {
        assert!(matches!(s.step(n), Err(RecoveryError::Index)));
    }
    assert_eq!(disk(&t.path("run")), before);
    for n in 2..=4 {
        s.step(n).unwrap();
    }
    let before = disk(&t.path("run"));
    assert_eq!(s.step(4).unwrap().index(), 4);
    assert!(s.step(5).is_err());
    assert_eq!(disk(&t.path("run")), before);
}
#[test]
fn wrong_source_model_plan_or_limits_rejects_before_tail_repair() {
    let t = Temp::new();
    let mut s = RunStore::create(t.path("run"), CSV, m()).unwrap();
    s.step(1).unwrap();
    drop(s);
    fs::OpenOptions::new()
        .append(true)
        .open(t.path("run/JOURNAL"))
        .unwrap()
        .write_all(b"tail")
        .unwrap();
    let before = disk(&t.path("run"));
    let reversed = std::str::from_utf8(CSV)
        .unwrap()
        .lines()
        .rev()
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let alternatives = [
        RunManifest::new(reversed.as_bytes(), p(), plan(), RunLimits::default()).unwrap(),
        RunManifest::new(
            CSV,
            Profile::new(
                Symbol::BtcUsdt,
                TimestampUnit::Milliseconds,
                200,
                LagAnchor::ReportedClose,
            )
            .unwrap(),
            plan(),
            RunLimits::default(),
        )
        .unwrap(),
        RunManifest::new(CSV, p(), vec![1, 2], RunLimits::default()).unwrap(),
        RunManifest::new(
            CSV,
            p(),
            plan(),
            RunLimits {
                max_decisions: 4,
                ..RunLimits::default()
            },
        )
        .unwrap(),
    ];
    for expected in alternatives {
        assert!(matches!(
            RunStore::open(t.path("run"), Some(&expected)),
            Err(RecoveryError::ExpectedManifest)
        ));
        assert_eq!(disk(&t.path("run")), before);
    }
    assert_eq!(
        RunStore::open(t.path("run"), Some(&m()))
            .unwrap()
            .durable_count()
            .unwrap(),
        1
    );
}
#[test]
fn input_and_plan_preflight_errors_do_not_create_a_directory() {
    let t = Temp::new();
    let wrong = RunStore::create(t.path("run"), b"invalid", m());
    assert!(wrong.is_err());
    assert!(!t.path("run").exists());
    let first = std::str::from_utf8(CSV).unwrap().lines().next().unwrap();
    let mut rows = String::new();
    for n in 0..257u64 {
        let mut cols: Vec<_> = first.split(',').map(String::from).collect();
        let open = 1704067200000 + n * 60000;
        cols[0] = open.to_string();
        cols[6] = (open + 59999).to_string();
        rows.push_str(&(cols.join(",") + "\n"));
    }
    assert!(RunManifest::new(
        rows.as_bytes(),
        p(),
        vec![1704067200000000 + 257 * 60000000 + 100000],
        RunLimits::default()
    )
    .is_err());
    assert!(!t.path("run").exists());
}
#[test]
fn resource_exhaustion_keeps_the_previous_certified_prefix_usable() {
    for limits in [
        RunLimits {
            max_physical_journal_bytes: 124,
            ..RunLimits::default()
        },
        RunLimits {
            max_decoded_record_bytes: 1,
            ..RunLimits::default()
        },
        RunLimits {
            max_report_bytes: 1,
            ..RunLimits::default()
        },
        RunLimits {
            max_snapshot_bytes: 1,
            ..RunLimits::default()
        },
    ] {
        let t = Temp::new();
        let m = RunManifest::new(CSV, p(), plan(), limits).unwrap();
        let mut s = RunStore::create(t.path("run"), CSV, m).unwrap();
        let before = disk(&t.path("run"));
        assert!(s.step(1).is_err());
        assert!(!s.recovery_required());
        assert_eq!(s.durable_count().unwrap(), 0);
        assert_eq!(disk(&t.path("run")), before);
        drop(s);
        assert_eq!(
            RunStore::open(t.path("run"), None)
                .unwrap()
                .durable_count()
                .unwrap(),
            0
        );
    }
}
#[test]
fn valid_partial_or_corrupt_uncommitted_tail_never_advances_head() {
    for tail_kind in 0..3 {
        let t = Temp::new();
        let mut s = RunStore::create(t.path("run"), CSV, m()).unwrap();
        s.step(1).unwrap();
        let head = fs::read(t.path("run/HEAD")).unwrap();
        let committed = fs::read(t.path("run/JOURNAL")).unwrap();
        s.step(2).unwrap();
        let full = fs::read(t.path("run/JOURNAL")).unwrap();
        drop(s);
        fs::write(t.path("run/HEAD"), &head).unwrap();
        if tail_kind == 1 {
            fs::write(t.path("run/JOURNAL"), &full[..full.len() - 10]).unwrap();
        }
        if tail_kind == 2 {
            let mut b = full;
            b[committed.len() + 30] ^= 1;
            fs::write(t.path("run/JOURNAL"), b).unwrap();
        }
        fs::write(t.path("run/HEAD.pending"), b"not authoritative").unwrap();
        let mut s = RunStore::open(t.path("run"), None).unwrap();
        assert_eq!(s.durable_count().unwrap(), 1);
        assert_eq!(fs::read(t.path("run/JOURNAL")).unwrap(), committed);
        assert!(!t.path("run/HEAD.pending").exists());
        s.step(2).unwrap();
    }
}
#[test]
fn committed_corruption_or_missing_components_never_roll_back_or_repair() {
    for name in ["HEAD", "MANIFEST", "SOURCE", "JOURNAL"] {
        for truncate in [false, true] {
            let t = Temp::new();
            let mut s = RunStore::create(t.path("run"), CSV, m()).unwrap();
            s.step(1).unwrap();
            drop(s);
            let original = fs::read(t.path(&format!("run/{name}"))).unwrap();
            let mut b = original.clone();
            if truncate {
                b.pop();
            } else {
                let mid = b.len() / 2;
                b[mid] ^= 1;
            }
            fs::write(t.path(&format!("run/{name}")), b).unwrap();
            fs::write(t.path("run/HEAD.pending"), b"tempting alternative").unwrap();
            let before = disk(&t.path("run"));
            assert!(RunStore::open(t.path("run"), None).is_err());
            assert_eq!(disk(&t.path("run")), before);
        }
    }
    for name in ["HEAD", "LOCK"] {
        let t = Temp::new();
        drop(RunStore::create(t.path("run"), CSV, m()).unwrap());
        fs::remove_file(t.path(&format!("run/{name}"))).unwrap();
        assert!(RunStore::open(t.path("run"), None).is_err());
        assert!(!t.path(&format!("run/{name}")).exists());
        assert!(RunStore::create(t.path("run"), CSV, m()).is_err());
    }
}
#[test]
fn source_capture_is_independent_of_later_original_path_changes() {
    let t = Temp::new();
    fs::write(t.path("input.csv"), CSV).unwrap();
    let mut command = cli();
    command
        .arg("create")
        .arg(t.path("run"))
        .arg(t.path("input.csv"))
        .args(["BTCUSDT", "ms", "100", "close", "1704067260099000"]);
    let out = command.output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    fs::write(t.path("input.csv"), b"corrupt replacement").unwrap();
    let mut s = RunStore::open(t.path("run"), None).unwrap();
    assert!(std::str::from_utf8(s.step(1).unwrap().report())
        .unwrap()
        .contains("UNRESOLVED"));
}
#[test]
fn changed_future_suffix_or_duplicate_creates_new_run_without_rebinding_earlier_core() {
    let t = Temp::new();
    let first = std::str::from_utf8(CSV)
        .unwrap()
        .lines()
        .next()
        .unwrap()
        .to_string()
        + "\n";
    let duplicate = [CSV, first.as_bytes()].concat();
    let mut previous: Option<(Hash, DecisionBundle)> = None;
    for (i, input) in [CSV, first.as_bytes(), &duplicate].iter().enumerate() {
        let manifest =
            RunManifest::new(input, p(), vec![1704067260099000], RunLimits::default()).unwrap();
        let id = manifest.run_id();
        let mut s = RunStore::create(t.path(&i.to_string()), input, manifest).unwrap();
        let b = s.step(1).unwrap();
        if let Some((old_id, old)) = previous {
            assert_ne!(id, old_id);
            assert_eq!(b.decision_id(), old.decision_id());
            assert_eq!(b.snapshot(), old.snapshot());
            assert_ne!(b.report(), old.report());
        }
        previous = Some((id, b));
    }
}
#[test]
fn second_process_cannot_enter_a_locked_run_and_lock_releases_on_drop() {
    let t = Temp::new();
    let s = RunStore::create(t.path("run"), CSV, m()).unwrap();
    let out = cli().arg("status").arg(t.path("run")).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("LockBusy"));
    drop(s);
    assert!(cli()
        .arg("status")
        .arg(t.path("run"))
        .status()
        .unwrap()
        .success());
}
#[test]
fn independent_cli_processes_resume_and_export_the_exact_isolated_decision() {
    let t = Temp::new();
    let mut c = cli();
    create_args(&mut c, &t.path("run"));
    assert!(c.output().unwrap().status.success());
    for n in 1..=3 {
        let out = cli()
            .arg("step")
            .arg(t.path("run"))
            .arg(n.to_string())
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    assert!(cli()
        .arg("export")
        .arg(t.path("run"))
        .arg("3")
        .arg(t.path("report.json"))
        .arg(t.path("core.bin"))
        .output()
        .unwrap()
        .status
        .success());
    let view = Dataset::from_csv(CSV, p(), Limits::default())
        .unwrap()
        .decision(plan()[2])
        .unwrap();
    assert_eq!(
        fs::read(t.path("report.json")).unwrap(),
        (view.report_json().unwrap() + "\n").as_bytes()
    );
    assert_eq!(
        fs::read(t.path("core.bin")).unwrap(),
        view.snapshot_bytes().unwrap()
    );
    assert!(
        std::str::from_utf8(&fs::read(t.path("report.json")).unwrap())
            .unwrap()
            .contains("UNRESOLVED")
    );
}
#[test]
fn symlinked_control_files_are_not_followed() {
    for name in ["SOURCE", "HEAD", "MANIFEST", "JOURNAL", "LOCK"] {
        let t = Temp::new();
        drop(RunStore::create(t.path("run"), CSV, m()).unwrap());
        fs::rename(t.path(&format!("run/{name}")), t.path("elsewhere")).unwrap();
        std::os::unix::fs::symlink(t.path("elsewhere"), t.path(&format!("run/{name}"))).unwrap();
        assert!(RunStore::open(t.path("run"), None).is_err());
    }
}
#[cfg(not(feature = "fault-injection"))]
#[test]
fn normal_binary_ignores_fault_injection_environment() {
    let t = Temp::new();
    let mut c = cli();
    create_args(&mut c, &t.path("run"));
    let out = c
        .env("MDC_REPLAY_V2_KILL_AT", "head-half")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(out.status.success());
}

#[cfg(feature = "fault-injection")]
fn kill_at(mut cmd: Command, point: &str, run: &std::path::Path) {
    use std::{
        io::{BufRead, BufReader},
        os::unix::process::ExitStatusExt,
        sync::mpsc,
        time::Duration,
    };
    let mut child = cmd
        .env("MDC_REPLAY_V2_KILL_AT", point)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    let needle = format!("MDC_REPLAY_V2_KILL_READY={point}");
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if line == needle {
                let _ = tx.send(());
                break;
            }
        }
    });
    let ready = rx.recv_timeout(Duration::from_secs(30));
    if ready.is_ok() {
        assert!(matches!(
            RunStore::open(run, None),
            Err(RecoveryError::LockBusy)
        ));
    }
    let _ = child.kill();
    let status = child.wait().unwrap();
    reader.join().unwrap();
    assert!(ready.is_ok(), "fault point {point} not reached");
    assert_eq!(status.signal(), Some(9));
}
#[cfg(feature = "fault-injection")]
#[test]
fn sigkill_all_commit_boundaries_recovers_only_a_certified_prefix() {
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
        let mut s = RunStore::create(t.path("run"), CSV, m()).unwrap();
        s.step(1).unwrap();
        drop(s);
        let mut c = cli();
        c.arg("step").arg(t.path("run")).arg("2");
        kill_at(c, point, &t.path("run"));
        let mut s = RunStore::open(t.path("run"), None).unwrap();
        assert!((1..=2).contains(&s.durable_count().unwrap()));
        if ["head-renamed", "head-dir-synced", "before-reply"].contains(&point) {
            assert_eq!(s.durable_count().unwrap(), 2);
        }
        s.step(2).unwrap();
        assert_eq!(s.durable_count().unwrap(), 2);
    }
}
#[cfg(feature = "fault-injection")]
#[test]
fn sigkill_create_boundaries_recover_complete_empty_or_reject_incomplete_run() {
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
        let mut c = cli();
        create_args(&mut c, &t.path("run"));
        kill_at(c, point, &t.path("run"));
        if let Ok(s) = RunStore::open(t.path("run"), None) {
            assert_eq!(s.durable_count().unwrap(), 0);
        }
        assert!(RunStore::create(t.path("run"), CSV, m()).is_err());
    }
}
#[cfg(feature = "fault-injection")]
#[test]
fn sigkill_tail_repair_does_not_promote_the_uncommitted_suffix() {
    for point in ["tail-truncated", "tail-synced"] {
        let t = Temp::new();
        let mut s = RunStore::create(t.path("run"), CSV, m()).unwrap();
        s.step(1).unwrap();
        drop(s);
        fs::OpenOptions::new()
            .append(true)
            .open(t.path("run/JOURNAL"))
            .unwrap()
            .write_all(b"suffix")
            .unwrap();
        let mut c = cli();
        c.arg("status").arg(t.path("run"));
        kill_at(c, point, &t.path("run"));
        assert_eq!(
            RunStore::open(t.path("run"), None)
                .unwrap()
                .durable_count()
                .unwrap(),
            1
        );
    }
}
