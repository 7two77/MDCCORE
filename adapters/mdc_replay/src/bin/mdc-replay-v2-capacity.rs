//! Host-only measurements. No timer or measurement field enters core/run bytes.
#[cfg(target_os = "linux")]
mod host {
    use mdc_replay::{
        recovery::RecoveryError,
        recovery_v2::{
            RunManifestV2 as RunManifest, RunStoreV2 as RunStore, StorageLimits as RunLimits,
        },
        Dataset, LagAnchor, Limits, Profile, Symbol, TimestampUnit,
    };
    use sha2::{Digest, Sha256};
    use std::{fmt::Write as _, fs, io::Read, path::Path, time::Instant};
    type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
    fn text(s: &str) -> String {
        let mut out = String::from("\"");
        for c in s.chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                c if c < ' ' => write!(&mut out, "\\u{:04x}", c as u32).unwrap(),
                c => out.push(c),
            }
        }
        out.push('"');
        out
    }
    fn hex(b: &[u8]) -> String {
        let mut s = String::new();
        for v in b {
            write!(&mut s, "{v:02x}").unwrap();
        }
        s
    }
    fn hash(b: &[u8]) -> String {
        hex(&Sha256::digest(b))
    }
    fn array(v: &[u64]) -> String {
        format!(
            "[{}]",
            v.iter().map(u64::to_string).collect::<Vec<_>>().join(",")
        )
    }
    fn object(v: &[(&str, String)]) -> String {
        format!(
            "{{{}}}",
            v.iter()
                .map(|(k, v)| format!("{}:{v}", text(k)))
                .collect::<Vec<_>>()
                .join(",")
        )
    }
    fn optional(v: Option<u64>) -> String {
        v.map(|n| n.to_string()).unwrap_or_else(|| "null".into())
    }
    fn limits(l: RunLimits) -> String {
        object(&[
            ("max_decisions", l.max_decisions.to_string()),
            ("max_manifest_bytes", l.max_manifest_bytes.to_string()),
            (
                "max_encoded_record_bytes",
                l.max_encoded_record_bytes.to_string(),
            ),
            (
                "max_physical_journal_bytes",
                l.max_physical_journal_bytes.to_string(),
            ),
            ("max_report_bytes", l.max_report_bytes.to_string()),
            ("max_snapshot_bytes", l.max_snapshot_bytes.to_string()),
            (
                "max_decoded_record_bytes",
                l.max_decoded_record_bytes.to_string(),
            ),
            (
                "max_total_decoded_blob_bytes",
                l.max_total_decoded_blob_bytes.to_string(),
            ),
        ])
    }
    fn error_class(e: &RecoveryError) -> &'static str {
        match e {
            RecoveryError::Limit(_) => "Limit",
            RecoveryError::Indeterminate(_) => "Indeterminate",
            RecoveryError::Replay(_) => "Replay",
            RecoveryError::Format(_) => "Format",
            RecoveryError::Integrity(_) => "Integrity",
            RecoveryError::Index => "Index",
            _ => "Other",
        }
    }
    fn common(phase: &str, outcome: &str, fields: Vec<(&str, String)>) -> String {
        let l = Limits::default();
        let mut v = vec![
            ("schema", text("MDC-STORAGE-V2-PHASE-001")),
            ("phase", text(phase)),
            ("outcome", text(outcome)),
            (
                "replay_limits",
                object(&[
                    ("max_input_bytes", l.max_input_bytes.to_string()),
                    ("max_rows", l.max_rows.to_string()),
                    ("max_line_bytes", l.max_line_bytes.to_string()),
                    ("max_visible_bars", l.max_visible_bars.to_string()),
                ]),
            ),
            (
                "target",
                object(&[
                    ("arch", text(std::env::consts::ARCH)),
                    ("pointer_width", usize::BITS.to_string()),
                    (
                        "endian",
                        text(if cfg!(target_endian = "little") {
                            "little"
                        } else {
                            "big"
                        }),
                    ),
                ]),
            ),
        ];
        v.extend(fields);
        object(&v)
    }
    fn manifest(m: &RunManifest) -> Vec<(&'static str, String)> {
        vec![
            ("run_id", text(&hex(&m.run_id()))),
            ("source_sha256", text(&hex(&m.source_sha256()))),
            ("cutoffs", array(m.cutoffs())),
            ("requested", m.cutoffs().len().to_string()),
            ("limits", limits(m.limits())),
        ]
    }
    fn bundles(s: &mut RunStore) -> Result<String> {
        let mut v = Vec::new();
        for index in 1..=s.durable_count()? {
            let b = s.lookup_owned(index)?.ok_or("missing committed bundle")?;
            v.push(object(&[
                ("index", index.to_string()),
                ("cutoff", b.cutoff().to_string()),
                ("report_bytes", b.report().len().to_string()),
                ("snapshot_bytes", b.snapshot().len().to_string()),
                ("report_sha256", text(&hash(b.report()))),
                ("snapshot_sha256", text(&hash(b.snapshot()))),
            ]));
        }
        Ok(format!("[{}]", v.join(",")))
    }
    fn boundary(dir: &Path) -> Result<(Vec<u8>, u64)> {
        let mut h = Vec::new();
        fs::File::open(dir.join("HEAD"))?
            .take(513)
            .read_to_end(&mut h)?;
        if h.len() > 512 {
            return Err("HEAD bound".into());
        }
        Ok((h, fs::metadata(dir.join("JOURNAL"))?.len()))
    }
    pub fn run() -> Result<String> {
        let a: Vec<_> = std::env::args().collect();
        let phase = a.get(1).ok_or("phase required")?;
        if phase == "create" && matches!(a.len(), 9..=11) {
            let profile = Profile::new(
                match a[4].as_str() {
                    "BTCUSDT" => Symbol::BtcUsdt,
                    "ETHUSDT" => Symbol::EthUsdt,
                    _ => return Err("unsupported symbol".into()),
                },
                match a[5].as_str() {
                    "ms" => TimestampUnit::Milliseconds,
                    "us" => TimestampUnit::Microseconds,
                    _ => return Err("explicit ms/us required".into()),
                },
                a[6].parse()?,
                match a[7].as_str() {
                    "close" => LagAnchor::ReportedClose,
                    "end" => LagAnchor::MinuteEnd,
                    _ => return Err("close/end required".into()),
                },
            )?;
            let n: usize = a[8].parse()?;
            if n == 0 || n > 4096 {
                return Err("requested count must be 1..4096".into());
            }
            let input_start = Instant::now();
            let mut source = Vec::new();
            fs::File::open(&a[3])?
                .take(4 * 1024 * 1024 + 1)
                .read_to_end(&mut source)?;
            let dataset = Dataset::from_csv(&source, profile, Limits::default())?;
            let input_ns = input_start.elapsed().as_nanos();
            let cutoffs: Vec<_> = dataset
                .evidence()
                .iter()
                .take(n)
                .map(|e| e.available_at())
                .collect();
            let selected_limits = RunLimits {
                max_physical_journal_bytes: a
                    .get(9)
                    .map(|v| v.parse())
                    .transpose()?
                    .unwrap_or(RunLimits::default().max_physical_journal_bytes),
                max_total_decoded_blob_bytes: a
                    .get(10)
                    .map(|v| v.parse())
                    .transpose()?
                    .unwrap_or(RunLimits::default().max_total_decoded_blob_bytes),
                ..RunLimits::default()
            };
            let mut fields = vec![
                ("requested", n.to_string()),
                ("source_bytes", source.len().to_string()),
                ("source_sha256", text(dataset.raw_sha256())),
                ("unique_rows", dataset.evidence().len().to_string()),
                ("duplicate_rows", dataset.duplicate_rows().to_string()),
                ("cutoffs", array(&cutoffs)),
                ("limits", limits(selected_limits)),
                ("input_ns", input_ns.to_string()),
            ];
            if cutoffs.len() != n {
                fields.push(("error_class", text("InsufficientRows")));
                return Ok(common(phase, "PreflightRejected", fields));
            }
            let start = Instant::now();
            let m = RunManifest::new(&source, profile, cutoffs, selected_limits);
            fields.push(("manifest_ns", start.elapsed().as_nanos().to_string()));
            let m = match m {
                Ok(m) => m,
                Err(e) => {
                    fields.push(("error_class", text(error_class(&e))));
                    fields.push(("error", text(&e.to_string())));
                    return Ok(common(phase, "PreflightRejected", fields));
                }
            };
            let run_id = hex(&m.run_id());
            let start = Instant::now();
            let s = RunStore::create(&a[2], &source, m);
            fields.push(("create_ns", start.elapsed().as_nanos().to_string()));
            match s {
                Ok(s) => {
                    fields.push(("run_id", text(&run_id)));
                    fields.push(("committed", s.durable_count()?.to_string()));
                    Ok(common(phase, "Created", fields))
                }
                Err(e) => {
                    fields.push(("error_class", text(error_class(&e))));
                    fields.push(("error", text(&e.to_string())));
                    Ok(common(phase, "Error", fields))
                }
            }
        } else if ((phase == "commit" || phase == "recover") && a.len() == 3)
            || (phase == "retry" && a.len() == 4)
        {
            let dir = Path::new(&a[2]);
            let start = Instant::now();
            let s = RunStore::open(dir, None);
            let open_ns = start.elapsed().as_nanos();
            let mut s = match s {
                Ok(s) => s,
                Err(e) => {
                    return Ok(common(
                        phase,
                        "Error",
                        vec![
                            ("open_ns", open_ns.to_string()),
                            ("error_class", text(error_class(&e))),
                            ("error", text(&e.to_string())),
                        ],
                    ))
                }
            };
            let mut fields = manifest(s.manifest());
            fields.push(("open_ns", open_ns.to_string()));
            let mut outcome = "Recovered";
            let mut steps = Vec::new();
            if phase == "commit" {
                outcome = "Completed";
                while let Some(index) = s.next_index()? {
                    let before = boundary(dir)?;
                    let start = Instant::now();
                    let result = s.step(index);
                    let ns = start.elapsed().as_nanos();
                    let mut step = vec![("index", index.to_string()), ("api_ns", ns.to_string())];
                    if let Err(e) = result {
                        outcome = if matches!(e, RecoveryError::Limit(_)) {
                            "Limited"
                        } else {
                            "Indeterminate"
                        };
                        step.push(("outcome", text(outcome)));
                        fields.push(("failed_index", index.to_string()));
                        fields.push(("error_class", text(error_class(&e))));
                        fields.push(("error", text(&e.to_string())));
                        let after = boundary(dir)?;
                        fields.push(("failed_head_unchanged", (before.0 == after.0).to_string()));
                        fields.push((
                            "failed_journal_length_unchanged",
                            (before.1 == after.1).to_string(),
                        ));
                        steps.push(object(&step));
                        break;
                    }
                    step.push(("outcome", text("Committed")));
                    steps.push(object(&step));
                }
            } else if phase == "retry" {
                let index: u64 = a[3].parse()?;
                if s.lookup_owned(index)?.is_none() {
                    return Err("retry requires a committed index".into());
                }
                let before = boundary(dir)?;
                let start = Instant::now();
                let result = s.step(index);
                fields.push(("retry_ns", start.elapsed().as_nanos().to_string()));
                result?;
                let after = boundary(dir)?;
                fields.push(("retry_index", index.to_string()));
                fields.push(("retry_head_unchanged", (before.0 == after.0).to_string()));
                fields.push((
                    "retry_journal_length_unchanged",
                    (before.1 == after.1).to_string(),
                ));
                outcome = "Retried";
            }
            fields.push(("committed", optional(s.durable_count().ok())));
            fields.push((
                "physical_journal_bytes",
                s.physical_journal_bytes()?.to_string(),
            ));
            fields.push(("decoded_blob_bytes", s.decoded_blob_bytes()?.to_string()));
            fields.push(("next_index", optional(s.next_index().ok().flatten())));
            fields.push(("steps", format!("[{}]", steps.join(","))));
            if !s.recovery_required() {
                let start = Instant::now();
                fields.push(("bundles", bundles(&mut s)?));
                fields.push(("diagnostic_hash_ns", start.elapsed().as_nanos().to_string()));
            }
            Ok(common(phase, outcome, fields))
        } else {
            Err("create DIR CSV SYMBOL ms|us LAG_MS close|end COUNT [JOURNAL_BYTES]; commit DIR; recover DIR; retry DIR INDEX".into())
        }
    }
}
fn main() {
    #[cfg(target_os = "linux")]
    match host::run() {
        Ok(value) => println!("{value}"),
        Err(e) => {
            eprintln!("MDC_STORAGE_V2_PHASE=ERROR {e}");
            std::process::exit(1);
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        eprintln!("MDC_STORAGE_V2_PHASE=ERROR Linux required");
        std::process::exit(1);
    }
}
