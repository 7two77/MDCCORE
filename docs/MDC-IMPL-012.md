# MDC-IMPL-012 — Bounded Linux WAL/recovery adapter

2026-11-04. Base: main `737f5b6bc6670642669badbdde48e2a8984c410d`, after PR #6.
Implements [MDC-WAL-001/1](MDC-WAL-WIRE-CONTRACT-v1.md) for Linux local files,
single writer, bounded recorded sessions. This is a storage implementation;
it does not claim universal power-loss recovery or production trading readiness.

## Package boundaries and API

The independent host crate is `adapters/mdc_store` (`publish = false`). It uses std
and pinned RustCrypto sha2 0.10.9 with a separate lockfile. The root Cargo.toml,
root Cargo.lock and preserved nested source tree remain unchanged. Root core is
still dependency-free and supports default/std and no_std + alloc.

Core additions: SnapshotRequest and SnapshotOutcome; exact encode/decode request;
recorded_call/recorded_outcome queries; prepare_call. The last method creates a
fresh candidate by verified export/restore, applies one original-order request,
optionally checks exact expected outcome and validates the bounded candidate.
An error leaves the source session unchanged. Semantic rejection is a recorded
outcome. No filesystem effects or mutable private Pipeline access are added.

| Store API | Behavior |
|---|---|
| create(path, Registry, protocol, Limits) | Explicit new empty store under an existing parent; no existing graph import |
| open(path, Limits) | Lock, validate HEAD/snapshot/WAL, replay committed prefix, then publish usable state |
| submit(id, SnapshotRequest, Mode) / submit_encoded | Full-ID/request comparison; append one bounded prepared operation, including semantic failures |
| Mode::Buffered | AcceptedVolatile: live result can be lost after restart |
| Mode::Durable / barrier() | WAL sync, new HEAD file sync, rename and directory sync before returning durable sequence |
| lookup(id) | Original typed historical outcome with current durability; no repeated core call |
| checkpoint() | Validate snapshot budgets, commit live prefix, publish new snapshot/WAL pair, then clean old files |
| session() | Shared LIVE view; compare live_seq with durable_seq before assuming durability |

Durability::DurableThrough(D) certifies storage only. Semantic conformance,
lifecycle usability and external trading authorization are separate. There is
no automatic authorization or order submission. Critical consumers must require
a durable barrier covering their entire input prefix before external use.

After I/O becomes indeterminate the instance is fenced: session, lookup, submit,
barrier and checkpoint fail until reopen. Drop the old Store to release flock
before opening a replacement. Sequence getters after failure are diagnostic
last-known in-memory values, not proof of the selected on-disk HEAD.

## File protocol implemented

The codec follows the fixed v1 magic/length/version/domain/SHA-256 framing and
eight frozen framing vectors. Snapshot wrappers retain the full 011 archive and
ordered host receipts. WAL records retain session/generation/seq, complete request
ID and bytes, exact outcome, resulting state hash and previous frame hash.

HEAD selects one exact snapshot and one WAL generation, a committed offset,
durable sequence and tip/state hashes. No independent tail marker or mtime
heuristic selects committed state. Recovery verifies snapshot receipt equality
against the core journal and every WAL outcome against production replay, then
requires exact final state/sequence/tip. Recomputed valid checksums cannot hide a
forged outcome or substituted snapshot receipt in the negative tests.

WAL bytes beyond the committed offset are ignored and removed before append.
Corruption inside that offset is rejected, with no silent fallback/reset. Missing
store is distinct from an incomplete existing one. An unacknowledged but fully
published new HEAD may be recovered; stable host request IDs handle the lost ACK.
An identical retry produces no new artifact, audit or event; conflicting bytes
under an existing ID are rejected. All nine core mutation kinds are supported.

Linux flock is held for the Store lifetime on a persistent LOCK inode. The binding
supports Rust 1.85.1 without relying on newer std lock APIs. This is an advisory
single-writer protocol for an owned directory, not protection against hostile
file modification or a writer ignoring the lock.

New generation files and their directory entries are synced before HEAD can name
them. HEAD temp is synced, renamed, then its directory is synced before ACK. Old
generation deletion follows successful publication. Recovery also syncs the
selected HEAD directory before suffix truncation or orphan cleanup: the previous
process could have died between rename and directory sync.

