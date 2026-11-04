use mdc_replay::{Dataset, LagAnchor, Limits, Profile, Symbol, TimestampUnit};
use std::{
    io::{Read, Write},
    path::Path,
};
fn write_new(path: &str, bytes: &[u8]) -> std::io::Result<()> {
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?
        .write_all(bytes)
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = std::env::args().collect();
    if !matches!(a.len(), 7 | 9 | 10) {
        return Err("inspect CSV BTCUSDT|ETHUSDT ms|us LAG_MS close|end; decision adds DECISION_US REPORT [SNAPSHOT]".into());
    }
    let symbol = match a[3].as_str() {
        "BTCUSDT" => Symbol::BtcUsdt,
        "ETHUSDT" => Symbol::EthUsdt,
        _ => return Err("unsupported symbol".into()),
    };
    let unit = match a[4].as_str() {
        "ms" => TimestampUnit::Milliseconds,
        "us" => TimestampUnit::Microseconds,
        _ => return Err("timestamp unit must be explicit ms or us".into()),
    };
    let lag: u64 = a[5].parse()?;
    let anchor = match a[6].as_str() {
        "close" => LagAnchor::ReportedClose,
        "end" => LagAnchor::MinuteEnd,
        _ => return Err("anchor must be close or end".into()),
    };
    let p = Profile::new(symbol, unit, lag, anchor)?;
    let limits = Limits::default();
    let mut bytes = Vec::new();
    std::fs::File::open(Path::new(&a[2]))?
        .take((limits.max_input_bytes + 1) as u64)
        .read_to_end(&mut bytes)?;
    let dataset = Dataset::from_csv(&bytes, p, limits)?;
    match a[1].as_str() {
        "inspect" if a.len() == 7 => println!(
            "MDC_BINANCE_CSV=OK rows={} unique={} duplicates={} unit={} availability=modeled",
            dataset.evidence().len() + dataset.duplicate_rows(),
            dataset.evidence().len(),
            dataset.duplicate_rows(),
            unit.text()
        ),
        "decision" if a.len() == 9 || a.len() == 10 => {
            let cutoff = a[7].parse()?;
            let view = dataset.decision(cutoff)?;
            write_new(&a[8], (view.report_json()? + "\n").as_bytes())?;
            if let Some(path) = a.get(9) {
                write_new(path, &view.snapshot_bytes()?)?;
            }
            println!(
                "MDC_BINANCE_REPLAY=OK eligible={} core_operations={} availability=modeled",
                view.evidence().len(),
                view.session().operation_count()
            );
        }
        _ => return Err("unknown command or argument count".into()),
    }
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("MDC_BINANCE_REPLAY=ERROR {e}");
        std::process::exit(1);
    }
}
