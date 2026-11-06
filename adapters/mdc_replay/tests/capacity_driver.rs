#![cfg(target_os = "linux")]
use mdc_replay::{recovery::RunStore, Dataset, LagAnchor, Limits, Profile, Symbol, TimestampUnit};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "mdc-capacity-driver-{}-{}",
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
fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mdc-replay-capacity"))
}
fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}
fn create(t: &Temp, file: &str, unit: &str, n: &str) -> std::process::Output {
    bin()
        .arg("create")
        .arg(t.run())
        .arg(fixture(file))
        .args(["BTCUSDT", unit, "100", "close", n])
        .output()
        .unwrap()
}
#[test]
fn plans_are_derived_by_the_real_normalizer_for_both_timestamp_eras() {
    for (file, unit, tag) in [
        (
            "BTCUSDT-1m-2024-01-01-first3.csv",
            "ms",
            TimestampUnit::Milliseconds,
        ),
        (
            "BTCUSDT-1m-2025-01-01-first3.csv",
            "us",
            TimestampUnit::Microseconds,
        ),
    ] {
        let t = Temp::new();
        let r = create(&t, file, unit, "3");
        assert!(r.status.success());
        assert!(String::from_utf8_lossy(&r.stdout).contains("\"outcome\":\"Created\""));
        let p = Profile::new(Symbol::BtcUsdt, tag, 100, LagAnchor::ReportedClose).unwrap();
        let d = Dataset::from_csv(&fs::read(fixture(file)).unwrap(), p, Limits::default()).unwrap();
        let expected: Vec<_> = d.evidence().iter().map(|e| e.available_at()).collect();
        assert_eq!(
            RunStore::open(t.run(), None).unwrap().manifest().cutoffs(),
            expected
        );
    }
}
#[test]
fn rejected_input_or_plan_never_creates_a_run_or_zero_fallback() {
    let t = Temp::new();
    let r = create(&t, "BTCUSDT-1m-2024-01-01-first3.csv", "ms", "1440");
    assert!(r.status.success());
    let s = String::from_utf8_lossy(&r.stdout);
    assert!(s.contains("PreflightRejected") && s.contains("InsufficientRows"));
    assert!(!t.run().exists());
    let r = create(&t, "BTCUSDT-1m-2024-01-01-first3.csv", "us", "1");
    assert!(!r.status.success());
    assert!(!t.run().exists());
}
#[test]
fn measurement_does_not_overwrite_an_existing_run() {
    let t = Temp::new();
    assert!(create(&t, "BTCUSDT-1m-2024-01-01-first3.csv", "ms", "2")
        .status
        .success());
    let before = fs::read(t.run().join("HEAD")).unwrap();
    let r = create(&t, "BTCUSDT-1m-2024-01-01-first3.csv", "ms", "3");
    assert!(String::from_utf8_lossy(&r.stdout).contains("\"outcome\":\"Error\""));
    assert_eq!(fs::read(t.run().join("HEAD")).unwrap(), before);
    assert_eq!(
        RunStore::open(t.run(), None)
            .unwrap()
            .manifest()
            .cutoffs()
            .len(),
        2
    );
}
#[cfg(feature = "fault-injection")]
#[test]
fn lost_measurement_reply_is_resolved_by_fresh_recovery() {
    use std::{
        io::{BufRead, BufReader},
        process::Stdio,
        sync::mpsc,
        time::Duration,
    };
    let t = Temp::new();
    assert!(create(&t, "BTCUSDT-1m-2024-01-01-first3.csv", "ms", "3")
        .status
        .success());
    let mut child = bin()
        .arg("commit")
        .arg(t.run())
        .env("MDC_REPLAY_KILL_AT", "before-reply")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        for s in BufReader::new(stdout).lines() {
            if s.unwrap().contains("MDC_REPLAY_KILL_READY=before-reply") {
                tx.send(()).unwrap();
                break;
            }
        }
    });
    let ready = rx.recv_timeout(Duration::from_secs(30));
    let _ = child.kill();
    child.wait().unwrap();
    reader.join().unwrap();
    assert!(ready.is_ok());
    let recovered = bin().arg("recover").arg(t.run()).output().unwrap();
    let text = String::from_utf8_lossy(&recovered.stdout);
    assert!(
        recovered.status.success()
            && text.contains("\"outcome\":\"Recovered\"")
            && text.contains("\"committed\":1")
    );
    assert_eq!(
        RunStore::open(t.run(), None)
            .unwrap()
            .durable_count()
            .unwrap(),
        1
    );
}
