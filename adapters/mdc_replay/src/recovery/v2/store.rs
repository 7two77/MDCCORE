use super::*;
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    os::{
        fd::AsRawFd,
        unix::fs::{DirBuilderExt, OpenOptionsExt},
    },
    path::{Path, PathBuf},
};

fn file(path: &Path, write: bool, new: bool) -> Result<File> {
    let f = OpenOptions::new()
        .read(true)
        .write(write)
        .create_new(new)
        .mode(0o600)
        .custom_flags(0o400000)
        .open(path)?; // Linux O_NOFOLLOW
    if !f.metadata()?.is_file() {
        return Err(RecoveryError::Format("regular file required"));
    }
    Ok(f)
}
unsafe extern "C" {
    fn flock(fd: std::ffi::c_int, op: std::ffi::c_int) -> std::ffi::c_int;
}
struct LockGuard {
    file: File,
    owner: u32,
}
impl Drop for LockGuard {
    fn drop(&mut self) {
        if self.owner != std::process::id() {
            return;
        }
        // flock belongs to an open file description. Closing this descriptor
        // alone can leave the lock alive in a concurrently forked child until
        // exec, even with CLOEXEC. Release it explicitly in its owning process.
        loop {
            if unsafe { flock(self.file.as_raw_fd(), 8) } == 0 {
                break;
            }
            if std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted {
                break;
            }
        }
    }
}
fn lock(dir: &Path, new: bool) -> Result<LockGuard> {
    let f = file(&dir.join("LOCK"), true, new)?;
    if unsafe { flock(f.as_raw_fd(), 2 | 4) } != 0 {
        let e = std::io::Error::last_os_error();
        return Err(if e.kind() == std::io::ErrorKind::WouldBlock {
            RecoveryError::LockBusy
        } else {
            RecoveryError::Io(e)
        });
    }
    Ok(LockGuard {
        file: f,
        owner: std::process::id(),
    })
}
fn sync_dir(dir: &Path) -> Result<()> {
    File::open(dir)?.sync_all()?;
    Ok(())
}
fn read(dir: &Path, name: &str, cap: usize) -> Result<Vec<u8>> {
    let f = file(&dir.join(name), false, false)?;
    if f.metadata()?.len() > cap as u64 {
        return Err(RecoveryError::Limit("file bytes"));
    }
    let mut b = Vec::new();
    f.take((cap as u64) + 1).read_to_end(&mut b)?;
    if b.len() > cap {
        return Err(RecoveryError::Limit("file bytes"));
    }
    Ok(b)
}
fn write_new(dir: &Path, name: &str, bytes: &[u8]) -> Result<File> {
    let mut f = file(&dir.join(name), true, true)?;
    let mid = bytes.len() / 2;
    f.write_all(&bytes[..mid])?;
    fault("create-half")?;
    f.write_all(&bytes[mid..])?;
    fault("create-written")?;
    f.sync_all()?;
    fault("create-synced")?;
    Ok(f)
}
fn publish(dir: &Path, head: &HeadV2) -> Result<()> {
    let b = head.encode()?;
    let mut f = file(&dir.join("HEAD.pending"), true, true)?;
    let mid = b.len() / 2;
    f.write_all(&b[..mid])?;
    fault("head-half")?;
    f.write_all(&b[mid..])?;
    fault("head-written")?;
    f.sync_all()?;
    fault("head-synced")?;
    fs::rename(dir.join("HEAD.pending"), dir.join("HEAD"))?;
    fault("head-renamed")?;
    sync_dir(dir)?;
    fault("head-dir-synced")?;
    Ok(())
}

#[derive(Clone)]
struct Entry {
    offset: u64,
    length: usize,
    previous: Hash,
    digest: Hash,
    decoded_before: usize,
}

/// Linux single-owner v2 store. Decoded bundles belong to the caller, never
/// to this index. Opening validates every committed decision before repair.
pub struct RunStoreV2 {
    dir: PathBuf,
    guard: LockGuard,
    journal: File,
    manifest: RunManifestV2,
    dataset: Dataset,
    head: HeadV2,
    index: Vec<Entry>,
    decoded_bytes: usize,
    poisoned: bool,
}

