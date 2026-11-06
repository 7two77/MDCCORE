//! Opt-in v2: independent bounded blobs, unchanged decoded decision semantics.
use super::*;
#[cfg(target_os = "linux")]
mod store;
#[cfg(target_os = "linux")]
pub use store::RunStoreV2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StorageLimits {
    pub max_decisions: usize,
    pub max_manifest_bytes: usize,
    pub max_encoded_record_bytes: usize,
    pub max_physical_journal_bytes: usize,
    pub max_report_bytes: usize,
    pub max_snapshot_bytes: usize,
    pub max_decoded_record_bytes: usize,
    pub max_total_decoded_blob_bytes: usize,
}
impl Default for StorageLimits {
    fn default() -> Self {
        Self {
            max_decisions: 256,
            max_manifest_bytes: 16384,
            max_encoded_record_bytes: 24 * 1024 * 1024,
            max_physical_journal_bytes: 64 * 1024 * 1024,
            max_report_bytes: 1024 * 1024,
            max_snapshot_bytes: 16 * 1024 * 1024,
            max_decoded_record_bytes: 24 * 1024 * 1024,
            max_total_decoded_blob_bytes: 256 * 1024 * 1024,
        }
    }
}
impl StorageLimits {
    fn values(self) -> [usize; 8] {
        [
            self.max_decisions,
            self.max_manifest_bytes,
            self.max_encoded_record_bytes,
            self.max_physical_journal_bytes,
            self.max_report_bytes,
            self.max_snapshot_bytes,
            self.max_decoded_record_bytes,
            self.max_total_decoded_blob_bytes,
        ]
    }
    fn validate(self) -> Result<()> {
        if self
            .values()
            .into_iter()
            .zip(Self::default().values())
            .any(|(n, max)| n == 0 || n > max)
        {
            Err(RecoveryError::Limit("v2 limits"))
        } else {
            Ok(())
        }
    }
    fn logical(self) -> RunLimits {
        RunLimits {
            max_decisions: self.max_decisions,
            max_manifest_bytes: self.max_manifest_bytes,
            max_record_bytes: self.max_decoded_record_bytes,
            max_journal_bytes: self.max_physical_journal_bytes,
            max_report_bytes: self.max_report_bytes,
            max_snapshot_bytes: self.max_snapshot_bytes,
        }
    }
}
#[derive(Clone, Copy)]
enum V2Kind {
    Manifest,
    Header,
    Decision,
    Head,
}
impl V2Kind {
    fn magic(self) -> &'static [u8; 16] {
        match self {
            Self::Manifest => b"MDC-RMAN-002\0\0\0\0",
            Self::Header => b"MDC-RJNL-002\0\0\0\0",
            Self::Decision => b"MDC-RDEC-002\0\0\0\0",
            Self::Head => b"MDC-RHED-002\0\0\0\0",
        }
    }
    fn domain(self) -> &'static [u8] {
        match self {
            Self::Manifest => b"MDC-REPLAY-RUN-002/manifest/v2\0",
            Self::Header => b"MDC-REPLAY-RUN-002/journal/v2\0",
            Self::Decision => b"MDC-REPLAY-RUN-002/decision/v2\0",
            Self::Head => b"MDC-REPLAY-RUN-002/head/v2\0",
        }
    }
}
fn frame2(kind: V2Kind, body: &[u8], cap: usize) -> Result<Vec<u8>> {
    if body.len() > cap {
        return Err(RecoveryError::Limit("v2 encoded record"));
    }
    let mut out = kind.magic().to_vec();
    out.extend_from_slice(&2u16.to_le_bytes());
    put(&mut out, body.len() as u64);
    out.extend_from_slice(body);
    out.extend_from_slice(&domain_hash(kind.domain(), &out));
    Ok(out)
}
fn parse2(kind: V2Kind, bytes: &[u8], cap: usize) -> Result<(&[u8], Hash)> {
    let mut r = Reader::new(bytes);
    if r.take(16)? != kind.magic() || r.u16()? != 2 {
        return Err(RecoveryError::Format("v2 magic/version"));
    }
    let n = r.size(cap)?;
    let body = r.take(n)?;
    let digest = r.hash()?;
    r.done()?;
    if domain_hash(kind.domain(), &bytes[..26 + n]) != digest {
        return Err(RecoveryError::Integrity("v2 frame checksum"));
    }
    Ok((body, digest))
}

