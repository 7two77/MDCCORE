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
throughput envelope. No background timer, automatic N-event flush or automatic
rotation is provided. The single owner invokes barrier/checkpoint explicitly.

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
