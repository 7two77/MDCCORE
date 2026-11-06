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
fn publish(dir: &Path, head: &Head) -> Result<()> {
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

/// One Linux writer for a HEAD-certified prefix of isolated 014 decisions.
/// No background thread, shared mutable core, or implicit flush on Drop.
pub struct RunStore {
    dir: PathBuf,
    _lock: LockGuard,
    journal: File,
    manifest: RunManifest,
    dataset: Dataset,
    head: Head,
    bundles: Vec<DecisionBundle>,
    poisoned: bool,
}
impl RunStore {
    pub fn create(dir: impl AsRef<Path>, source: &[u8], manifest: RunManifest) -> Result<Self> {
        let bytes = manifest.encode()?;
        let dataset = manifest.dataset(source)?;
        let run = manifest.run_id();
        let mh = hash(&bytes);
        let hdr = header(run, mh)?;
        if hdr.len() > manifest.limits.max_journal_bytes {
            return Err(RecoveryError::Limit("journal bytes"));
        }
        let head = Head {
            run,
            manifest: mh,
            offset: hdr.len() as u64,
            count: 0,
            tip: hdr[hdr.len() - 32..].try_into().unwrap(),
            bundle: [0; 32],
        };
        fs::DirBuilder::new().mode(0o700).create(dir.as_ref())?;
        // Once a run directory exists, an error is an incomplete/indeterminate
        // create. Never delete or silently overwrite it as error recovery.
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
                _lock: guard,
                journal,
                manifest,
                dataset,
                head,
                bundles: Vec::new(),
                poisoned: false,
            })
        })();
        io.map_err(|e| RecoveryError::Indeterminate(Box::new(e)))
    }
    /// Validation occurs before any tail repair. Optional expected manifest
    /// prevents silently continuing against changed source/model/plan/limits.
    pub fn open(dir: impl AsRef<Path>, expected: Option<&RunManifest>) -> Result<Self> {
        let dir = dir.as_ref().canonicalize()?;
        let guard = lock(&dir, false)?;
        let bytes = read(&dir, "MANIFEST", RunLimits::default().max_manifest_bytes)?;
        let manifest = RunManifest::decode(&bytes)?;
        if expected.is_some_and(|m| m != &manifest) {
            return Err(RecoveryError::ExpectedManifest);
        }
        let source = read(&dir, "SOURCE", adapter_limits().max_input_bytes)?;
        let dataset = manifest.dataset(&source)?;
        let head_bytes = read(&dir, "HEAD", 512)?;
        let head = Head::decode(&head_bytes)?;
        if head.offset > manifest.limits.max_journal_bytes as u64 {
            return Err(RecoveryError::Limit("HEAD offset"));
        }
        let mut journal = file(&dir.join("JOURNAL"), true, false)?;
        if journal.metadata()?.len() < head.offset {
            return Err(RecoveryError::Integrity("missing committed journal"));
        }
        let mut prefix = Vec::new();
        (&mut journal).take(head.offset).read_to_end(&mut prefix)?;
        let bundles = validate_prefix(&bytes, &source, &head_bytes, &prefix)?;
        let repair = (|| {
            let had_tail = journal.metadata()?.len() != head.offset;
            if had_tail {
                journal.set_len(head.offset)?;
                fault("tail-truncated")?;
            }
            // Also seals a truncation performed by an earlier interrupted open.
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
            // A selected HEAD may come from an indeterminate rename whose
            // caller never received directory-sync success. Seal it before reads.
            sync_dir(&dir)?;
            fault("recovery-dir-synced")?;
            journal.seek(SeekFrom::Start(head.offset))?;
            Ok(())
        })();
        repair.map_err(|e| RecoveryError::Indeterminate(Box::new(e)))?;
        Ok(Self {
            dir,
            _lock: guard,
            journal,
            manifest,
            dataset,
            head,
            bundles,
            poisoned: false,
        })
    }
    fn ready(&self) -> Result<()> {
        if self._lock.owner != std::process::id() {
            return Err(RecoveryError::WrongProcess);
        }
        if self.poisoned {
            Err(RecoveryError::RecoveryRequired)
        } else {
            Ok(())
        }
    }
    pub fn recovery_required(&self) -> bool {
        self.poisoned
    }
    pub fn manifest(&self) -> &RunManifest {
        &self.manifest
    }
    pub fn durable_count(&self) -> Result<u64> {
        self.ready()?;
        Ok(self.head.count)
    }
    pub fn next_index(&self) -> Result<Option<u64>> {
        self.ready()?;
        Ok(if self.head.count == self.manifest.cutoffs.len() as u64 {
            None
        } else {
            Some(self.head.count + 1)
        })
    }
    pub fn lookup(&self, index: u64) -> Result<Option<&DecisionBundle>> {
        self.ready()?;
        let Some(zero) = index.checked_sub(1).and_then(|n| usize::try_from(n).ok()) else {
            return Ok(None);
        };
        Ok(self.bundles.get(zero))
    }
    /// Successful return certifies the entire prefix through `index`, conditional
    /// on filesystem/device sync guarantees. A bundle parsed in memory alone is
    /// not a physical durability acknowledgment.
    pub fn step(&mut self, index: u64) -> Result<DecisionBundle> {
        self.ready()?;
        if index == 0 || index > self.manifest.cutoffs.len() as u64 {
            return Err(RecoveryError::Index);
        }
        if index <= self.head.count {
            return Ok(self.bundles[(index - 1) as usize].clone());
        }
        if index != self.head.count + 1 {
            return Err(RecoveryError::Index);
        }
        let bundle = DecisionBundle::build(&self.dataset, &self.manifest, index)?;
        let record = bundle.encode(self.head.run, self.head.tip, self.manifest.limits)?;
        let offset = self
            .head
            .offset
            .checked_add(record.len() as u64)
            .ok_or(RecoveryError::Limit("journal offset"))?;
        if offset > self.manifest.limits.max_journal_bytes as u64 {
            return Err(RecoveryError::Limit("journal bytes"));
        }
        let head = Head {
            run: self.head.run,
            manifest: self.head.manifest,
            offset,
            count: index,
            tip: record[record.len() - 32..].try_into().unwrap(),
            bundle: bundle.digest(self.head.run),
        };
        let io = (|| {
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
        self.head = head;
        self.bundles.push(bundle.clone());
        Ok(bundle)
    }
}

fn fault(point: &'static str) -> Result<()> {
    #[cfg(test)]
    if FAULT.with(|v| v.get() == Some(point)) {
        return Err(std::io::Error::other("injected I/O error").into());
    }
    #[cfg(feature = "fault-injection")]
    if std::env::var("MDC_REPLAY_KILL_AT").ok().as_deref() == Some(point) {
        println!("MDC_REPLAY_KILL_READY={point}");
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