## Verification and scope of evidence

Local Rust 1.85.1 / 1.98.0 matrix:

- Root core: **213 Rust tests + 25 Python checks** in each of six feature configurations.
- Linux host: **23 default-build tests**, **25 fault-injection tests** per toolchain.
- Format, Clippy with warnings denied, offline locked checks/builds after an
  explicit dependency fetch, examples and old scalar E2E pass.
- Root portable/snapshot controls remain unchanged. The rich core archive retains
  38 operations, 31 artifacts, lifecycle revision 2, with exact export bytes.
- Each host toolchain runs a producer and two independent consumers for WAL
  recovery and again for rotated checkpoint recovery. The recorded core archive
  SHA-256 remains `349f902ecd41fec5da8c158eba913fe8812bace0dcfb9283547c3a250f796928`.
- SIGKILL covers eight append/HEAD points and ten checkpoint/cleanup points.
  Separate injected I/O tests exercise partial files and fenced state.

[Local receipt](evidence/impl-012-local-verification.json) records actual counts,
toolchains, hashes of logs, process receipts and dirty pre-commit working-copy
metadata. [Source manifest](evidence/impl-012-source-manifest.json) fingerprints
the independent host sources/manifest/lockfile and frozen framing vectors as well
as the core. It excludes build outputs. Hosted execution is recorded separately.

CI adds two Linux host jobs. Four cross-target jobs continue core portable and
snapshot tests and now include four bridge tests each. Host filesystem recovery
is tested natively on x86_64 Linux; a host recovery claim for i686 or s390x is not
inferred from core cross-target results.

| Recovery scenario | Implemented evidence / limit |
|---|---|
| R01 | Fresh-process exact WAL and checkpoint recovery, continuation and rich graph |
| R02 | Complete or arbitrary volatile suffix discarded; committed prefix retained |
| R03 | Whole-prefix barrier, including invalid/audit state and all semantic levels |
| R04 | Durable producer restart and exact receipt/state comparison |
| R05 | Kill after HEAD publication before reply; retry ID creates one core effect |
| R06 | I/O failure fencing and diagnostic last-known sequences |
| R07 | Kill on half/full write, file sync, rename, directory sync and rotation/cleanup boundaries |
| R08 | Checksum, bounds, version/EOF, short committed WAL, forged outcomes/receipts, foreign header |
| R09 | Same-process and different-process contention; lock released after SIGKILL |
| R10 | Full rich fixture preserves lifecycle, Evaluation aliases, captured missing inputs and audit |
| R11 | Bounds/errors before I/O leave prior HEAD/WAL/session unchanged; no hard OOM/time promise |
| R12 | Storage checkpoint/rotation portion implemented; external cursor transactions excluded by wire v1 |

The tests exercise real process termination on the test filesystem and simulated
I/O errors. They are not physical power-cut validation. The fault-injection
feature is opt-in and absent from normal deployed builds; only test drivers use
MDC_STORE_KILL_AT. The normal build is separately tested and rebuilt for the
independent process verifier.

## Operational limits and next work

This first adapter uses full candidate export/restore for isolation and retains
all core history and request IDs. Logical defaults are 4096 calls, 16 MiB core
archive, 64 MiB frame/WAL and 2 MiB request/outcome fields. WAL rotation does not
compact core history. Performance recorded for the fixture is execution timing,
not a production throughput or latency SLO. No hard memory/CPU bound is claimed.

Buffered calls require an owner to invoke barrier/checkpoint. No background timer,
automatic N-event flush or automatic rotation is included. Interval and N remain
explicit consumer policy, not a guaranteed one-second loss window. Reaching a
logical limit returns an error/backpressure instead of silently pruning history.

Source cursors, strategy state, broker positions/orders and exactly-once external
delivery are not persisted. Reinstatement and historical available_at/decision_at
eligibility are still outside this profile. Before a trading consumer relies on
causal replay, its domain/availability contract is required. Before production
durability claims, validate the chosen filesystem, mount/driver/device sync stack
and measure resource/flush/recovery budgets. No main merge or release is implied
by preparing this implementation branch.
