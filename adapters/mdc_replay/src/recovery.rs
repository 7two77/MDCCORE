//! Versioned bounded replay-run framing. Linux persistence lives in `RunStore`.
use crate::{Dataset, LagAnchor, Limits, Profile, Symbol, TimestampUnit};
use mdc_core::pipeline::{SnapshotLimits, SnapshotSession};
use sha2::{Digest, Sha256};

#[cfg(target_os = "linux")]
mod store;
#[cfg(target_os = "linux")]
pub use store::RunStore;

pub type Hash = [u8; 32];
pub type Result<T> = std::result::Result<T, RecoveryError>;
#[derive(Debug)]
pub enum RecoveryError {
    Format(&'static str),
    Integrity(&'static str),
    Limit(&'static str),
    Index,
    ExpectedManifest,
    LockBusy,
    WrongProcess,
    RecoveryRequired,
    Indeterminate(Box<RecoveryError>),
    Io(std::io::Error),
    Replay(crate::Error),
    Snapshot(mdc_core::pipeline::SnapshotError),
}
impl std::fmt::Display for RecoveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for RecoveryError {}
impl From<std::io::Error> for RecoveryError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<crate::Error> for RecoveryError {
    fn from(e: crate::Error) -> Self {
        Self::Replay(e)
    }
}
impl From<mdc_core::pipeline::SnapshotError> for RecoveryError {
    fn from(e: mdc_core::pipeline::SnapshotError) -> Self {
        Self::Snapshot(e)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RunLimits {
    pub max_decisions: usize,
    pub max_manifest_bytes: usize,
    pub max_record_bytes: usize,
    pub max_journal_bytes: usize,
    pub max_report_bytes: usize,
    pub max_snapshot_bytes: usize,
}
impl Default for RunLimits {
    fn default() -> Self {
        Self {
            max_decisions: 256,
            max_manifest_bytes: 16 * 1024,
            max_record_bytes: 24 * 1024 * 1024,
            max_journal_bytes: 64 * 1024 * 1024,
            max_report_bytes: 1024 * 1024,
            max_snapshot_bytes: 16 * 1024 * 1024,
        }
    }
}
impl RunLimits {
    fn values(self) -> [usize; 6] {
        [
            self.max_decisions,
            self.max_manifest_bytes,
            self.max_record_bytes,
            self.max_journal_bytes,
            self.max_report_bytes,
            self.max_snapshot_bytes,
        ]
    }
    fn validate(self) -> Result<()> {
        if self
            .values()
            .iter()
            .zip(Self::default().values())
            .any(|(n, cap)| *n == 0 || *n > cap)
        {
            return Err(RecoveryError::Limit("run limits"));
        }
        Ok(())
    }
    fn core(self) -> SnapshotLimits {
        SnapshotLimits {
            max_snapshot_bytes: self.max_snapshot_bytes,
            max_field_bytes: 2 * 1024 * 1024,
            max_collection_items: 16384,
            max_total_items: 65536,
            max_definitions: 4096,
            max_operations: 4096,
            max_manifest_bytes: 8 * 1024 * 1024,
        }
    }
}
fn adapter_limits() -> Limits {
    Limits {
        max_input_bytes: 4 * 1024 * 1024,
        max_rows: 4096,
        max_line_bytes: 4096,
        max_visible_bars: 256,
    }
}
fn hash(bytes: &[u8]) -> Hash {
    Sha256::digest(bytes).into()
}
fn domain_hash(domain: &[u8], bytes: &[u8]) -> Hash {
    let mut h = Sha256::new();
    h.update(domain);
    h.update(bytes);
    h.finalize().into()
}
fn put(v: &mut Vec<u8>, n: u64) {
    v.extend_from_slice(&n.to_le_bytes());
}
fn blob(v: &mut Vec<u8>, b: &[u8]) {
    put(v, b.len() as u64);
    v.extend_from_slice(b);
}
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self
            .at
            .checked_add(n)
            .ok_or(RecoveryError::Limit("offset"))?;
        let b = self
            .bytes
            .get(self.at..end)
            .ok_or(RecoveryError::Format("truncated"))?;
        self.at = end;
        Ok(b)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn size(&mut self, cap: usize) -> Result<usize> {
        let n = usize::try_from(self.u64()?).map_err(|_| RecoveryError::Limit("length"))?;
        if n > cap {
            return Err(RecoveryError::Limit("length"));
        }
        Ok(n)
    }
    fn hash(&mut self) -> Result<Hash> {
        Ok(self.take(32)?.try_into().unwrap())
    }
    fn blob(&mut self, cap: usize) -> Result<&'a [u8]> {
        let n = self.size(cap)?;
        self.take(n)
    }
    fn done(&self) -> Result<()> {
        if self.at != self.bytes.len() {
            Err(RecoveryError::Format("trailing bytes"))
        } else {
            Ok(())
        }
    }
}
#[derive(Clone, Copy)]
enum Kind {
    Manifest,
    Header,
    Decision,
    Head,
}
impl Kind {
    fn magic(self) -> &'static [u8; 16] {
        match self {
            Self::Manifest => b"MDC-RMAN-001\0\0\0\0",
            Self::Header => b"MDC-RJNL-001\0\0\0\0",
            Self::Decision => b"MDC-RDEC-001\0\0\0\0",
            Self::Head => b"MDC-RHED-001\0\0\0\0",
        }
    }
    fn domain(self) -> &'static [u8] {
        match self {
            Self::Manifest => b"MDC-REPLAY-RUN-001/manifest/v1\0",
            Self::Header => b"MDC-REPLAY-RUN-001/journal/v1\0",
            Self::Decision => b"MDC-REPLAY-RUN-001/decision/v1\0",
            Self::Head => b"MDC-REPLAY-RUN-001/head/v1\0",
        }
    }
}
fn frame(kind: Kind, body: &[u8], cap: usize) -> Result<Vec<u8>> {
    if body.len() > cap {
        return Err(RecoveryError::Limit("frame body"));
    }
    let mut out = kind.magic().to_vec();
    out.extend_from_slice(&1u16.to_le_bytes());
    put(&mut out, body.len() as u64);
    out.extend_from_slice(body);
    let h = domain_hash(kind.domain(), &out);
    out.extend_from_slice(&h);
    Ok(out)
}
fn parse<'a>(kind: Kind, r: &mut Reader<'a>, cap: usize) -> Result<(&'a [u8], Hash)> {
    let start = r.at;
    if r.take(16)? != kind.magic() || r.u16()? != 1 {
        return Err(RecoveryError::Format("magic or version"));
    }
    let n = r.size(cap)?;
    let body = r.take(n)?;
    let end = r.at;
    let digest = r.hash()?;
    if domain_hash(kind.domain(), &r.bytes[start..end]) != digest {
        return Err(RecoveryError::Integrity("frame checksum"));
    }
    Ok((body, digest))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunManifest {
    profile: Profile,
    source_len: usize,
    source_hash: Hash,
    cutoffs: Vec<u64>,
    limits: RunLimits,
}
impl RunManifest {
    pub fn new(
        source: &[u8],
        profile: Profile,
        cutoffs: Vec<u64>,
        limits: RunLimits,
    ) -> Result<Self> {
        let m = Self {
            profile,
            source_len: source.len(),
            source_hash: hash(source),
            cutoffs,
            limits,
        };
        m.validate()?;
        m.dataset(source)?;
        m.encode()?;
        Ok(m)
    }
    pub fn profile(&self) -> Profile {
        self.profile
    }
    pub fn cutoffs(&self) -> &[u64] {
        &self.cutoffs
    }
    pub fn limits(&self) -> RunLimits {
        self.limits
    }
    pub fn source_sha256(&self) -> Hash {
        self.source_hash
    }
    pub fn run_id(&self) -> Hash {
        domain_hash(b"MDC-REPLAY-RUN-001/run-id/v1\0", &self.body())
    }
    fn validate(&self) -> Result<()> {
        self.limits.validate()?;
        if self.source_len > adapter_limits().max_input_bytes {
            return Err(RecoveryError::Limit("source bytes"));
        }
        if self.cutoffs.is_empty()
            || self.cutoffs.len() > self.limits.max_decisions
            || self.cutoffs.windows(2).any(|v| v[0] >= v[1])
        {
            return Err(RecoveryError::Format("strictly increasing nonempty plan"));
        }
        Ok(())
    }
    fn dataset(&self, source: &[u8]) -> Result<Dataset> {
        if source.len() != self.source_len || hash(source) != self.source_hash {
            return Err(RecoveryError::Integrity("captured source"));
        }
        let dataset = Dataset::from_csv(source, self.profile, adapter_limits())?;
        let n = dataset.eligible(*self.cutoffs.last().unwrap())?.len();
        if n > 0
            && n.checked_mul(11)
                .and_then(|n| n.checked_add(1))
                .ok_or(RecoveryError::Limit("core calls"))?
                > 4096
        {
            return Err(RecoveryError::Limit("core calls"));
        }
        Ok(dataset)
    }
    fn body(&self) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.push(1);
        put(&mut b, self.source_len as u64);
        b.extend_from_slice(&self.source_hash);
        self.profile.encode(&mut b);
        for n in self.limits.values() {
            put(&mut b, n as u64);
        }
        put(&mut b, self.cutoffs.len() as u64);
        for cutoff in &self.cutoffs {
            put(&mut b, *cutoff);
        }
        b
    }
    pub fn encode(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let size = self
            .cutoffs
            .len()
            .checked_mul(8)
            .and_then(|n| n.checked_add(170))
            .ok_or(RecoveryError::Limit("manifest bytes"))?;
        if size > self.limits.max_manifest_bytes {
            return Err(RecoveryError::Limit("manifest bytes"));
        }
        let out = frame(Kind::Manifest, &self.body(), self.limits.max_manifest_bytes)?;
        if out.len() > self.limits.max_manifest_bytes {
            return Err(RecoveryError::Limit("manifest bytes"));
        }
        Ok(out)
    }
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        #[cfg(test)]
        crate::work_counts::bump(|c| c.manifests += 1);
        if bytes.len() > RunLimits::default().max_manifest_bytes {
            return Err(RecoveryError::Limit("manifest bytes"));
        }
        let mut r = Reader::new(bytes);
        let (b, _) = parse(
            Kind::Manifest,
            &mut r,
            RunLimits::default().max_manifest_bytes,
        )?;
        r.done()?;
        let mut r = Reader::new(b);
        if r.u16()? != 1 || r.u16()? != 1 || r.u8()? != 1 {
            return Err(RecoveryError::Format("engine/consumer/identity"));
        }
        let source_len = r.size(adapter_limits().max_input_bytes)?;
        let source_hash = r.hash()?;
        let symbol = match r.u8()? {
            0 => Symbol::BtcUsdt,
            1 => Symbol::EthUsdt,
            _ => return Err(RecoveryError::Format("symbol")),
        };
        let unit = match r.u8()? {
            0 => TimestampUnit::Milliseconds,
            1 => TimestampUnit::Microseconds,
            _ => return Err(RecoveryError::Format("unit")),
        };
        let anchor = match r.u8()? {
            0 => LagAnchor::ReportedClose,
            1 => LagAnchor::MinuteEnd,
            _ => return Err(RecoveryError::Format("anchor")),
        };
        let lag = r.u64()?;
        if lag % 1000 != 0 {
            return Err(RecoveryError::Format("integer millisecond lag"));
        }
        let profile = Profile::new(symbol, unit, lag / 1000, anchor)?;
        let caps = RunLimits::default().values();
        let mut v = [0; 6];
        for (n, cap) in v.iter_mut().zip(caps) {
            *n = r.size(cap)?;
        }
        let limits = RunLimits {
            max_decisions: v[0],
            max_manifest_bytes: v[1],
            max_record_bytes: v[2],
            max_journal_bytes: v[3],
            max_report_bytes: v[4],
            max_snapshot_bytes: v[5],
        };
        limits.validate()?;
        let n = r.size(limits.max_decisions)?;
        let mut cutoffs = Vec::new();
        for _ in 0..n {
            cutoffs.push(r.u64()?);
        }
        r.done()?;
        let m = Self {
            profile,
            source_len,
            source_hash,
            cutoffs,
            limits,
        };
        m.validate()?;
        if m.encode()?.as_slice() != bytes {
            return Err(RecoveryError::Format("noncanonical manifest"));
        }
        Ok(m)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecisionBundle {
    index: u64,
    cutoff: u64,
    decision_id: Hash,
    report: Vec<u8>,
    snapshot: Vec<u8>,
}
impl DecisionBundle {
    pub fn index(&self) -> u64 {
        self.index
    }
    pub fn cutoff(&self) -> u64 {
        self.cutoff
    }
    pub fn decision_id(&self) -> Hash {
        self.decision_id
    }
    pub fn report(&self) -> &[u8] {
        &self.report
    }
    pub fn snapshot(&self) -> &[u8] {
        &self.snapshot
    }
    pub fn snapshot_sha256(&self) -> Hash {
        hash(&self.snapshot)
    }
    fn build(dataset: &Dataset, m: &RunManifest, index: u64) -> Result<Self> {
        let pos = usize::try_from(index.checked_sub(1).ok_or(RecoveryError::Index)?)
            .map_err(|_| RecoveryError::Index)?;
        let cutoff = *m.cutoffs.get(pos).ok_or(RecoveryError::Index)?;
        let (decision_id_text, report, snapshot) =
            crate::core_view::capture(dataset, cutoff)?.into_parts();
        let report = (report + "\n").into_bytes();
        if report.len() > m.limits.max_report_bytes || snapshot.len() > m.limits.max_snapshot_bytes
        {
            return Err(RecoveryError::Limit("decision blobs"));
        }
        #[cfg(test)]
        crate::work_counts::bump(|c| c.restores += 1);
        SnapshotSession::restore(&snapshot, m.limits.core())?;
        let mut decision_id = [0; 32];
        for (i, b) in decision_id.iter_mut().enumerate() {
            *b = u8::from_str_radix(&decision_id_text[i * 2..i * 2 + 2], 16)
                .map_err(|_| RecoveryError::Format("decision id"))?;
        }
        Ok(Self {
            index,
            cutoff,
            decision_id,
            report,
            snapshot,
        })
    }
    fn encode(&self, run: Hash, previous: Hash, l: RunLimits) -> Result<Vec<u8>> {
        if self.report.len() > l.max_report_bytes || self.snapshot.len() > l.max_snapshot_bytes {
            return Err(RecoveryError::Limit("decision blobs"));
        }
        let size = 192usize
            .checked_add(self.report.len())
            .and_then(|n| n.checked_add(self.snapshot.len()))
            .ok_or(RecoveryError::Limit("record bytes"))?;
        if size > l.max_record_bytes {
            return Err(RecoveryError::Limit("record bytes"));
        }
        let mut b = run.to_vec();
        put(&mut b, self.index);
        put(&mut b, self.cutoff);
        b.extend_from_slice(&self.decision_id);
        b.extend_from_slice(&hash(&self.report));
        b.extend_from_slice(&hash(&self.snapshot));
        b.extend_from_slice(&previous);
        blob(&mut b, &self.report);
        blob(&mut b, &self.snapshot);
        frame(Kind::Decision, &b, l.max_record_bytes)
    }
    fn decode(body: &[u8], run: Hash, previous: Hash, l: RunLimits) -> Result<Self> {
        let mut r = Reader::new(body);
        if r.hash()? != run {
            return Err(RecoveryError::Integrity("record run"));
        }
        let index = r.u64()?;
        let cutoff = r.u64()?;
        let decision_id = r.hash()?;
        let report_hash = r.hash()?;
        let snapshot_hash = r.hash()?;
        if r.hash()? != previous {
            return Err(RecoveryError::Integrity("record chain"));
        }
        let report = r.blob(l.max_report_bytes)?.to_vec();
        let snapshot = r.blob(l.max_snapshot_bytes)?.to_vec();
        r.done()?;
        if hash(&report) != report_hash || hash(&snapshot) != snapshot_hash {
            return Err(RecoveryError::Integrity("bundle hashes"));
        }
        Ok(Self {
            index,
            cutoff,
            decision_id,
            report,
            snapshot,
        })
    }
    fn digest(&self, run: Hash) -> Hash {
        let mut b = run.to_vec();
        put(&mut b, self.index);
        put(&mut b, self.cutoff);
        b.extend_from_slice(&self.decision_id);
        b.extend_from_slice(&hash(&self.report));
        b.extend_from_slice(&hash(&self.snapshot));
        domain_hash(b"MDC-REPLAY-RUN-001/bundle/v1\0", &b)
    }
}

fn header(run: Hash, manifest: Hash) -> Result<Vec<u8>> {
    let mut b = 1u16.to_le_bytes().to_vec();
    b.extend_from_slice(&run);
    b.extend_from_slice(&manifest);
    frame(Kind::Header, &b, 128)
}
#[derive(Clone)]
struct Head {
    run: Hash,
    manifest: Hash,
    offset: u64,
    count: u64,
    tip: Hash,
    bundle: Hash,
}
impl Head {
    fn encode(&self) -> Result<Vec<u8>> {
        let mut b = self.run.to_vec();
        b.extend_from_slice(&self.manifest);
        put(&mut b, self.offset);
        put(&mut b, self.count);
        b.extend_from_slice(&self.tip);
        b.extend_from_slice(&self.bundle);
        frame(Kind::Head, &b, 256)
    }
    fn decode(bytes: &[u8]) -> Result<Self> {
        let mut r = Reader::new(bytes);
        let (b, _) = parse(Kind::Head, &mut r, 256)?;
        r.done()?;
        let mut r = Reader::new(b);
        let h = Self {
            run: r.hash()?,
            manifest: r.hash()?,
            offset: r.u64()?,
            count: r.u64()?,
            tip: r.hash()?,
            bundle: r.hash()?,
        };
        r.done()?;
        Ok(h)
    }
}

/// Portable bounded validation of a complete HEAD-certified journal prefix.
/// Tail bytes are not arguments to this function; filesystem recovery selects them.
pub fn validate_prefix(
    manifest: &[u8],
    source: &[u8],
    head: &[u8],
    prefix: &[u8],
) -> Result<Vec<DecisionBundle>> {
    PreparedManifest::decode(manifest)?
        .bind_source(source)?
        .validate_prefix(head, prefix)
}

// Preparation is private and inseparable from the exact captured manifest bytes.
// Public raw validation and filesystem open share the same complete validator.
struct PreparedManifest {
    manifest: RunManifest,
    manifest_hash: Hash,
}
impl PreparedManifest {
    fn decode(bytes: &[u8]) -> Result<Self> {
        Ok(Self {
            manifest: RunManifest::decode(bytes)?,
            manifest_hash: hash(bytes),
        })
    }
    fn bind_source(self, source: &[u8]) -> Result<PreparedRun> {
        let dataset = self.manifest.dataset(source)?;
        Ok(PreparedRun {
            manifest: self.manifest,
            manifest_hash: self.manifest_hash,
            dataset,
        })
    }
}
struct PreparedRun {
    manifest: RunManifest,
    manifest_hash: Hash,
    dataset: Dataset,
}
impl PreparedRun {
    fn validate_prefix(&self, head: &[u8], prefix: &[u8]) -> Result<Vec<DecisionBundle>> {
        let m = &self.manifest;
        let dataset = &self.dataset;
        let h = Head::decode(head)?;
        let run = m.run_id();
        let mh = self.manifest_hash;
        if h.run != run || h.manifest != mh || h.offset != prefix.len() as u64 {
            return Err(RecoveryError::Integrity("HEAD binding"));
        }
        if prefix.len() > m.limits.max_journal_bytes || h.count > m.cutoffs.len() as u64 {
            return Err(RecoveryError::Limit("certified prefix"));
        }
        let expected = header(run, mh)?;
        if !prefix.starts_with(&expected) {
            return Err(RecoveryError::Integrity("journal header"));
        }
        let mut tip: Hash = expected[expected.len() - 32..].try_into().unwrap();
        let mut r = Reader {
            bytes: prefix,
            at: expected.len(),
        };
        let mut result = Vec::new();
        let mut last = [0; 32];
        for index in 1..=h.count {
            let (b, digest) = parse(Kind::Decision, &mut r, m.limits.max_record_bytes)?;
            let saved = DecisionBundle::decode(b, run, tip, m.limits)?;
            if saved.index != index || saved != DecisionBundle::build(dataset, m, index)? {
                return Err(RecoveryError::Integrity("decision recomputation"));
            }
            last = saved.digest(run);
            tip = digest;
            result.push(saved);
        }
        r.done()?;
        if tip != h.tip || last != h.bundle {
            return Err(RecoveryError::Integrity("HEAD tip"));
        }
        Ok(result)
    }
}