fn read_frame(journal: &mut File, offset: u64, end: u64, cap: usize) -> Result<Vec<u8>> {
    if end.checked_sub(offset).is_none_or(|n| n < 58) {
        return Err(RecoveryError::Integrity("v2 frame boundary"));
    }
    journal.seek(SeekFrom::Start(offset))?;
    let mut prefix = [0; 26];
    journal.read_exact(&mut prefix)?;
    let mut reader = Reader::new(&prefix);
    if reader.take(16)? != V2Kind::Decision.magic() || reader.u16()? != 2 {
        return Err(RecoveryError::Format("v2 decision magic/version"));
    }
    let body_len = reader.size(cap)?;
    let length = body_len
        .checked_add(58)
        .ok_or(RecoveryError::Limit("v2 frame length"))?;
    if length as u64 > end - offset {
        return Err(RecoveryError::Integrity("v2 frame exceeds HEAD"));
    }
    let mut bytes = vec![0; length];
    bytes[..26].copy_from_slice(&prefix);
    journal.read_exact(&mut bytes[26..])?;
    Ok(bytes)
}

impl RunStoreV2 {
    pub fn create(dir: impl AsRef<Path>, source: &[u8], manifest: RunManifestV2) -> Result<Self> {
        let bytes = manifest.encode()?;
        let dataset = manifest.logical.dataset(source)?;
        let run = manifest.run_id();
        let mh = hash(&bytes);
        let hdr = header2(run, mh)?;
        if hdr.len() > manifest.limits.max_physical_journal_bytes {
            return Err(RecoveryError::Limit("v2 journal bytes"));
        }
        let head = HeadV2 {
            run,
            manifest: mh,
            offset: hdr.len() as u64,
            count: 0,
            tip: hdr[hdr.len() - 32..].try_into().unwrap(),
            bundle: [0; 32],
        };
        fs::DirBuilder::new().mode(0o700).create(dir.as_ref())?;
        let io = (|| {
            let dir = dir.as_ref().canonicalize()?;
            let guard = lock(&dir, true)?;
            sync_dir(
                dir.parent()
                    .ok_or(RecoveryError::Format("parent directory"))?,
            )?;
            fault("create-parent-synced")?;
            write_new(&dir, "SOURCE", source)?;
            fault("source-synced")?;
            write_new(&dir, "MANIFEST", &bytes)?;
            fault("manifest-synced")?;
            let journal = write_new(&dir, "JOURNAL", &hdr)?;
            fault("journal-created")?;
            sync_dir(&dir)?;
            fault("create-files-synced")?;
            publish(&dir, &head)?;
            Ok(Self {
                dir,
                guard,
                journal,
                manifest,
                dataset,
                head,
                index: Vec::new(),
                decoded_bytes: 0,
                poisoned: false,
            })
        })();
        io.map_err(|e| RecoveryError::Indeterminate(Box::new(e)))
    }

