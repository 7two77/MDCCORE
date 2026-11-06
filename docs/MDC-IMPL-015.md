# MDC-IMPL-015 — Exact recovery of isolated replay decisions

2026-11-06. Base main 9f21fe7 after PR #9. This realizes the recovery proposal
prepared in OPS-005. Runtime lives in the independent mdc_replay host crate;
root core, original nested package, mdc_store and their persisted formats remain
unchanged. No dependency is added; fault-injection is an opt-in test feature.

## Behavior and API

An 014 cutoff has its own Registry and SnapshotSession. RunStore retains separate
completed decision bundles instead of importing populated sessions into the old
core Store or sharing a later full graph with an earlier decision.

RunManifest::new binds exact captured CSV bytes/hash, profile, explicit nonempty
strictly increasing cutoffs and limits. The run namespace includes the full source
and plan but is excluded from earlier eligible decision/core identity. Opening
with an expected manifest rejects changed source/model/plan/limits before repair;
opening without one explicitly uses the retained source. An appended CSV suffix
requires a separate run. Ordinary input-file paths are never reread implicitly.

RunStore::create/open takes a single Linux writer lock. step(index) returns only
after append/sync and atomic HEAD publication/directory sync. A committed-index
retry returns the retained exact bundle without a new record. Skipped indices,
invalid plans and resource limits do not advance the cursor. next_index returns
None at completion. lookup and durable_count expose only a validated/synced prefix.
The in-memory portable validate_prefix decoder verifies logical certification;
it does not itself acknowledge physical durability.

Recovery validates HEAD/source/manifest/chain and replays engine-011 snapshots,
recomputing every matching 014 decision through production APIs. Reports and
snapshots must match byte-for-byte. Uncommitted suffix cannot advance HEAD and
is truncated before further use. A corrupted committed prefix fails without
fallback to an older candidate. Missing HEAD after partial create is an error.

Post-mutation I/O error fences the object until reopen. Open always syncs the
selected journal and directory before exposing it, including after an earlier
interrupted truncate or indeterminate HEAD rename. This seals the recovered
position before a new successful read/retry receipt. No success is inferred
merely because a complete record is visible in the file.

The writer is process-bound. Owner drop explicitly releases flock rather than
relying only on descriptor close: a parallel fork can briefly retain a duplicate
open file description until exec. A regression keeps such a duplicate alive
across drop/reopen, and ten parallel fault-suite repetitions passed after the fix.
An inherited RunStore rejects access from another process with WrongProcess;
its child-side drop does not unlock the parent's guard.

[Wire v1](MDC-REPLAY-RUN-WIRE-v1.md) specifies every field, digest domain and cap.
[CLI/API reference](../adapters/mdc_replay/README.md) documents create, step,
finish, status and export. Exported copies remain ordinary output files;
HEAD/JOURNAL are authoritative. Source availability remains MODELED.

## Acceptance coverage

The proposal's RR01…RR16 map to these executable checks. Negative-test success
means the expected rejection/fencing result, not a semantic Pass.

| Cases | Production checks / tests |
|---|---|
| RR01 / RR02 | Continuous writer vs restart at each prefix; full disk/report/snapshot equality; native CLI scenario uses 23 processes |
| RR03 | Committed retry and discarded/lost reply, unchanged HEAD/JOURNAL; SIGKILL before reply resolves by reopen |
| RR04 | Different source, lag, plan and limits rejected before tail repair; future suffix creates a new audit run |
| RR05 | Restored third cutoff excludes second-bar availability; full result equals unchanged 014 late-witness control |
| RR06 | New raw-input namespace for duplicate/suffix while earlier decision ID/core bytes remain identical; prior permutation tests retained |
| RR07 | Second-process lock rejection while idle; paused child locks checked during create/commit/tail recovery; release after drop/SIGKILL |
| RR08 | 13 create SIGKILL points plus injected I/O errors: complete empty run or explicit incomplete error, no implicit overwrite |
| RR09 | Nine append/publication/reply SIGKILL points and error injection; only certified old/new contiguous prefix |
| RR10 | Valid, partial and corrupt extra tail discarded; two tail-repair SIGKILL points; failed repair never advances cursor |
| RR11 | Corrupt/truncated HEAD, manifest, source and committed journal fail without repair/rollback; missing LOCK/HEAD not recreated |
| RR12 | Forged report/snapshot/index rejected despite recomputed outer record/HEAD checksums |
| RR13 | Independent codec vectors; unknown versions/tags, truncation, bad length/limit bombs, EOF and plan bounds |
| RR14 | Pre-I/O budget errors preserve usable prefix; post-mutation/recovery errors never expose a successful new prefix |
| RR15 | Completed lookup/retry, skipped/out-of-range rejection without append |
| RR16 | Frozen 014 report/snapshot and prior core/WAL controls unchanged; pure codec tests run on cross targets |

## Verification

Local receipts and manifest are in
[impl-015-local-verification.json](evidence/impl-015-local-verification.json) and
[impl-015-source-manifest.json](evidence/impl-015-source-manifest.json).
Hosted execution is bound separately to the exact PR head; workflow configuration
or local tests alone are not reported as hosted PASS.

The CI matrix has 16 jobs. Each native replay job runs 48 default Rust tests,
50 fault-injection Rust tests, nine Python validator tests, Format/Clippy in both
feature modes and a production build without fault hooks. The process-termination
suite covers 24 points (13 create + 9 commit + 2 tail repair). Pure wire tests add
seven to the prior cross suite, totaling 50 per cross configuration. Two native
recovery results are compared using full files and exported bundles, not just logs.

Prior root verification stays 213 Rust + 25 Python per configuration, six cells;
Linux core Store stays 36 default + 38 fault tests per toolchain. Scalar E2E still
executes 23 mandatory cases twice. Prior portable/core snapshot controls agree
across ten configurations; existing modeled replay bytes agree across six.

## Limits and next work

Default source/plan/visible/journal caps are deliberately bounded. The complete
source is retained, and history contains full snapshots per decision. Recovery
recomputes that bounded history and can be expensive; no throughput, hard allocator
or wall-clock SLO is claimed. A source may parse successfully but exceed derivation
or aggregate journal capacity. Limit errors preserve the last certified prefix.

Filesystem/device sync assumptions still apply. SIGKILL checks process termination,
not physical power loss. This is not authenticated or rollback-resistant storage
against an actor replacing valid files. No compaction/rotation, plan extension,
buffered decision ACK, multiwriter service, measured receipt, market strategy or
order transaction is implemented. The source-run cursor is a count of completed
cutoffs, distinct from a broker or market-stream cursor.

Next work should measure resource use on the intended dataset and define retention
or compaction before scaling this exact-history profile. Market State/Transition
semantics and any trading execution require their own agreed evidence/acceptance
contracts; they are not inferred from technical confirmation.
