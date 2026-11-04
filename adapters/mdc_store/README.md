# mdc_store — Linux single-writer MDC-WAL-001/1 adapter

Independent std-only host crate. The root `mdc_core` remains dependency-free and
supports no_std + alloc. SHA-256 uses pinned RustCrypto sha2 0.10.9 (host only).
Linux flock is an advisory OS lock held by the Store file descriptor, not a PID
file. The caller must own/control the store directory and all writers must honor
this protocol. This is not authentication against a process rewriting the files.

`Store::create(path, registry, protocol, limits)` requires a nonexistent directory
under an existing parent, creates a fresh empty recorded session, and syncs the
initial files/directories. It does not import arbitrary pre-existing Pipelines.
`Store::open(path, limits)` restores only the prefix certified by HEAD. A damaged
store fails closed; it is never silently replaced by an empty one.

`submit(id, SnapshotRequest, Mode::Buffered)` appends a complete WAL record and
returns AcceptedVolatile. It may be lost after restart. `Mode::Durable` commits
all previous inputs/receipts before returning; `barrier()` explicitly commits the
whole live prefix. Semantic failures are persisted outcomes, not storage errors.
`lookup(id)` and identical-ID retry return the original outcome; conflicting
request bytes with the same ID are rejected without mutation.

`session()` is a shared LIVE view: inspect `live_seq()` and `durable_seq()`.
A durable barrier certifies storage, not conformance, lifecycle usability or
trading authorization. Critical decisions must cross a durable barrier before
external use AND satisfy the consumer's semantic/authorization rules. The adapter
never sends orders or modifies broker state.

I/O errors after write begins return Indeterminate and fence further access:
`session`, `lookup`, `submit`, `barrier`, `checkpoint` require reopen. Diagnostic
sequence getters remain available, but after an indeterminate error they are
only last-known in-memory values, not proof of disk state. Errors before I/O
(e.g. request/limit checks) preserve the usable session.

`checkpoint()` first prepares bounded snapshot data, flushes the live prefix,
publishes a synced new snapshot/WAL pair and HEAD, then removes older generations.
WAL rotation does not prune the full recorded core history or host request IDs.
Full candidate export/restore is used for isolation: memory/CPU and throughput
are bounded only by explicit logical limits; this is not yet a measured trading
throughput envelope. Raw Store has no background timer or automatic rotation. MDC-IMPL-013 adds
optional ScheduledStore for time/count flush; checkpoint/rotation stays explicit.

The core bridge, framing and flow follow `docs/MDC-WAL-WIRE-CONTRACT-v1.md`.
Default host limits: 64 MiB frame/WAL, 2 MiB request/outcome, 256-byte request ID,
4096 operations, plus the existing SnapshotLimits. They are checked before I/O
where the candidate can be validated. No hard allocator/time guarantee is claimed.

Run from repository root:

```bash
cargo fetch --locked --manifest-path adapters/mdc_store/Cargo.toml
cargo test --locked --offline --manifest-path adapters/mdc_store/Cargo.toml
cargo test --locked --offline --manifest-path adapters/mdc_store/Cargo.toml --features fault-injection
bash .github/ci/store.sh
```

The `fault-injection` feature is for testing only. It makes the verification
binary block at explicit I/O points under MDC_STORE_KILL_AT so a parent test can
SIGKILL it. Normal builds ignore that environment variable. Never enable this
feature in the deployed host. `mdc-recovery` is a verification driver, not a
production daemon or trading client.

Tests cover simulated I/O failures and actual process termination. Power-loss
behavior still depends on supported filesystem/mount/driver/device sync
semantics; these tests do not validate every hardware stack. Network filesystems,
multiple writers, hostile file modification, source cursor transactions,
exactly-once external delivery, reinstatement, compaction and historical
available_at/decision_at eligibility are outside this package.

## Optional single-worker batch scheduler (MDC-IMPL-013)

ScheduledStore::create/open take an explicit BatchPolicy (max_pending, max_delay,
queue_capacity). No global interval or default loss-window SLA is imposed. One
worker owns Store and flock; cloneable StoreClient values send bounded commands.
Clients cannot obtain a raw Store or a shared live pipeline reference. snapshot()
returns owned 011 bytes and status from one worker position; bytes can contain a
volatile suffix. status() and lookup() expose current durability.

A successful count threshold barrier updates the triggering receipt to durable.
The timer starts for the oldest pending suffix, does not reset on each request,
and wakes while idle. It is also checked between commands under continuous load.
Monotonic scheduling is soft: OS scheduling, core preparation, a full queue and
blocking sync can delay observation/publication. max_delay is not a real-time
ACK guarantee. No background checkpoint, history compaction or external timer
framework is introduced.

Submit/lookup/barrier/checkpoint/snapshot/status are synchronous RPCs; queue
admission blocks when full. Stable IDs resolve lost replies; dropping a caller
is not cancellation of an already queued operation. Concurrent producers are
serialized in accepted queue order, not a promised cross-thread arrival order.
Queue capacity bounds queued commands, not all caller memory or blocked threads.

Autonomous flush failure becomes a fenced status with last_error; subsequent
mutations and snapshot reads fail RecoveryRequired. Durable sequence is last-known
only when unhealthy. shutdown() processes the queued prefix, flushes and joins
before returning; check its Result. StoreClient clones stop working after it.
Drop attempts shutdown/join but cannot report an error or provide a durable ACK.
Blocked storage can delay shutdown; no hard wall-clock bound is claimed. Durable
receipts remain storage evidence, not semantic or trading authorization.
Scheduler lifecycle failures use typed InvalidBatchPolicy, SchedulerStopped and
WorkerPanicked errors in addition to the existing storage errors. External
exhaustive matches on the host Error enum need these new variants. A constructor
error after store creation/thread startup does not authorize silently resetting
an existing directory; inspect/open the store explicitly.