    pub fn open(dir: impl AsRef<Path>, expected: Option<&RunManifestV2>) -> Result<Self> {
        let dir = dir.as_ref().canonicalize()?;
        let guard = lock(&dir, false)?;
        let bytes = read(
            &dir,
            "MANIFEST",
            StorageLimits::default().max_manifest_bytes,
        )?;
        let manifest = RunManifestV2::decode(&bytes)?;
        if expected.is_some_and(|m| m != &manifest) {
            return Err(RecoveryError::ExpectedManifest);
        }
        let source = read(&dir, "SOURCE", adapter_limits().max_input_bytes)?;
        let dataset = manifest.logical.dataset(&source)?;
        let head = HeadV2::decode(&read(&dir, "HEAD", 512)?)?;
        let hdr = header2(manifest.run_id(), hash(&bytes))?;
        let mut tip: Hash = hdr[hdr.len() - 32..].try_into().unwrap();
        if head.run != manifest.run_id()
            || head.manifest != hash(&bytes)
            || head.offset < hdr.len() as u64
            || head.offset > manifest.limits.max_physical_journal_bytes as u64
            || head.count > manifest.cutoffs().len() as u64
        {
            return Err(RecoveryError::Integrity("v2 HEAD binding/bounds"));
        }
        let mut journal = file(&dir.join("JOURNAL"), true, false)?;
        if journal.metadata()?.len() < head.offset {
            return Err(RecoveryError::Integrity("v2 missing committed bytes"));
        }
        let mut actual = vec![0; hdr.len()];
        journal.read_exact(&mut actual)?;
        if actual != hdr {
            return Err(RecoveryError::Integrity("v2 journal header"));
        }
        let mut at = hdr.len() as u64;
        let mut index = Vec::new();
        let mut decoded_bytes = 0;
        let mut last_bundle = [0; 32];
        for n in 1..=head.count {
            let record = read_frame(
                &mut journal,
                at,
                head.offset,
                manifest.limits.max_encoded_record_bytes,
            )?;
            let (bundle, digest, total) =
                decode_record(&record, head.run, tip, manifest.limits, decoded_bytes)?;
            // Includes bounded root snapshot restoration, recomputation and exact
            // report comparison, one isolated decision at a time.
            if bundle.index != n || bundle != DecisionBundle::build(&dataset, &manifest.logical, n)?
            {
                return Err(RecoveryError::Integrity("v2 decision replay"));
            }
            last_bundle = bundle_digest(head.run, &bundle);
            index.push(Entry {
                offset: at,
                length: record.len(),
                previous: tip,
                digest,
                decoded_before: decoded_bytes,
            });
            at += record.len() as u64;
            tip = digest;
            decoded_bytes = total;
        }
        if at != head.offset || tip != head.tip || last_bundle != head.bundle {
            return Err(RecoveryError::Integrity("v2 HEAD prefix"));
        }
        let repair: Result<()> = (|| {
            let had_tail = journal.metadata()?.len() != head.offset;
            if had_tail {
                journal.set_len(head.offset)?;
                fault("tail-truncated")?;
            }
            journal.sync_all()?;
            fault("recovery-journal-synced")?;
            if had_tail {
                fault("tail-synced")?;
            }
            match fs::remove_file(dir.join("HEAD.pending")) {
                Ok(()) => (),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
                Err(e) => return Err(e.into()),
            }
            sync_dir(&dir)?;
            fault("recovery-dir-synced")?;
            journal.seek(SeekFrom::Start(head.offset))?;
            Ok(())
        })();
        repair.map_err(|e| RecoveryError::Indeterminate(Box::new(e)))?;
        Ok(Self {
            dir,
            guard,
            journal,
            manifest,
            dataset,
            head,
            index,
            decoded_bytes,
            poisoned: false,
        })
    }

