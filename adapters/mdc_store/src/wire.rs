use crate::{Error, Limits, Result};
use sha2::{Digest, Sha256};

pub(crate) type Hash = [u8; 32];
pub(crate) fn hash(bytes: &[u8]) -> Hash {
    Sha256::digest(bytes).into()
}
#[derive(Clone, Copy)]
pub(crate) enum Kind {
    Snapshot,
    Header,
    Record,
    Head,
}
impl Kind {
    fn magic(self) -> [u8; 16] {
        match self {
            Self::Snapshot => *b"MDC-SNP-001\0\0\0\0\0",
            Self::Header => *b"MDC-WAL-001\0\0\0\0\0",
            Self::Record => *b"MDC-REC-001\0\0\0\0\0",
            Self::Head => *b"MDC-HEAD-001\0\0\0\0",
        }
    }
    fn domain(self) -> &'static [u8] {
        match self {
            Self::Snapshot => b"MDC-RECOVERY-001/snapshot/v1\0",
            Self::Header => b"MDC-RECOVERY-001/wal-header/v1\0",
            Self::Record => b"MDC-RECOVERY-001/wal-record/v1\0",
            Self::Head => b"MDC-RECOVERY-001/head/v1\0",
        }
    }
    fn cap(self, limits: Limits) -> usize {
        if matches!(self, Self::Head) {
            limits.max_frame_body.min(4096)
        } else {
            limits.max_frame_body
        }
    }
}
pub(crate) fn frame(kind: Kind, body: Vec<u8>, limits: Limits) -> Result<Vec<u8>> {
    if body.len() > kind.cap(limits) {
        return Err(Error::Limit("frame body"));
    }
    let mut v = Vec::new();
    v.extend_from_slice(&kind.magic());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&(body.len() as u64).to_le_bytes());
    v.extend_from_slice(&body);
    let mut h = Sha256::new();
    h.update(kind.domain());
    h.update(&v);
    v.extend_from_slice(&h.finalize());
    Ok(v)
}
pub(crate) fn parse<'a>(
    data: &'a [u8],
    offset: &mut usize,
    kind: Kind,
    limits: Limits,
) -> Result<(&'a [u8], Hash)> {
    let start = *offset;
    let prefix = data
        .get(start..start.checked_add(26).ok_or(Error::Limit("offset"))?)
        .ok_or(Error::Format("truncated frame header"))?;
    if prefix[..16] != kind.magic() {
        return Err(Error::Format("frame magic"));
    }
    if prefix[16..18] != 1u16.to_le_bytes() {
        return Err(Error::Format("frame version"));
    }
    let len = usize::try_from(u64::from_le_bytes(prefix[18..26].try_into().unwrap()))
        .map_err(|_| Error::Limit("length"))?;
    if len > kind.cap(limits) {
        return Err(Error::Limit("frame body"));
    }
    let end = start
        .checked_add(58)
        .and_then(|x| x.checked_add(len))
        .ok_or(Error::Limit("frame end"))?;
    let whole = data
        .get(start..end)
        .ok_or(Error::Format("truncated frame"))?;
    let mut h = Sha256::new();
    h.update(kind.domain());
    h.update(&whole[..whole.len() - 32]);
    let digest: Hash = h.finalize().into();
    if whole[whole.len() - 32..] != digest {
        return Err(Error::Integrity("frame checksum"));
    }
    *offset = end;
    Ok((&whole[26..26 + len], digest))
}
pub(crate) struct W {
    pub bytes: Vec<u8>,
    cap: usize,
}
impl W {
    pub fn new(limits: Limits) -> Self {
        Self {
            bytes: Vec::new(),
            cap: limits.max_frame_body,
        }
    }
    pub fn raw(&mut self, b: &[u8]) -> Result<()> {
        if self
            .bytes
            .len()
            .checked_add(b.len())
            .filter(|n| *n <= self.cap)
            .is_none()
        {
            return Err(Error::Limit("body"));
        }
        self.bytes.extend_from_slice(b);
        Ok(())
    }
    pub fn u16(&mut self, n: u16) -> Result<()> {
        self.raw(&n.to_le_bytes())
    }
    pub fn u64(&mut self, n: u64) -> Result<()> {
        self.raw(&n.to_le_bytes())
    }
    pub fn blob(&mut self, b: &[u8], cap: usize) -> Result<()> {
        if b.len() > cap {
            return Err(Error::Limit("field"));
        }
        self.u64(b.len() as u64)?;
        self.raw(b)
    }
}
pub(crate) struct R<'a> {
    bytes: &'a [u8],
    pos: usize,
}
impl<'a> R<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }
    pub fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(n).ok_or(Error::Limit("field end"))?;
        let b = self
            .bytes
            .get(self.pos..end)
            .ok_or(Error::Format("truncated field"))?;
        self.pos = end;
        Ok(b)
    }
    pub fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    pub fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    pub fn hash(&mut self) -> Result<Hash> {
        Ok(self.take(32)?.try_into().unwrap())
    }
    pub fn engine(&mut self) -> Result<()> {
        if self.u16()? != 1 {
            return Err(Error::Format("engine version"));
        }
        Ok(())
    }
    pub fn blob(&mut self, cap: usize) -> Result<Vec<u8>> {
        let n = usize::try_from(self.u64()?).map_err(|_| Error::Limit("field length"))?;
        if n > cap {
            return Err(Error::Limit("field"));
        }
        Ok(self.take(n)?.to_vec())
    }
    pub fn id(&mut self, limits: Limits) -> Result<String> {
        let v = self.blob(limits.max_request_id)?;
        let s = String::from_utf8(v).map_err(|_| Error::Format("request ID UTF-8"))?;
        crate::check_id(&s, limits)?;
        Ok(s)
    }
    pub fn done(self) -> Result<()> {
        if self.pos != self.bytes.len() {
            return Err(Error::Format("trailing fields"));
        }
        Ok(())
    }
}
#[derive(Clone, Debug)]
pub(crate) struct Call {
    pub seq: u64,
    pub id: String,
    pub request: Vec<u8>,
    pub outcome: Vec<u8>,
}
impl Call {
    pub fn write(&self, w: &mut W, l: Limits) -> Result<()> {
        w.u64(self.seq)?;
        w.blob(self.id.as_bytes(), l.max_request_id)?;
        w.blob(&self.request, l.max_field)?;
        w.blob(&self.outcome, l.max_field)
    }
    pub fn read(r: &mut R<'_>, l: Limits) -> Result<Self> {
        Ok(Self {
            seq: r.u64()?,
            id: r.id(l)?,
            request: r.blob(l.max_field)?,
            outcome: r.blob(l.max_field)?,
        })
    }
}
#[derive(Clone, Debug)]
pub(crate) struct Head {
    pub session: Hash,
    pub generation: u64,
    pub snapshot: u64,
    pub snapshot_seq: u64,
    pub snapshot_hash: Hash,
    pub wal: u64,
    pub base: u64,
    pub offset: u64,
    pub seq: u64,
    pub tip: Hash,
    pub state: Hash,
}
impl Head {
    pub fn encode(&self, l: Limits) -> Result<Vec<u8>> {
        let mut w = W::new(l);
        w.u16(1)?;
        w.raw(&self.session)?;
        for n in [self.generation, self.snapshot, self.snapshot_seq] {
            w.u64(n)?;
        }
        w.raw(&self.snapshot_hash)?;
        for n in [self.wal, self.base, self.offset, self.seq] {
            w.u64(n)?;
        }
        w.raw(&self.tip)?;
        w.raw(&self.state)?;
        frame(Kind::Head, w.bytes, l)
    }
    pub fn decode(b: &[u8], l: Limits) -> Result<Self> {
        let mut p = 0;
        let (body, _) = parse(b, &mut p, Kind::Head, l)?;
        if p != b.len() {
            return Err(Error::Format("HEAD trailing bytes"));
        }
        let mut r = R::new(body);
        r.engine()?;
        let h = Self {
            session: r.hash()?,
            generation: r.u64()?,
            snapshot: r.u64()?,
            snapshot_seq: r.u64()?,
            snapshot_hash: r.hash()?,
            wal: r.u64()?,
            base: r.u64()?,
            offset: r.u64()?,
            seq: r.u64()?,
            tip: r.hash()?,
            state: r.hash()?,
        };
        r.done()?;
        if h.generation == 0
            || h.snapshot == 0
            || h.snapshot > h.generation
            || h.wal > h.generation
            || h.wal == 0
            || h.base != h.snapshot_seq
            || h.seq < h.base
            || h.seq > l.max_operations as u64
            || h.offset > l.max_wal_bytes as u64
        {
            return Err(Error::Format("HEAD invariants"));
        }
        Ok(h)
    }
}
