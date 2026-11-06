//! Bounded Binance Spot 1m final-bar replay with explicitly MODELED availability.
mod core_view;
pub mod recovery;
#[cfg(test)]
mod work_counts;
pub use core_view::{AdmittedBar, DecisionView, Field};
use mdc_core::{
    contract::registry::RegistryError,
    pipeline::{PipelineError, SnapshotError},
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
pub const SCALE: i64 = 100_000_000;
const MINUTE_US: u64 = 60_000_000;
const CUTOVER_US: u64 = 1_735_689_600_000_000;
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowIssue {
    Columns,
    UnsignedInteger,
    Decimal,
    Precision,
    Overflow,
    MinuteAlignment,
    Interval,
    TimestampEra,
    Ohlc,
    Volume,
}
#[derive(Debug)]
pub enum Error {
    Profile(&'static str),
    Limit(&'static str),
    EmptyInput,
    Utf8,
    Row {
        line: usize,
        column: usize,
        reason: RowIssue,
    },
    ConflictingBar {
        open_us: u64,
    },
    Registry(RegistryError),
    Core(PipelineError),
    Snapshot(SnapshotError),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
impl From<RegistryError> for Error {
    fn from(e: RegistryError) -> Self {
        Self::Registry(e)
    }
}
impl From<PipelineError> for Error {
    fn from(e: PipelineError) -> Self {
        Self::Core(e)
    }
}
impl From<SnapshotError> for Error {
    fn from(e: SnapshotError) -> Self {
        Self::Snapshot(e)
    }
}
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Symbol {
    BtcUsdt,
    EthUsdt,
}
impl Symbol {
    pub fn text(self) -> &'static str {
        match self {
            Self::BtcUsdt => "BTCUSDT",
            Self::EthUsdt => "ETHUSDT",
        }
    }
    pub fn base(self) -> &'static str {
        match self {
            Self::BtcUsdt => "BTC",
            Self::EthUsdt => "ETH",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimestampUnit {
    Milliseconds,
    Microseconds,
}
impl TimestampUnit {
    fn quantum(self) -> u64 {
        match self {
            Self::Milliseconds => 1000,
            Self::Microseconds => 1,
        }
    }
    pub fn text(self) -> &'static str {
        match self {
            Self::Milliseconds => "ms",
            Self::Microseconds => "us",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LagAnchor {
    ReportedClose,
    MinuteEnd,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Profile {
    symbol: Symbol,
    unit: TimestampUnit,
    lag_us: u64,
    anchor: LagAnchor,
}
impl Profile {
    pub fn new(
        symbol: Symbol,
        unit: TimestampUnit,
        lag_ms: u64,
        anchor: LagAnchor,
    ) -> Result<Self> {
        let lag_us = lag_ms
            .checked_mul(1000)
            .ok_or(Error::Profile("lag overflow"))?;
        if anchor == LagAnchor::ReportedClose && lag_us < unit.quantum() {
            return Err(Error::Profile(
                "final-bar availability precedes minute end; use MinuteEnd anchor for zero lag",
            ));
        }
        Ok(Self {
            symbol,
            unit,
            lag_us,
            anchor,
        })
    }
    pub fn symbol(self) -> Symbol {
        self.symbol
    }
    pub fn unit(self) -> TimestampUnit {
        self.unit
    }
    pub fn lag_us(self) -> u64 {
        self.lag_us
    }
    pub fn anchor(self) -> LagAnchor {
        self.anchor
    }
    fn encode(self, v: &mut Vec<u8>) {
        v.push(match self.symbol {
            Symbol::BtcUsdt => 0,
            Symbol::EthUsdt => 1,
        });
        v.push(match self.unit {
            TimestampUnit::Milliseconds => 0,
            TimestampUnit::Microseconds => 1,
        });
        v.push(match self.anchor {
            LagAnchor::ReportedClose => 0,
            LagAnchor::MinuteEnd => 1,
        });
        v.extend_from_slice(&self.lag_us.to_le_bytes());
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_input_bytes: usize,
    pub max_rows: usize,
    pub max_line_bytes: usize,
    pub max_visible_bars: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_input_bytes: 4 * 1024 * 1024,
            max_rows: 4096,
            max_line_bytes: 4096,
            max_visible_bars: 256,
        }
    }
}
/// Exact decimal e8 conversion. No rounding, exponent, sign or zero fallback.
pub fn decimal_e8(value: &str) -> std::result::Result<i64, RowIssue> {
    if value.is_empty() || value.len() > 64 {
        return Err(RowIssue::Decimal);
    }
    let mut parts = value.split('.');
    let whole = parts.next().ok_or(RowIssue::Decimal)?;
    let fraction = parts.next();
    if whole.is_empty() || parts.next().is_some() {
        return Err(RowIssue::Decimal);
    }
    let mut integer = 0i64;
    for b in whole.bytes() {
        if !b.is_ascii_digit() {
            return Err(RowIssue::Decimal);
        }
        integer = integer
            .checked_mul(10)
            .and_then(|n| n.checked_add(i64::from(b - b'0')))
            .ok_or(RowIssue::Overflow)?;
    }
    let mut frac = 0i64;
    let mut digits = 0;
    if let Some(part) = fraction {
        if part.is_empty() {
            return Err(RowIssue::Decimal);
        }
        for (i, b) in part.bytes().enumerate() {
            if !b.is_ascii_digit() {
                return Err(RowIssue::Decimal);
            }
            if i < 8 {
                frac = frac * 10 + i64::from(b - b'0');
                digits += 1;
            } else if b != b'0' {
                return Err(RowIssue::Precision);
            }
        }
    }
    for _ in digits..8 {
        frac *= 10;
    }
    integer
        .checked_mul(SCALE)
        .and_then(|n| n.checked_add(frac))
        .ok_or(RowIssue::Overflow)
}
fn unsigned(s: &str) -> std::result::Result<u64, RowIssue> {
    if s.is_empty() || s.len() > 20 || !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err(RowIssue::UnsignedInteger);
    }
    s.parse().map_err(|_| RowIssue::Overflow)
}
pub(crate) fn digest(b: &[u8]) -> String {
    let hash = Sha256::digest(b);
    let mut s = String::with_capacity(64);
    for b in hash {
        use std::fmt::Write;
        write!(&mut s, "{b:02x}").expect("String formatting");
    }
    s
}
pub(crate) fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    for byte in bytes {
        write!(&mut out, "{byte:02x}").expect("String formatting");
    }
    out
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CandlePayload {
    values: [i64; 5],
}
impl CandlePayload {
    pub fn open(&self) -> i64 {
        self.values[0]
    }
    pub fn high(&self) -> i64 {
        self.values[1]
    }
    pub fn low(&self) -> i64 {
        self.values[2]
    }
    pub fn close(&self) -> i64 {
        self.values[3]
    }
    pub fn volume(&self) -> i64 {
        self.values[4]
    }
    pub fn values(&self) -> &[i64; 5] {
        &self.values
    }
}
/// A declared-model envelope, not an admitted MDC object or verified receipt time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarketEvidence {
    profile: Profile,
    open_us: u64,
    event_us: u64,
    available_us: u64,
    payload: CandlePayload,
    canonical: Vec<u8>,
    id: String,
}
impl MarketEvidence {
    pub fn profile(&self) -> Profile {
        self.profile
    }
    pub fn open_us(&self) -> u64 {
        self.open_us
    }
    pub fn event_timestamp(&self) -> u64 {
        self.event_us
    }
    pub fn available_at(&self) -> u64 {
        self.available_us
    }
    pub fn payload(&self) -> &CandlePayload {
        &self.payload
    }
    pub fn evidence_id(&self) -> &str {
        &self.id
    }
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical
    }
}
fn parse_row(line: &str, index: usize, p: Profile) -> Result<MarketEvidence> {
    let columns: Vec<_> = line.split(',').collect();
    let row = |column, reason| Error::Row {
        line: index,
        column,
        reason,
    };
    if columns.len() != 12 {
        return Err(row(0, RowIssue::Columns));
    }
    let number = |i: usize| unsigned(columns[i]).map_err(|e| row(i, e));
    let decimal = |i: usize| decimal_e8(columns[i]).map_err(|e| row(i, e));
    let open_us = number(0)?
        .checked_mul(p.unit.quantum())
        .ok_or_else(|| row(0, RowIssue::Overflow))?;
    let event_us = number(6)?
        .checked_mul(p.unit.quantum())
        .ok_or_else(|| row(6, RowIssue::Overflow))?;
    if (open_us >= CUTOVER_US) != (p.unit == TimestampUnit::Microseconds) {
        return Err(row(0, RowIssue::TimestampEra));
    }
    if open_us % MINUTE_US != 0 {
        return Err(row(0, RowIssue::MinuteAlignment));
    }
    let end = open_us
        .checked_add(MINUTE_US)
        .ok_or_else(|| row(0, RowIssue::Overflow))?;
    if event_us != end - p.unit.quantum() {
        return Err(row(6, RowIssue::Interval));
    }
    let payload = CandlePayload {
        values: [
            decimal(1)?,
            decimal(2)?,
            decimal(3)?,
            decimal(4)?,
            decimal(5)?,
        ],
    };
    if payload.values[..4].iter().any(|v| *v <= 0)
        || payload.low() > payload.open().min(payload.close())
        || payload.high() < payload.open().max(payload.close())
        || payload.low() > payload.high()
    {
        return Err(row(1, RowIssue::Ohlc));
    }
    let quote = decimal(7)?;
    let trades = number(8)?;
    let taker_base = decimal(9)?;
    let taker_quote = decimal(10)?;
    decimal(11)?;
    if taker_base > payload.volume() || taker_quote > quote {
        return Err(row(9, RowIssue::Volume));
    }
    let available_us = match p.anchor {
        LagAnchor::ReportedClose => event_us,
        LagAnchor::MinuteEnd => end,
    }
    .checked_add(p.lag_us)
    .ok_or_else(|| row(6, RowIssue::Overflow))?;
    let mut canonical = b"BINANCE-SPOT-1M-MODELED-001\0".to_vec();
    p.encode(&mut canonical);
    canonical.extend_from_slice(&open_us.to_le_bytes());
    canonical.extend_from_slice(&event_us.to_le_bytes());
    canonical.extend_from_slice(&available_us.to_le_bytes());
    for n in payload.values {
        canonical.extend_from_slice(&n.to_le_bytes());
    }
    canonical.extend_from_slice(&quote.to_le_bytes());
    canonical.extend_from_slice(&trades.to_le_bytes());
    canonical.extend_from_slice(&taker_base.to_le_bytes());
    canonical.extend_from_slice(&taker_quote.to_le_bytes());
    let id = digest(&canonical);
    Ok(MarketEvidence {
        profile: p,
        open_us,
        event_us,
        available_us,
        payload,
        canonical,
        id,
    })
}
#[derive(Clone, Debug)]
pub struct Dataset {
    profile: Profile,
    evidence: Vec<MarketEvidence>,
    raw_hash: String,
    duplicates: usize,
    limits: Limits,
}
impl Dataset {
    pub fn from_csv(bytes: &[u8], profile: Profile, limits: Limits) -> Result<Self> {
        #[cfg(test)]
        work_counts::bump(|c| c.parses += 1);
        if bytes.len() > limits.max_input_bytes {
            return Err(Error::Limit("input bytes"));
        }
        let input = std::str::from_utf8(bytes).map_err(|_| Error::Utf8)?;
        let mut bars: BTreeMap<u64, MarketEvidence> = BTreeMap::new();
        let mut duplicates = 0;
        let mut count = 0;
        for (i, line) in input.lines().enumerate() {
            count += 1;
            if count > limits.max_rows {
                return Err(Error::Limit("rows"));
            }
            if line.len() > limits.max_line_bytes {
                return Err(Error::Limit("line bytes"));
            }
            let e = parse_row(line, i + 1, profile)?;
            if let Some(old) = bars.get(&e.open_us) {
                if old.canonical != e.canonical {
                    return Err(Error::ConflictingBar { open_us: e.open_us });
                }
                duplicates += 1;
            } else {
                bars.insert(e.open_us, e);
            }
        }
        if count == 0 {
            return Err(Error::EmptyInput);
        }
        let mut evidence: Vec<_> = bars.into_values().collect();
        evidence.sort_by(|a, b| {
            (a.available_us, a.event_us, a.open_us, &a.id).cmp(&(
                b.available_us,
                b.event_us,
                b.open_us,
                &b.id,
            ))
        });
        Ok(Self {
            profile,
            evidence,
            raw_hash: digest(bytes),
            duplicates,
            limits,
        })
    }
    pub fn evidence(&self) -> &[MarketEvidence] {
        &self.evidence
    }
    pub fn profile(&self) -> Profile {
        self.profile
    }
    pub fn raw_sha256(&self) -> &str {
        &self.raw_hash
    }
    pub fn duplicate_rows(&self) -> usize {
        self.duplicates
    }
    pub fn eligible(&self, decision_us: u64) -> Result<Vec<&MarketEvidence>> {
        let e: Vec<_> = self
            .evidence
            .iter()
            .filter(|e| e.available_us <= decision_us)
            .collect();
        if e.len() > self.limits.max_visible_bars {
            return Err(Error::Limit("visible bars"));
        }
        Ok(e)
    }
    pub fn decision(&self, decision_us: u64) -> Result<DecisionView> {
        core_view::build(self, decision_us)
    }
}