    fn ready(&self) -> Result<()> {
        if self.guard.owner != std::process::id() {
            return Err(RecoveryError::WrongProcess);
        }
        if self.poisoned {
            return Err(RecoveryError::RecoveryRequired);
        }
        Ok(())
    }
    pub fn recovery_required(&self) -> bool {
        self.poisoned
    }
    pub fn manifest(&self) -> &RunManifestV2 {
        &self.manifest
    }
    pub fn durable_count(&self) -> Result<u64> {
        self.ready()?;
        Ok(self.head.count)
    }
    pub fn physical_journal_bytes(&self) -> Result<u64> {
        self.ready()?;
        Ok(self.head.offset)
    }
    pub fn decoded_blob_bytes(&self) -> Result<usize> {
        self.ready()?;
        Ok(self.decoded_bytes)
    }
    pub fn next_index(&self) -> Result<Option<u64>> {
        self.ready()?;
        Ok((self.head.count < self.manifest.cutoffs().len() as u64).then_some(self.head.count + 1))
    }
    pub fn lookup_owned(&mut self, n: u64) -> Result<Option<DecisionBundle>> {
        self.ready()?;
        let Some(entry) = n
            .checked_sub(1)
            .and_then(|n| usize::try_from(n).ok())
            .and_then(|n| self.index.get(n))
            .cloned()
        else {
            return Ok(None);
        };
        let result = (|| {
            let record = read_frame(
                &mut self.journal,
                entry.offset,
                entry.offset + entry.length as u64,
                self.manifest.limits.max_encoded_record_bytes,
            )?;
            // Authenticate the physical frame against the certified index before
            // parsing blob envelopes or invoking either blob decoder.
            if record.len() != entry.length {
                return Err(RecoveryError::Integrity("v2 owned frame changed"));
            }
            let (body, digest) = parse2(
                V2Kind::Decision,
                &record,
                self.manifest.limits.max_encoded_record_bytes,
            )?;
            if digest != entry.digest {
                return Err(RecoveryError::Integrity("v2 owned frame changed"));
            }
            let (bundle, _) = decode_record_body(
                body,
                self.head.run,
                entry.previous,
                self.manifest.limits,
                entry.decoded_before,
            )?;
            // Semantic replay has already certified this exact physical frame.
            if bundle.index != n {
                return Err(RecoveryError::Integrity("v2 owned frame changed"));
            }
            Ok(Some(bundle))
        })();
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
    pub fn step(&mut self, n: u64) -> Result<DecisionBundle> {
        self.ready()?;
        if n == 0 || n > self.manifest.cutoffs().len() as u64 || n > self.head.count + 1 {
            return Err(RecoveryError::Index);
        }
        if n <= self.head.count {
            return self.lookup_owned(n)?.ok_or(RecoveryError::Index);
        }
        let b = DecisionBundle::build(&self.dataset, &self.manifest.logical, n)?;
        let total = decoded_total(&b, self.decoded_bytes, self.manifest.limits)?;
        let record = encode_record(&b, self.head.run, self.head.tip, self.manifest.limits)?;
        let offset = self
            .head
            .offset
            .checked_add(record.len() as u64)
            .ok_or(RecoveryError::Limit("v2 journal offset"))?;
        if offset > self.manifest.limits.max_physical_journal_bytes as u64 {
            return Err(RecoveryError::Limit("v2 physical journal"));
        }
        let digest = record[record.len() - 32..].try_into().unwrap();
        let head = HeadV2 {
            run: self.head.run,
            manifest: self.head.manifest,
            offset,
            count: n,
            tip: digest,
            bundle: bundle_digest(self.head.run, &b),
        };
        let io: Result<()> = (|| {
            self.journal.seek(SeekFrom::Start(self.head.offset))?;
            let mid = record.len() / 2;
            self.journal.write_all(&record[..mid])?;
            fault("record-half")?;
            self.journal.write_all(&record[mid..])?;
            fault("record-written")?;
            self.journal.sync_all()?;
            fault("record-synced")?;
            publish(&self.dir, &head)?;
            fault("before-reply")?;
            Ok(())
        })();
        if let Err(e) = io {
            self.poisoned = true;
            return Err(RecoveryError::Indeterminate(Box::new(e)));
        }
        self.index.push(Entry {
            offset: self.head.offset,
            length: record.len(),
            previous: self.head.tip,
            digest,
            decoded_before: self.decoded_bytes,
        });
        self.head = head;
        self.decoded_bytes = total;
        Ok(b)
    }
}

fn fault(point: &'static str) -> Result<()> {
    #[cfg(test)]
    if FAULT.with(|v| v.get() == Some(point)) {
        return Err(std::io::Error::other("injected I/O error").into());
    }
    #[cfg(feature = "fault-injection")]
    if std::env::var("MDC_REPLAY_V2_KILL_AT").ok().as_deref() == Some(point) {
        println!("MDC_REPLAY_V2_KILL_READY={point}");
        std::io::stdout().flush()?;
        let mut line = String::new();
        std::io::stdin().read_line(&mut line)?;
        return Err(std::io::Error::other("injected I/O error after pause").into());
    }
    let _ = point;
    Ok(())
}
#[cfg(test)]
thread_local! {static FAULT:std::cell::Cell<Option<&'static str>>=const{std::cell::Cell::new(None)};}

#[cfg(test)]
mod tests;
