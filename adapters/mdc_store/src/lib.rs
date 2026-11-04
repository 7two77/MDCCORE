//! Bounded Linux single-writer storage for MDC-WAL-001/1.
//! A durable receipt is conditional on the filesystem/device honoring sync.
#![cfg_attr(not(target_os = "linux"), allow(unused))]
#[cfg(not(target_os = "linux"))]
compile_error!("mdc_store v1 supports Linux local filesystems only");
mod scheduler;
mod wire;
use mdc_core::{
    contract::registry::Registry,
    identity::IdentityProtocol,
    pipeline::{SnapshotError, SnapshotLimits, SnapshotOutcome, SnapshotRequest, SnapshotSession},
};
pub use scheduler::{BatchPolicy, ScheduledSnapshot, ScheduledStore, SchedulerStatus, StoreClient};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    os::{
        fd::AsRawFd,
        unix::fs::{DirBuilderExt, OpenOptionsExt},
    },
    path::{Path, PathBuf},
};
use wire::{frame, hash, parse, Call, Hash, Head, Kind, R, W};

#[derive(Debug)]
pub enum Error {
    InvalidBatchPolicy(&'static str),
    SchedulerStopped,
    WorkerPanicked,
    MissingStore,
    Io(std::io::Error),
    Indeterminate(Box<Error>),
    RecoveryRequired,
    LockBusy,
    Format(&'static str),
    Integrity(&'static str),
    Limit(&'static str),
    Core(SnapshotError),
    RequestIdConflict,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<SnapshotError> for Error {
    fn from(e: SnapshotError) -> Self {
        Self::Core(e)
    }
}
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub core: SnapshotLimits,
    pub max_frame_body: usize,
    pub max_field: usize,
    pub max_request_id: usize,
    pub max_operations: usize,
    pub max_wal_bytes: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            core: SnapshotLimits::default(),
            max_frame_body: 64 * 1024 * 1024,
            max_field: 2 * 1024 * 1024,
            max_request_id: 256,
            max_operations: 4096,
            max_wal_bytes: 64 * 1024 * 1024,
        }
    }
}
impl Limits {
    fn validate(self) -> Result<()> {
        if self.max_request_id == 0
            || self.max_request_id > 256
            || self.max_operations > self.core.max_operations
            || self.max_frame_body > usize::MAX - 58
            || self.max_wal_bytes > usize::MAX - 1
        {
            return Err(Error::Limit("invalid limits"));
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Buffered,
    Durable,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Durability {
    AcceptedVolatile,
    DurableThrough(u64),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Receipt {
    pub seq: u64,
    pub outcome: SnapshotOutcome,
    pub durability: Durability,
}

fn check_id(id: &str, l: Limits) -> Result<()> {
    if id.is_empty() || id.len() > l.max_request_id || id.as_bytes().contains(&0) {
        return Err(Error::Format("request ID"));
    }
    Ok(())
}
fn next(n: u64) -> Result<u64> {
    n.checked_add(1)
        .ok_or(Error::Limit("sequence/generation overflow"))
}
fn path(dir: &Path, tag: char, generation: u64) -> PathBuf {
    dir.join(format!(
        "{tag}-{generation:016x}.{}",
        if tag == 'S' { "mdcs" } else { "mdcw" }
    ))
}
fn sync_dir(dir: &Path) -> Result<()> {
    File::open(dir)?.sync_all()?;
    Ok(())
}
fn open_new(p: &Path) -> Result<File> {
    Ok(OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(p)?)
}
fn read_file(p: &Path, cap: usize) -> Result<Vec<u8>> {
    let f = File::open(p)?;
    if !f.metadata()?.is_file() {
        return Err(Error::Format("regular file required"));
    }
    let mut b = Vec::new();
    f.take(cap.checked_add(1).ok_or(Error::Limit("file cap"))? as u64)
        .read_to_end(&mut b)?;
    if b.len() > cap {
        return Err(Error::Limit("file bytes"));
    }
    Ok(b)
}
fn lock(dir: &Path, create: bool) -> Result<File> {
    let f = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(create)
        .mode(0o600)
        .open(dir.join("LOCK"))?;
    unsafe extern "C" {
        fn flock(fd: std::ffi::c_int, operation: std::ffi::c_int) -> std::ffi::c_int;
    }
    // Linux LOCK_EX | LOCK_NB. File lives as long as Store; never unlink LOCK.
    if unsafe { flock(f.as_raw_fd(), 2 | 4) } != 0 {
        let e = std::io::Error::last_os_error();
        return Err(if e.kind() == std::io::ErrorKind::WouldBlock {
            Error::LockBusy
        } else {
            Error::Io(e)
        });
    }
    Ok(f)
}
fn write_new(p: &Path, b: &[u8]) -> Result<File> {
    let mut f = open_new(p)?;
    let mid = b.len() / 2;
    f.write_all(&b[..mid])?;
    fault("generation-half")?;
    f.write_all(&b[mid..])?;
    fault("generation-written")?;
    f.sync_all()?;
    fault("generation-synced")?;
    Ok(f)
}
fn publish(dir: &Path, h: &Head, l: Limits) -> Result<()> {
    let bytes = h.encode(l)?;
    let tmp = dir.join("HEAD.pending");
    let mut f = open_new(&tmp)?;
    let mid = bytes.len() / 2;
    f.write_all(&bytes[..mid])?;
    fault("head-half")?;
    f.write_all(&bytes[mid..])?;
    fault("head-written")?;
    f.sync_all()?;
    fault("head-synced")?;
    fs::rename(tmp, dir.join("HEAD"))?;
    fault("head-renamed")?;
    sync_dir(dir)?;
    fault("head-dir-synced")?;
    Ok(())
}
fn cleanup(dir: &Path, h: &Head) -> Result<()> {
    sync_dir(dir)?;
    let keep_s = path(dir, 'S', h.snapshot);
    let keep_w = path(dir, 'W', h.wal);
    for e in fs::read_dir(dir)? {
        let e = e?;
        let p = e.path();
        let name = e.file_name();
        let Some(s) = name.to_str() else { continue };
        let generation = s.len() == 23
            && (s.starts_with("S-") && s.ends_with(".mdcs")
                || s.starts_with("W-") && s.ends_with(".mdcw"))
            && s.as_bytes()[2..18]
                .iter()
                .all(|b| b.is_ascii_digit() || (*b >= b'a' && *b <= b'f'));
        if (generation && p != keep_s && p != keep_w) || s == "HEAD.pending" {
            fs::remove_file(p)?;
            fault("cleanup-unlinked")?;
        }
    }
    sync_dir(dir)
}
fn state(s: &SnapshotSession, l: Limits) -> Result<Hash> {
    Ok(hash(&s.state_manifest(l.core)?))
}
fn snapshot(
    s: &SnapshotSession,
    calls: &[Call],
    session: Hash,
    generation: u64,
    l: Limits,
) -> Result<Vec<u8>> {
    let bytes = s.export(l.core)?;
    let mut w = W::new(l);
    w.u16(1)?;
    w.raw(&session)?;
    w.u64(generation)?;
    w.u64(s.operation_count() as u64)?;
    w.blob(&bytes, l.core.max_snapshot_bytes)?;
    w.u64(calls.len() as u64)?;
    for call in calls {
        call.write(&mut w, l)?;
    }
    frame(Kind::Snapshot, w.bytes, l)
}
fn wal_header(h: &Head, l: Limits) -> Result<Vec<u8>> {
    let mut w = W::new(l);
    w.u16(1)?;
    w.raw(&h.session)?;
    w.u64(h.wal)?;
    w.u64(h.snapshot)?;
    w.u64(h.base)?;
    w.raw(&h.snapshot_hash)?;
    w.raw(&h.state)?;
    frame(Kind::Header, w.bytes, l)
}
fn wal_record(call: &Call, h: &Head, tip: Hash, state: Hash, l: Limits) -> Result<Vec<u8>> {
    let mut w = W::new(l);
    w.raw(&h.session)?;
    w.u64(h.wal)?;
    call.write(&mut w, l)?;
    w.raw(&state)?;
    w.raw(&tip)?;
    frame(Kind::Record, w.bytes, l)
}

/// Holds the exclusive OS lock and exposes only a qualified shared session view.
/// No background flush or external decision authorization is implied.
pub struct Store {
    dir: PathBuf,
    _lock: File,
    wal: File,
    head: Head,
    offset: u64,
    tip: Hash,
    session: SnapshotSession,
    calls: Vec<Call>,
    limits: Limits,
    poisoned: bool,
}
impl Store {
    pub fn create(
        dir: impl AsRef<Path>,
        registry: Registry,
        protocol: IdentityProtocol,
        limits: Limits,
    ) -> Result<Self> {
        limits.validate()?;
        let dir = dir.as_ref();
        // Budget/codec validation precedes filesystem creation.
        let session = SnapshotSession::new(registry, protocol);
        let mut id = [0; 32];
        File::open("/dev/urandom")?.read_exact(&mut id)?;
        let snap = snapshot(&session, &[], id, 1, limits)?;
        let base_state = state(&session, limits)?;
        fs::DirBuilder::new().mode(0o700).create(dir)?;
        let dir = dir.canonicalize()?;
        let guard = lock(&dir, true)?;
        let result = (|| {
            sync_dir(dir.parent().ok_or(Error::Format("store parent"))?)?;
            write_new(&path(&dir, 'S', 1), &snap)?;
            let mut head = Head {
                session: id,
                generation: 1,
                snapshot: 1,
                snapshot_seq: 0,
                snapshot_hash: hash(&snap),
                wal: 1,
                base: 0,
                offset: 0,
                seq: 0,
                tip: [0; 32],
                state: base_state,
            };
            let header = wal_header(&head, limits)?;
            if header.len() > limits.max_wal_bytes {
                return Err(Error::Limit("WAL bytes"));
            }
            let wal = write_new(&path(&dir, 'W', 1), &header)?;
            sync_dir(&dir)?;
            head.offset = header.len() as u64;
            head.tip = header[header.len() - 32..].try_into().unwrap();
            publish(&dir, &head, limits)?;
            Ok((head, wal))
        })();
        let (head, wal) = result.map_err(|e| Error::Indeterminate(Box::new(e)))?;
        Ok(Self {
            dir,
            _lock: guard,
            wal,
            offset: head.offset,
            tip: head.tip,
            head,
            session,
            calls: Vec::new(),
            limits,
            poisoned: false,
        })
    }
    pub fn open(dir: impl AsRef<Path>, limits: Limits) -> Result<Self> {
        limits.validate()?;
        let dir = dir.as_ref().canonicalize().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Error::MissingStore
            } else {
                Error::Io(e)
            }
        })?;
        if !dir.join("LOCK").is_file() || !dir.join("HEAD").is_file() {
            return Err(Error::Format("incomplete existing store"));
        }
        let guard = lock(&dir, false)?;
        let head = Head::decode(
            &read_file(&dir.join("HEAD"), limits.max_frame_body.min(4096) + 58)?,
            limits,
        )?;
        let bytes = read_file(&path(&dir, 'S', head.snapshot), limits.max_frame_body + 58)?;
        if hash(&bytes) != head.snapshot_hash {
            return Err(Error::Integrity("snapshot file"));
        }
        let mut p = 0;
        let (body, _) = parse(&bytes, &mut p, Kind::Snapshot, limits)?;
        if p != bytes.len() {
            return Err(Error::Format("snapshot trailing bytes"));
        }
        let mut r = R::new(body);
        r.engine()?;
        if r.hash()? != head.session || r.u64()? != head.snapshot || r.u64()? != head.snapshot_seq {
            return Err(Error::Integrity("snapshot binding"));
        }
        let core = r.blob(limits.core.max_snapshot_bytes)?;
        let mut session = SnapshotSession::restore(&core, limits.core)?;
        let count = r.u64()?;
        if count != head.snapshot_seq
            || count > limits.max_operations as u64
            || session.operation_count() as u64 != count
        {
            return Err(Error::Format("snapshot receipt count"));
        }
        let mut calls: Vec<Call> = Vec::new();
        let mut ids = std::collections::BTreeSet::new();
        for i in 0..count {
            let call = Call::read(&mut r, limits)?;
            if call.seq != i + 1 || !ids.insert(call.id.clone()) {
                return Err(Error::Format("snapshot receipt order/ID"));
            }
            let expected = session.recorded_call(i as usize, limits.core)?;
            if expected != (call.request.clone(), call.outcome.clone()) {
                return Err(Error::Integrity("snapshot receipt vs journal"));
            }
            calls.push(call);
        }
        r.done()?;
        let mut wal = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path(&dir, 'W', head.wal))?;
        if !wal.metadata()?.is_file() || wal.metadata()?.len() < head.offset {
            return Err(Error::Integrity("WAL committed size"));
        }
        let size = usize::try_from(head.offset).map_err(|_| Error::Limit("WAL offset"))?;
        let mut data = Vec::new();
        std::io::Read::by_ref(&mut wal)
            .take(head.offset)
            .read_to_end(&mut data)?;
        if data.len() != size {
            return Err(Error::Integrity("WAL committed short read"));
        }
        let mut pos = 0;
        let (body, mut tip) = parse(&data, &mut pos, Kind::Header, limits)?;
        let mut r = R::new(body);
        r.engine()?;
        if r.hash()? != head.session
            || r.u64()? != head.wal
            || r.u64()? != head.snapshot
            || r.u64()? != head.base
            || r.hash()? != head.snapshot_hash
            || r.hash()? != state(&session, limits)?
        {
            return Err(Error::Integrity("WAL header binding"));
        }
        r.done()?;
        let mut seq = head.base;
        while pos < data.len() {
            let (body, digest) = parse(&data, &mut pos, Kind::Record, limits)?;
            let mut r = R::new(body);
            if r.hash()? != head.session || r.u64()? != head.wal {
                return Err(Error::Integrity("WAL record binding"));
            }
            let call = Call::read(&mut r, limits)?;
            let expected_state = r.hash()?;
            let previous = r.hash()?;
            r.done()?;
            if call.seq != next(seq)?
                || call.seq > head.seq
                || previous != tip
                || !ids.insert(call.id.clone())
            {
                return Err(Error::Integrity("WAL chain/sequence/ID"));
            }
            let (candidate, _, _) =
                session.prepare_call(&call.request, Some(&call.outcome), limits.core)?;
            if state(&candidate, limits)? != expected_state {
                return Err(Error::Integrity("WAL resulting state"));
            }
            session = candidate;
            seq = call.seq;
            calls.push(call);
            tip = digest;
        }
        if seq != head.seq || tip != head.tip || state(&session, limits)? != head.state {
            return Err(Error::Integrity("HEAD vs replay"));
        }
        // A selected HEAD may have been renamed by a process which died before
        // directory sync. Stabilize selection BEFORE truncation/cleanup.
        sync_dir(&dir)?;
        wal.set_len(head.offset)?;
        wal.sync_all()?;
        wal.seek(SeekFrom::Start(head.offset))?;
        cleanup(&dir, &head)?;
        Ok(Self {
            dir,
            _lock: guard,
            wal,
            offset: head.offset,
            tip: head.tip,
            head,
            session,
            calls,
            limits,
            poisoned: false,
        })
    }
    fn ready(&self) -> Result<()> {
        if self.poisoned {
            Err(Error::RecoveryRequired)
        } else {
            Ok(())
        }
    }
    pub fn session(&self) -> Result<&SnapshotSession> {
        self.ready()?;
        Ok(&self.session)
    }
    pub fn durable_seq(&self) -> u64 {
        self.head.seq
    }
    pub fn live_seq(&self) -> u64 {
        self.session.operation_count() as u64
    }
    pub fn recovery_required(&self) -> bool {
        self.poisoned
    }
    fn receipt_at(&self, index: usize) -> Result<Receipt> {
        let c = &self.calls[index];
        Ok(Receipt {
            seq: c.seq,
            outcome: self
                .session
                .recorded_outcome(index)
                .ok_or(Error::Format("outcome index"))?,
            durability: if c.seq <= self.head.seq {
                Durability::DurableThrough(self.head.seq)
            } else {
                Durability::AcceptedVolatile
            },
        })
    }
    pub fn lookup(&self, id: &str) -> Result<Option<Receipt>> {
        self.ready()?;
        self.calls
            .iter()
            .position(|c| c.id == id)
            .map(|i| self.receipt_at(i))
            .transpose()
    }
    pub fn submit(&mut self, id: &str, request: SnapshotRequest, mode: Mode) -> Result<Receipt> {
        self.ready()?;
        let bytes = SnapshotSession::encode_request(&request, self.limits.core)?;
        self.submit_encoded(id, &bytes, mode)
    }
    pub fn submit_encoded(&mut self, id: &str, request: &[u8], mode: Mode) -> Result<Receipt> {
        self.ready()?;
        check_id(id, self.limits)?;
        if request.len() > self.limits.max_field {
            return Err(Error::Limit("request"));
        }
        if let Some(i) = self.calls.iter().position(|c| c.id == id) {
            if self.calls[i].request != request {
                return Err(Error::RequestIdConflict);
            }
            if mode == Mode::Durable {
                self.barrier()?;
            }
            return self.receipt_at(i);
        }
        if self.calls.len() >= self.limits.max_operations {
            return Err(Error::Limit("operations"));
        }
        let (candidate, outcome, _) = self.session.prepare_call(request, None, self.limits.core)?;
        if outcome.len() > self.limits.max_field {
            return Err(Error::Limit("outcome"));
        }
        let call = Call {
            seq: next(self.live_seq())?,
            id: id.into(),
            request: request.to_vec(),
            outcome,
        };
        let bytes = wal_record(
            &call,
            &self.head,
            self.tip,
            state(&candidate, self.limits)?,
            self.limits,
        )?;
        let end = self
            .offset
            .checked_add(bytes.len() as u64)
            .filter(|n| *n <= self.limits.max_wal_bytes as u64)
            .ok_or(Error::Limit("WAL bytes"))?;
        let io = (|| {
            self.wal.seek(SeekFrom::Start(self.offset))?;
            let mid = bytes.len() / 2;
            self.wal.write_all(&bytes[..mid])?;
            fault("wal-half")?;
            self.wal.write_all(&bytes[mid..])?;
            fault("wal-written")?;
            Ok(())
        })();
        if let Err(e) = io {
            self.poisoned = true;
            return Err(Error::Indeterminate(Box::new(e)));
        }
        self.offset = end;
        self.tip = bytes[bytes.len() - 32..].try_into().unwrap();
        self.session = candidate;
        self.calls.push(call);
        if mode == Mode::Durable {
            self.barrier()?;
        }
        self.receipt_at(self.calls.len() - 1)
    }
    /// Durability only: callers must independently check semantic conformance,
    /// lifecycle usability and external trading authorization.
    pub fn barrier(&mut self) -> Result<u64> {
        self.ready()?;
        if self.live_seq() == self.head.seq {
            return Ok(self.head.seq);
        }
        let mut head = self.head.clone();
        head.generation = next(head.generation)?;
        head.seq = self.live_seq();
        head.offset = self.offset;
        head.tip = self.tip;
        head.state = state(&self.session, self.limits)?;
        let io = (|| {
            self.wal.sync_all()?;
            fault("wal-synced")?;
            publish(&self.dir, &head, self.limits)
        })();
        if let Err(e) = io {
            self.poisoned = true;
            return Err(Error::Indeterminate(Box::new(e)));
        }
        self.head = head;
        Ok(self.head.seq)
    }
    pub fn checkpoint(&mut self) -> Result<u64> {
        self.ready()?;
        let generation = next(if self.live_seq() == self.head.seq {
            self.head.generation
        } else {
            next(self.head.generation)?
        })?;
        let snap = snapshot(
            &self.session,
            &self.calls,
            self.head.session,
            generation,
            self.limits,
        )?;
        let mut head = self.head.clone();
        head.generation = generation;
        head.snapshot = generation;
        head.wal = generation;
        head.seq = self.live_seq();
        head.state = state(&self.session, self.limits)?;
        head.base = head.seq;
        head.snapshot_seq = head.seq;
        head.snapshot_hash = hash(&snap);
        let header = wal_header(&head, self.limits)?;
        if header.len() > self.limits.max_wal_bytes {
            return Err(Error::Limit("WAL header"));
        }
        head.offset = header.len() as u64;
        head.tip = header[header.len() - 32..].try_into().unwrap();
        self.barrier()?;
        let io = (|| {
            write_new(&path(&self.dir, 'S', generation), &snap)?;
            let wal = write_new(&path(&self.dir, 'W', generation), &header)?;
            sync_dir(&self.dir)?;
            fault("pair-dir-synced")?;
            publish(&self.dir, &head, self.limits)?;
            cleanup(&self.dir, &head)?;
            Ok(wal)
        })();
        match io {
            Ok(wal) => {
                self.wal = wal;
                self.offset = head.offset;
                self.tip = head.tip;
                self.head = head;
                Ok(generation)
            }
            Err(e) => {
                self.poisoned = true;
                Err(Error::Indeterminate(Box::new(e)))
            }
        }
    }
}

// No production environment-variable hooks. Opt-in feature is used ONLY by
// the process-crash example/CI. Unit fault injection is thread-local.
fn fault(point: &'static str) -> Result<()> {
    #[cfg(test)]
    if FAULT.with(|x| x.get() == Some(point)) {
        return Err(Error::Io(std::io::Error::other("injected I/O failure")));
    }
    #[cfg(feature = "fault-injection")]
    if std::env::var("MDC_STORE_KILL_AT").ok().as_deref() == Some(point) {
        println!("MDC_STORE_KILL_READY={point}");
        std::io::stdout().flush()?;
        let mut s = String::new();
        std::io::stdin().read_line(&mut s)?;
        return Err(Error::Format("fault worker resumed"));
    }
    let _ = point;
    Ok(())
}
#[cfg(test)]
thread_local! {static FAULT:std::cell::Cell<Option<&'static str>>=const{std::cell::Cell::new(None)};}
#[cfg(test)]
mod tests;
