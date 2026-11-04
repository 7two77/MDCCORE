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
            "mdc-014-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mdc-binance-replay"))
}
fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/BTCUSDT-1m-2024-01-01-first3.csv")
}
#[test]
fn three_independent_processes_produce_identical_reports_and_snapshots() {
    let t = Temp::new();
    let mut first = None;
    for n in 0..3 {
        let report = t.0.join(format!("{n}.json"));
        let snapshot = t.0.join(format!("{n}.bin"));
        let r = binary()
            .args(["decision"])
            .arg(fixture())
            .args(["BTCUSDT", "ms", "100", "close", "1704067320049000"])
            .arg(&report)
            .arg(&snapshot)
            .output()
            .unwrap();
        assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
        let bytes = (fs::read(report).unwrap(), fs::read(snapshot).unwrap());
        if let Some(first) = &first {
            assert_eq!(&bytes, first);
        } else {
            first = Some(bytes);
        }
    }
}
#[test]
fn parse_error_never_writes_a_successful_report() {
    let t = Temp::new();
    let bad = t.0.join("bad.csv");
    let report = t.0.join("report.json");
    fs::write(
        &bad,
        include_str!("fixtures/BTCUSDT-1m-2024-01-01-first3.csv").replace("42298.61000000", "NaN"),
    )
    .unwrap();
    let r = binary()
        .arg("decision")
        .arg(bad)
        .args(["BTCUSDT", "ms", "100", "close", "1704067320049000"])
        .arg(&report)
        .output()
        .unwrap();
    assert!(!r.status.success());
    assert!(!report.exists());
    assert!(String::from_utf8_lossy(&r.stderr).contains("Row"));
}
#[test]
fn existing_input_and_outputs_are_not_overwritten() {
    let t = Temp::new();
    let input = t.0.join("input.csv");
    let original = fs::read(fixture()).unwrap();
    fs::write(&input, &original).unwrap();
    let r = binary()
        .arg("decision")
        .arg(&input)
        .args(["BTCUSDT", "ms", "100", "close", "1704067320049000"])
        .arg(&input)
        .output()
        .unwrap();
    assert!(!r.status.success());
    assert_eq!(fs::read(input).unwrap(), original);
}
#[test]
fn timestamp_unit_negative_lag_and_symbol_require_explicit_supported_values() {
    let t = Temp::new();
    for (symbol, unit, lag) in [
        ("BTCUSDT", "auto", "100"),
        ("BTCUSDT", "ms", "-1"),
        ("DERIBIT", "ms", "100"),
    ] {
        let out = t.0.join("none.json");
        let r = binary()
            .arg("decision")
            .arg(fixture())
            .args([symbol, unit, lag, "close", "1704067320049000"])
            .arg(&out)
            .output()
            .unwrap();
        assert!(!r.status.success());
        assert!(!out.exists());
    }
}
#[test]
fn inspect_parses_the_official_numeric_schema_without_claiming_receipt_time() {
    let r = binary()
        .arg("inspect")
        .arg(fixture())
        .args(["BTCUSDT", "ms", "100", "close"])
        .output()
        .unwrap();
    assert!(r.status.success());
    assert!(String::from_utf8_lossy(&r.stdout)
        .contains("rows=3 unique=3 duplicates=0 unit=ms availability=modeled"));
}
