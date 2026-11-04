//! One worker owns Store; explicit bounded queue and soft monotonic flush policy.
use crate::{check_id, Error, Limits, Mode, Receipt, Result, Store};
use mdc_core::{
    contract::registry::Registry,
    identity::IdentityProtocol,
    pipeline::{SnapshotRequest, SnapshotSession},
};
use std::{
    path::Path,
    sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, SyncSender},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug)]
pub struct BatchPolicy {
    pub max_pending: usize,
    pub max_delay: Duration,
    pub queue_capacity: usize,
}
impl BatchPolicy {
    fn validate(self, limits: Limits) -> Result<()> {
        if self.max_pending == 0
            || self.max_pending > limits.max_operations
            || self.max_delay.is_zero()
            || self.queue_capacity == 0
            || self.queue_capacity > 1024
            || Instant::now().checked_add(self.max_delay).is_none()
        {
            return Err(Error::InvalidBatchPolicy(
                "positive bounded count/delay/queue required",
            ));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchedulerStatus {
    pub live_seq: u64,
    pub durable_seq: u64,
    pub healthy: bool,
    pub last_error: Option<String>,
}
pub struct ScheduledSnapshot {
    pub bytes: Vec<u8>,
    pub status: SchedulerStatus,
}
type Reply<T> = Sender<Result<T>>;
enum Command {
    Submit(String, Vec<u8>, Mode, Reply<Receipt>),
    Lookup(String, Reply<Option<Receipt>>),
    Barrier(Reply<u64>),
    Checkpoint(Reply<u64>),
    Snapshot(Reply<ScheduledSnapshot>),
    Status(Reply<SchedulerStatus>),
    Stop(Reply<SchedulerStatus>),
}
#[derive(Clone)]
pub struct StoreClient {
    sender: SyncSender<Command>,
    limits: Limits,
}
impl StoreClient {
    fn call<T>(&self, make: impl FnOnce(Reply<T>) -> Command) -> Result<T> {
        let (tx, rx) = mpsc::channel();
        self.sender
            .send(make(tx))
            .map_err(|_| Error::SchedulerStopped)?;
        rx.recv().map_err(|_| Error::SchedulerStopped)?
    }
    /// Queue admission blocks when full. A dropped receiver does not cancel a
    /// queued operation; stable request ID is required to resolve a lost reply.
    pub fn submit(&self, id: &str, request: SnapshotRequest, mode: Mode) -> Result<Receipt> {
        check_id(id, self.limits)?;
        let bytes = SnapshotSession::encode_request(&request, self.limits.core)?;
        if bytes.len() > self.limits.max_field {
            return Err(Error::Limit("request"));
        }
        self.call(|r| Command::Submit(id.into(), bytes, mode, r))
    }
    pub fn lookup(&self, id: &str) -> Result<Option<Receipt>> {
        check_id(id, self.limits)?;
        self.call(|r| Command::Lookup(id.into(), r))
    }
    pub fn barrier(&self) -> Result<u64> {
        self.call(Command::Barrier)
    }
    pub fn checkpoint(&self) -> Result<u64> {
        self.call(Command::Checkpoint)
    }
    pub fn status(&self) -> Result<SchedulerStatus> {
        self.call(Command::Status)
    }
    /// Snapshot and status refer to one worker position; live bytes may contain
    /// volatile state. No raw shared/mutable Store escapes the worker.
    pub fn snapshot(&self) -> Result<ScheduledSnapshot> {
        self.call(Command::Snapshot)
    }
}
/// Owner joins the worker on shutdown/Drop. Explicit shutdown is the only way
/// to observe a shutdown flush result; Drop provides no durable receipt.
pub struct ScheduledStore {
    client: StoreClient,
    worker: Option<JoinHandle<()>>,
}
impl ScheduledStore {
    pub fn create(
        path: impl AsRef<Path>,
        registry: Registry,
        protocol: IdentityProtocol,
        limits: Limits,
        policy: BatchPolicy,
    ) -> Result<Self> {
        policy.validate(limits)?;
        Self::launch(Store::create(path, registry, protocol, limits)?, policy)
    }
    pub fn open(path: impl AsRef<Path>, limits: Limits, policy: BatchPolicy) -> Result<Self> {
        policy.validate(limits)?;
        Self::launch(Store::open(path, limits)?, policy)
    }
    fn launch(store: Store, policy: BatchPolicy) -> Result<Self> {
        let limits = store.limits;
        let (tx, rx) = mpsc::sync_channel(policy.queue_capacity);
        let worker = thread::Builder::new()
            .name("mdc-store-flush".into())
            .spawn(move || run(store, rx, policy))?;
        Ok(Self {
            client: StoreClient { sender: tx, limits },
            worker: Some(worker),
        })
    }
    pub fn client(&self) -> StoreClient {
        self.client.clone()
    }
    pub fn shutdown(mut self) -> Result<SchedulerStatus> {
        let result = self.client.call(Command::Stop);
        let handle = self.worker.take().ok_or(Error::SchedulerStopped)?;
        if handle.join().is_err() {
            return Err(Error::WorkerPanicked);
        }
        result
    }
}
impl Drop for ScheduledStore {
    fn drop(&mut self) {
        if let Some(handle) = self.worker.take() {
            let _ = self.client.call(Command::Stop);
            let _ = handle.join();
        }
    }
}
fn status(store: &Store, failure: &Option<String>) -> SchedulerStatus {
    SchedulerStatus {
        live_seq: store.live_seq(),
        durable_seq: store.durable_seq(),
        healthy: failure.is_none() && !store.recovery_required(),
        last_error: failure.clone(),
    }
}
fn flush(store: &mut Store, failure: &mut Option<String>) -> Result<u64> {
    let result = store.barrier();
    if let Err(e) = &result {
        *failure = Some(e.to_string());
    }
    result
}
fn run(mut store: Store, rx: Receiver<Command>, policy: BatchPolicy) {
    let mut deadline: Option<Instant> = None;
    let mut failure = None;
    loop {
        // Checked before EVERY receive, so a busy queue cannot indefinitely reset
        // the oldest pending deadline. This is soft scheduling, not real time.
        if failure.is_none() && deadline.is_some_and(|t| Instant::now() >= t) {
            let _ = flush(&mut store, &mut failure);
            deadline = None;
        }
        let command = match deadline.filter(|_| failure.is_none()) {
            Some(t) => match rx.recv_timeout(t.saturating_duration_since(Instant::now())) {
                Ok(c) => c,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => {
                    let _ = flush(&mut store, &mut failure);
                    return;
                }
            },
            None => match rx.recv() {
                Ok(c) => c,
                Err(_) => {
                    if failure.is_none() {
                        let _ = flush(&mut store, &mut failure);
                    }
                    return;
                }
            },
        };
        match command {
            Command::Status(reply) => {
                let _ = reply.send(Ok(status(&store, &failure)));
            }
            Command::Stop(reply) => {
                let result = if failure.is_some() {
                    Err(Error::RecoveryRequired)
                } else {
                    flush(&mut store, &mut failure).map(|_| status(&store, &failure))
                };
                let _ = reply.send(result);
                return;
            }
            Command::Submit(id, request, mode, reply) => {
                let mut result = if failure.is_some() {
                    Err(Error::RecoveryRequired)
                } else {
                    store
                        .submit_encoded(&id, &request, mode)
                        .and_then(|receipt| {
                            if store.live_seq() - store.durable_seq() >= policy.max_pending as u64 {
                                flush(&mut store, &mut failure)?;
                                store.lookup(&id)?.ok_or(Error::Format("lost receipt"))
                            } else {
                                Ok(receipt)
                            }
                        })
                };
                if store.recovery_required() {
                    if let Err(e) = &result {
                        failure = Some(e.to_string());
                    }
                }
                // Establish the oldest-pending deadline BEFORE delivering a
                // volatile receipt. Descheduling after ACK must not reset age.
                if failure.is_none() && store.live_seq() > store.durable_seq() && deadline.is_none()
                {
                    deadline = Instant::now().checked_add(policy.max_delay);
                    if deadline.is_none() {
                        failure = Some("flush deadline overflow".into());
                        result = Err(Error::RecoveryRequired);
                    }
                }
                let _ = reply.send(result);
            }
            Command::Lookup(id, reply) => {
                let _ = reply.send(if failure.is_some() {
                    Err(Error::RecoveryRequired)
                } else {
                    store.lookup(&id)
                });
            }
            Command::Barrier(reply) => {
                let result = if failure.is_some() {
                    Err(Error::RecoveryRequired)
                } else {
                    flush(&mut store, &mut failure)
                };
                let _ = reply.send(result);
            }
            Command::Checkpoint(reply) => {
                let result = if failure.is_some() {
                    Err(Error::RecoveryRequired)
                } else {
                    store.checkpoint()
                };
                if store.recovery_required() {
                    if let Err(e) = &result {
                        failure = Some(e.to_string());
                    }
                }
                let _ = reply.send(result);
            }
            Command::Snapshot(reply) => {
                let result = if failure.is_some() {
                    Err(Error::RecoveryRequired)
                } else {
                    store.session().and_then(|s| {
                        Ok(ScheduledSnapshot {
                            bytes: s.export(store.limits.core)?,
                            status: status(&store, &failure),
                        })
                    })
                };
                let _ = reply.send(result);
            }
        }
        if store.recovery_required() && failure.is_none() {
            failure = Some("underlying store requires recovery".into());
        }
        if failure.is_some() || store.live_seq() == store.durable_seq() {
            deadline = None;
        } else if deadline.is_none() {
            deadline = Instant::now().checked_add(policy.max_delay);
            if deadline.is_none() {
                failure = Some("flush deadline overflow".into());
            }
        }
    }
}
