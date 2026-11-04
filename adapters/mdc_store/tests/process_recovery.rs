//! Fresh processes and SIGKILL tests. Fault hooks exist only under explicit feature.
#[cfg(feature = "fault-injection")]
use mdc_core::{
    contract::registry::VersionedId,
    pipeline::{ObservationRequest, SnapshotRequest},
};
#[cfg(feature = "fault-injection")]
use mdc_store::Mode;
use mdc_store::{Limits, Store};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
#[cfg(feature = "fault-injection")]
use std::{
    io::{BufRead, BufReader},
    process::Stdio,
    time::Duration,
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "mdc-012-process-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn db(&self) -> PathBuf {
        self.0.join("db")
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn driver() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mdc-recovery"))
}
fn seed(t: &Temp) {
    let r = driver().arg("seed").arg(t.db()).output().unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
}
#[test]
fn producer_and_two_consumers_recover_exact_wal_and_checkpoint_histories() {
    for checkpoint in [false, true] {
        let t = Temp::new();
        let oracle = t.0.join("expected.bin");
        let mut cmd = driver();
        cmd.arg("produce").arg(t.db()).arg(&oracle);
        if checkpoint {
            cmd.arg("checkpoint");
        }
        let produced = cmd.output().unwrap();
        assert!(
            produced.status.success(),
            "{}",
            String::from_utf8_lossy(&produced.stderr)
        );
        assert!(String::from_utf8_lossy(&produced.stdout)
            .contains("operations=38 durable=38 objects=31 revision=2"));
        for _ in 0..2 {
            let consumed = driver()
                .arg("consume")
                .arg(t.db())
                .arg(&oracle)
                .output()
                .unwrap();
            assert!(
                consumed.status.success(),
                "{}",
                String::from_utf8_lossy(&consumed.stderr)
            );
            assert_eq!(produced.stdout, consumed.stdout);
        }
    }
}
#[test]
fn complete_volatile_suffix_is_not_mistaken_for_durable_after_process_exit() {
    let t = Temp::new();
    seed(&t);
    let r = driver().arg("volatile").arg(t.db()).output().unwrap();
    assert!(r.status.success());
    assert!(String::from_utf8_lossy(&r.stdout).contains("live=2 durable=1"));
    let s = Store::open(t.db(), Limits::default()).unwrap();
    assert_eq!((s.live_seq(), s.durable_seq()), (1, 1));
}
#[test]
fn different_process_cannot_take_an_existing_writer_lock() {
    let t = Temp::new();
    seed(&t);
    let _s = Store::open(t.db(), Limits::default()).unwrap();
    let output = driver().arg("volatile").arg(t.db()).output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("LockBusy"));
}
#[cfg(feature = "fault-injection")]
#[test]
fn sigkill_at_append_sync_and_head_boundaries_preserves_ack_prefix_and_deduplicates_lost_ack() {
    for point in [
        "wal-half",
        "wal-written",
        "wal-synced",
        "head-half",
        "head-written",
        "head-synced",
        "head-renamed",
        "head-dir-synced",
    ] {
        let t = Temp::new();
        seed(&t);
        kill_at(&t, point, "append");
        let mut s = Store::open(t.db(), Limits::default()).unwrap();
        assert_eq!(
            s.durable_seq(),
            if matches!(point, "head-renamed" | "head-dir-synced") {
                2
            } else {
                1
            },
            "{point}"
        );
        let req = SnapshotRequest::Admit(ObservationRequest {
            source: VersionedId::new("unresolved", 1).unwrap(),
            timestamp: Some(2),
            payload: 2u64.to_le_bytes().to_vec(),
            acquisition_context: "request/2".into(),
        });
        s.submit("two", req, Mode::Durable).unwrap();
        assert_eq!(s.live_seq(), 2, "{point}: lost ACK created duplicate call");
    }
}
#[cfg(feature = "fault-injection")]
#[test]
fn sigkill_at_checkpoint_and_cleanup_boundaries_preserves_selected_snapshot_wal_pair() {
    for point in [
        "generation-half",
        "generation-written",
        "generation-synced",
        "pair-dir-synced",
        "head-half",
        "head-written",
        "head-synced",
        "head-renamed",
        "head-dir-synced",
        "cleanup-unlinked",
    ] {
        let t = Temp::new();
        seed(&t);
        kill_at(&t, point, "checkpoint");
        let s = Store::open(t.db(), Limits::default()).unwrap();
        assert_eq!((s.live_seq(), s.durable_seq()), (1, 1), "{point}");
        assert!(s.lookup("one").unwrap().is_some());
    }
}
#[cfg(feature = "fault-injection")]
fn kill_at(t: &Temp, point: &str, mode: &str) {
    let mut child = driver()
        .arg("worker")
        .arg(t.db())
        .arg(mode)
        .env("MDC_STORE_KILL_AT", point)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        let mut line = String::new();
        let result = BufReader::new(stdout).read_line(&mut line).map(|_| line);
        let _ = tx.send(result);
    });
    let ready = rx.recv_timeout(Duration::from_secs(20));
    if !matches!(&ready,Ok(Ok(line)) if line.trim()==format!("MDC_STORE_KILL_READY={point}")) {
        let _ = child.kill();
        let output = child.wait_with_output().unwrap();
        panic!(
            "fault point {point}: {ready:?}; stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    child.kill().unwrap();
    let status = child.wait().unwrap();
    assert!(!status.success());
    reader.join().unwrap();
}