/// Pure Rust miniz_oxide 0.9.1, level 6; zlib format, no dictionaries.
pub mod codec {
    use super::*;
    pub fn encode_blob(bytes: &[u8], cap: usize) -> Result<Vec<u8>> {
        if cap > StorageLimits::default().max_snapshot_bytes || bytes.len() > cap {
            return Err(RecoveryError::Limit("v2 decoded blob"));
        }
        let compressed = miniz_oxide::deflate::compress_to_vec_zlib(bytes, 6);
        let (tag, encoded) = if compressed.len() < bytes.len() {
            (1, compressed.as_slice())
        } else {
            (0, bytes)
        };
        let mut out = vec![tag];
        put(&mut out, bytes.len() as u64);
        put(&mut out, encoded.len() as u64);
        out.extend_from_slice(&hash(bytes));
        out.extend_from_slice(encoded);
        Ok(out)
    }
    pub fn decode_blob(bytes: &[u8], decoded_cap: usize, encoded_cap: usize) -> Result<Vec<u8>> {
        if decoded_cap > StorageLimits::default().max_snapshot_bytes
            || encoded_cap > StorageLimits::default().max_encoded_record_bytes
        {
            return Err(RecoveryError::Limit("v2 codec limits"));
        }
        let mut r = Reader::new(bytes);
        let blob = Blob::read(&mut r, decoded_cap, encoded_cap)?;
        r.done()?;
        blob.decode()
    }
    pub(super) struct Blob<'a> {
        pub(super) decoded: usize,
        encoded: &'a [u8],
        tag: u8,
        digest: Hash,
    }
    impl<'a> Blob<'a> {
        pub(super) fn read(r: &mut Reader<'a>, dcap: usize, ecap: usize) -> Result<Self> {
            let tag = r.u8()?;
            if tag > 1 {
                return Err(RecoveryError::Format("v2 blob tag"));
            }
            let decoded = r.size(dcap)?;
            let n = r.size(ecap)?;
            let digest = r.hash()?;
            let encoded = r.take(n)?;
            if tag == 0 && n != decoded {
                return Err(RecoveryError::Format("v2 raw length"));
            }
            Ok(Self {
                decoded,
                encoded,
                tag,
                digest,
            })
        }
        pub(super) fn decode(&self) -> Result<Vec<u8>> {
            let data = if self.tag == 0 {
                self.encoded.to_vec()
            } else {
                use miniz_oxide::{
                    inflate::stream::{inflate, InflateState},
                    DataFormat, MZFlush, MZStatus,
                };
                if self.encoded.len() < 2 || self.encoded[1] & 0x20 != 0 {
                    return Err(RecoveryError::Format("v2 dictionary/header"));
                }
                let mut data = vec![
                    0;
                    self.decoded
                        .checked_add(1)
                        .ok_or(RecoveryError::Limit("v2 decoded length"))?
                ];
                let mut state = InflateState::new_boxed(DataFormat::Zlib);
                let result = inflate(&mut state, self.encoded, &mut data, MZFlush::Finish);
                if result.status != Ok(MZStatus::StreamEnd)
                    || result.bytes_consumed != self.encoded.len()
                    || result.bytes_written != self.decoded
                {
                    return Err(RecoveryError::Integrity("v2 zlib stream/length"));
                }
                data.truncate(self.decoded);
                data
            };
            if hash(&data) != self.digest {
                return Err(RecoveryError::Integrity("v2 decoded hash"));
            }
            Ok(data)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunManifestV2 {
    logical: RunManifest,
    limits: StorageLimits,
}
impl RunManifestV2 {
    pub fn new(
        source: &[u8],
        profile: Profile,
        cutoffs: Vec<u64>,
        limits: StorageLimits,
    ) -> Result<Self> {
        limits.validate()?;
        let logical = RunManifest::new(source, profile, cutoffs, limits.logical())?;
        let m = Self { logical, limits };
        m.encode()?;
        Ok(m)
    }
    pub fn profile(&self) -> Profile {
        self.logical.profile()
    }
    pub fn cutoffs(&self) -> &[u64] {
        self.logical.cutoffs()
    }
    pub fn limits(&self) -> StorageLimits {
        self.limits
    }
    pub fn source_sha256(&self) -> Hash {
        self.logical.source_sha256()
    }
    pub fn run_id(&self) -> Hash {
        domain_hash(b"MDC-REPLAY-RUN-002/run-id/v2\0", &self.body())
    }
    fn body(&self) -> Vec<u8> {
        let mut b = vec![];
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.push(1);
        put(&mut b, self.logical.source_len as u64);
        b.extend_from_slice(&self.logical.source_hash);
        self.profile().encode(&mut b);
        for n in self.limits.values() {
            put(&mut b, n as u64);
        }
        b.extend_from_slice(&[1, 6]);
        put(&mut b, self.cutoffs().len() as u64);
        for n in self.cutoffs() {
            put(&mut b, *n);
        }
        b
    }
    pub fn encode(&self) -> Result<Vec<u8>> {
        self.limits.validate()?;
        self.logical.validate()?;
        let b = frame2(
            V2Kind::Manifest,
            &self.body(),
            self.limits.max_manifest_bytes,
        )?;
        if b.len() > self.limits.max_manifest_bytes {
            return Err(RecoveryError::Limit("v2 manifest"));
        }
        Ok(b)
    }
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > StorageLimits::default().max_manifest_bytes {
            return Err(RecoveryError::Limit("v2 manifest"));
        }
        let (body, _) = parse2(
            V2Kind::Manifest,
            bytes,
            StorageLimits::default().max_manifest_bytes,
        )?;
        let mut r = Reader::new(body);
        if r.u16()? != 1 || r.u16()? != 1 || r.u8()? != 1 {
            return Err(RecoveryError::Format("v2 engine/consumer/identity"));
        }
        let source_len = r.size(adapter_limits().max_input_bytes)?;
        let source_hash = r.hash()?;
        let symbol = match r.u8()? {
            0 => Symbol::BtcUsdt,
            1 => Symbol::EthUsdt,
            _ => return Err(RecoveryError::Format("v2 symbol")),
        };
        let unit = match r.u8()? {
            0 => TimestampUnit::Milliseconds,
            1 => TimestampUnit::Microseconds,
            _ => return Err(RecoveryError::Format("v2 unit")),
        };
        let anchor = match r.u8()? {
            0 => LagAnchor::ReportedClose,
            1 => LagAnchor::MinuteEnd,
            _ => return Err(RecoveryError::Format("v2 anchor")),
        };
        let lag = r.u64()?;
        if lag % 1000 != 0 {
            return Err(RecoveryError::Format("v2 lag"));
        }
        let profile = Profile::new(symbol, unit, lag / 1000, anchor)?;
        let mut values = [0; 8];
        for (n, cap) in values.iter_mut().zip(StorageLimits::default().values()) {
            *n = r.size(cap)?;
        }
        let limits = StorageLimits {
            max_decisions: values[0],
            max_manifest_bytes: values[1],
            max_encoded_record_bytes: values[2],
            max_physical_journal_bytes: values[3],
            max_report_bytes: values[4],
            max_snapshot_bytes: values[5],
            max_decoded_record_bytes: values[6],
            max_total_decoded_blob_bytes: values[7],
        };
        limits.validate()?;
        if r.u8()? != 1 || r.u8()? != 6 {
            return Err(RecoveryError::Format("v2 codec profile"));
        }
        let n = r.size(limits.max_decisions)?;
        let mut cutoffs = Vec::new();
        for _ in 0..n {
            cutoffs.push(r.u64()?);
        }
        r.done()?;
        let logical = RunManifest {
            profile,
            source_len,
            source_hash,
            cutoffs,
            limits: limits.logical(),
        };
        logical.validate()?;
        let m = Self { logical, limits };
        if m.encode()?.as_slice() != bytes {
            return Err(RecoveryError::Format("v2 canonical manifest"));
        }
        Ok(m)
    }
}
#[derive(Clone)]
struct HeadV2 {
    run: Hash,
    manifest: Hash,
    offset: u64,
    count: u64,
    tip: Hash,
    bundle: Hash,
}
impl HeadV2 {
    fn encode(&self) -> Result<Vec<u8>> {
        let mut b = self.run.to_vec();
        b.extend_from_slice(&self.manifest);
        put(&mut b, self.offset);
        put(&mut b, self.count);
        b.extend_from_slice(&self.tip);
        b.extend_from_slice(&self.bundle);
        frame2(V2Kind::Head, &b, 256)
    }
    fn decode(bytes: &[u8]) -> Result<Self> {
        let (body, _) = parse2(V2Kind::Head, bytes, 256)?;
        let mut r = Reader::new(body);
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
fn header2(run: Hash, manifest: Hash) -> Result<Vec<u8>> {
    let mut b = 1u16.to_le_bytes().to_vec();
    b.extend_from_slice(&run);
    b.extend_from_slice(&manifest);
    frame2(V2Kind::Header, &b, 128)
}
fn bundle_digest(run: Hash, b: &DecisionBundle) -> Hash {
    let mut v = run.to_vec();
    put(&mut v, b.index);
    put(&mut v, b.cutoff);
    v.extend_from_slice(&b.decision_id);
    v.extend_from_slice(&hash(&b.report));
    v.extend_from_slice(&hash(&b.snapshot));
    domain_hash(b"MDC-REPLAY-RUN-002/decoded-bundle/v2\0", &v)
}
fn decoded_total(b: &DecisionBundle, previous: usize, limits: StorageLimits) -> Result<usize> {
    let n = b
        .report
        .len()
        .checked_add(b.snapshot.len())
        .ok_or(RecoveryError::Limit("v2 decoded lengths"))?;
    check_decoded(n, previous, limits)
}
fn check_decoded(n: usize, previous: usize, l: StorageLimits) -> Result<usize> {
    if n.checked_add(192)
        .ok_or(RecoveryError::Limit("v2 decoded lengths"))?
        > l.max_decoded_record_bytes
    {
        return Err(RecoveryError::Limit("v2 decoded record"));
    }
    let total = previous
        .checked_add(n)
        .ok_or(RecoveryError::Limit("v2 aggregate"))?;
    if total > l.max_total_decoded_blob_bytes {
        return Err(RecoveryError::Limit("v2 decoded aggregate"));
    }
    Ok(total)
}
fn encode_record(
    b: &DecisionBundle,
    run: Hash,
    previous: Hash,
    l: StorageLimits,
) -> Result<Vec<u8>> {
    decoded_total(b, 0, l)?;
    let mut v = run.to_vec();
    put(&mut v, b.index);
    put(&mut v, b.cutoff);
    v.extend_from_slice(&b.decision_id);
    v.extend_from_slice(&previous);
    v.extend_from_slice(&codec::encode_blob(&b.report, l.max_report_bytes)?);
    v.extend_from_slice(&codec::encode_blob(&b.snapshot, l.max_snapshot_bytes)?);
    frame2(V2Kind::Decision, &v, l.max_encoded_record_bytes)
}
fn decode_record(
    bytes: &[u8],
    run: Hash,
    previous: Hash,
    l: StorageLimits,
    decoded_before: usize,
) -> Result<(DecisionBundle, Hash, usize)> {
    let (body, digest) = parse2(V2Kind::Decision, bytes, l.max_encoded_record_bytes)?;
    let mut r = Reader::new(body);
    if r.hash()? != run {
        return Err(RecoveryError::Integrity("v2 run"));
    }
    let index = r.u64()?;
    let cutoff = r.u64()?;
    let decision_id = r.hash()?;
    if r.hash()? != previous {
        return Err(RecoveryError::Integrity("v2 chain"));
    }
    let report = codec::Blob::read(&mut r, l.max_report_bytes, l.max_encoded_record_bytes)?;
    let snapshot = codec::Blob::read(&mut r, l.max_snapshot_bytes, l.max_encoded_record_bytes)?;
    r.done()?;
    let total = check_decoded(
        report
            .decoded
            .checked_add(snapshot.decoded)
            .ok_or(RecoveryError::Limit("v2 decoded lengths"))?,
        decoded_before,
        l,
    )?;
    let b = DecisionBundle {
        index,
        cutoff,
        decision_id,
        report: report.decode()?,
        snapshot: snapshot.decode()?,
    };
    Ok((b, digest, total))
}

#[cfg(test)]
mod tests;
