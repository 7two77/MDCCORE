# MDC-REPLAY-RECOVERY-001 — Durable replay decisions, draft v0.1

2026-11-04. Preparation for MDC-IMPL-015, based on merged MDC-IMPL-014.
Status: HISTORICAL PROPOSAL, realized by MDC-IMPL-015 and the
[frozen wire contract v1](MDC-REPLAY-RUN-WIRE-v1.md). Future-tense sections below
preserve the preparation checkpoint; current execution/limits are in
[the implementation report](MDC-IMPL-015.md).
Scope: Binance Spot modeled final-bar profile, bounded input, Linux local
filesystem, one writer. The owner has selected historical replay, exact recovery
and local snapshot + journal storage. This document specifies the next bounded
consumer transaction; it does not change MDC-SNAPSHOT-001/1 or MDC-WAL-001/1.

## Recovery unit and isolation

An 014 decision creates a fresh SnapshotSession, whose Registry includes its own
cutoff. Its core snapshot is one independent view, not the next state of a shared
market graph. mdc_store currently creates an empty session with a frozen Registry;
it cannot import this populated view or atomically retain an external cursor.
Copying CSV offsets alongside an existing core Store would not create a transaction.

The recovery unit will therefore be a **completed decision bundle**. A separate
host replay journal retains the report and exact recorded core snapshot for each
cutoff. One atomic run HEAD selects a contiguous prefix of these bundles and the
next decision index. This preserves the 014 isolation boundary. No future input
is inserted into an earlier decision's session. The existing core Store and its
automatic flush scheduler remain available for their own profile, unchanged.

This first replay transaction is strict at every completed decision. There is no
AcceptedVolatile decision receipt, periodic flush promise or hard latency SLA.
Buffering raw inputs or decisions may be a subsequent explicit extension.

## Immutable run input and identity

Create binds and retains these values before publishing an empty committed run:

| Field | Meaning |
|---|---|
| Run format / consumer version | New replay-run format v1; existing BINANCE-SPOT-1M-MODELED-001 semantics |
| Source | Exact copied CSV bytes, byte length and SHA-256, bounded by 014 input limits |
| Model | Symbol, declared ms/us unit, lag anchor and lag; availability quality MODELED |
| Plan | Explicit nonempty ordered array of unique strictly increasing UTC-us decision cutoffs |
| Identity / core codec | PortableV1 and recorded snapshot engine/format 1 |
| Bounds | Fixed run limits, core import limits and adapter input/visible limits |

No source pathname, file mtime or current wall clock is the source identity.
Reading/copying is bounded, parsed as one captured byte buffer, and the exact
captured bytes are stored; concurrent changes to the original path cannot replace
the copied input after capture. As in 014, parsing is not a provider authenticity
attestation. The retained source, manifest and plan must be synced before HEAD.

Run ID is a domain-separated SHA-256 of the canonical immutable manifest, including
source content hash and explicit plan. It is an audit namespace, never an input
to 014 evidence IDs, decision IDs, Registry context or core canonical encoding.
The same run identity can exist in two directories; directory locking is not a
global execution or trading-order deduplication service.

Opening an existing run uses its retained source, manifest and plan. If the caller
supplies expected source/model/plan, require exact agreement; changed CSV bytes,
extra suffix, reordered rows, model or cutoffs require a new run. Semantic replay
permutation invariance still holds across separately created runs. An unavailable
suffix may change run identity without changing an earlier 014 decision ID or
core snapshot. Neither restart nor a data change repairs historical bundles.

The cursor is **the number of committed plan entries**, not a CSV byte/row offset.
This avoids confusing physical file order/duplicates with normalized eligibility.
At prefix k, entries 1…k are durable and the next index is k+1; at k=plan length
the run is complete. Cutoffs are replayed from the retained input with the unchanged
014 eligibility rule, including receipt-before-decision equality at modeled ties.

## Journal and sole commit authority

Proposed files are LOCK, immutable SOURCE and MANIFEST, an append-only JOURNAL,
and an atomically replaced HEAD. Temporary files are never recovery authorities.
The implementation must freeze explicit binary framing and independent vectors
before the first writer is added. Framing uses versioned domains, SHA-256 integrity,
checked LE length fields, bounds and exact EOF; native Rust Hash, Debug, platform
integers and implicit serde defaults are forbidden in persisted identity.

Required journal header fields: format/engine/profile versions and exact manifest
hash/run ID. Each decision record binds run ID, contiguous plan index, exact
cutoff, decision ID, complete report bytes, full 011 core snapshot bytes and the
previous record/header hash. SHA-256 binds both blobs, framing and record order.
The report must contain the declared source audit hash, model and canonical
evidence; it is not reconstructed by accepting a stored success flag.

HEAD binds format, run/manifest identity, committed journal byte offset, durable
decision count, tip hash and latest bundle digest. The initial HEAD certifies an
empty prefix and the journal header. A valid-looking tail past its certified
offset is uncommitted even if it contains complete checksummed records. No mtime,
longest file, last complete frame or temporary HEAD can advance the cursor.

One commit must:

1. Verify expected index and resource budgets; compute the next isolated view
   using production 014/core APIs without mutating prior snapshots.
2. Encode the full report and snapshot, validate their internal binding, and
   append one chained record. The old HEAD still selects the old prefix.
3. Sync the journal file. Publish and sync a temporary HEAD, rename it over HEAD
   in the same directory, then sync that directory.
4. Only after successful publication return DurableDecision(index, decision_id,
   snapshot_hash, durable_through=index). The receipt certifies the source/plan
   and every prior completed decision, not only the last output JSON.

Create must sync the parent directory and all retained input/header files before
publishing its initial HEAD. Exclusive writer lock covers create/open, validation,
tail repair, append and publication. No overwrite of an existing run directory
or silent recreation after missing/corrupt HEAD is allowed. Partial create is
reported as incomplete/indeterminate; a new path or explicit operator action is
required. Sync guarantees remain conditional on the Linux filesystem/device stack.

## Failure, retry and recovery

Pre-I/O invalid arguments, parse errors and budget rejection leave the previous
committed prefix unchanged. An error after a journal/publication mutation fences
the writer and returns an indeterminate result: no further writes or durable ACK
until close/reopen. If rename succeeded but directory sync failed, callers must
not assume that the old HEAD remains authoritative after a restart.

The request key is (run ID, one-based plan index); its meaning is fixed by the
immutable plan. Retrying an already committed index returns its retained exact
receipt without another record or cursor advance. Skipped/future/out-of-range
indices are errors. A reply lost after successful commit can be resolved by reopen
and lookup. A deterministic core or I/O failure never synthesizes a successful
decision, zero value or cursor advancement.

Recovery holds the writer lock, validates HEAD, manifest/source hashes, versions
and budgets, and verifies exactly the certified journal prefix. It requires
contiguous indices matching the plan, exact lengths and hashes, report/source/model
agreement and no missing committed data. For each retained bundle, it restores
the full SnapshotSession through engine 011 and recomputes the corresponding 014
decision from the retained dataset; report and snapshot must match byte-for-byte.
Checksummed but semantically altered snapshots/reports must therefore fail.

Only after full validation may recovery expose committed results/next index.
Uncommitted suffix is ignored as state and truncated/synced before further writes.
Damage/truncation inside the certified prefix, unknown versions or corrupt/missing
HEAD fail closed; do not silently fall back to an older prefix. A damaged
uncommitted tail is not itself proof that an acknowledged commit was lost.

Technical NO_INPUT, UNRESOLVED and PASS are all legitimate computed decision
outcomes which can be durably retained. Durable success describes persistence,
not a semantic Pass or authorization to trade. Completed prefix lookup is valid
after the plan ends; an extra step does not create an implicit new cutoff.

## Bounds and compatibility

Proposed defaults: 4 MiB captured source, 4096 input rows, 256 visible bars,
256 explicit plan entries, 16 KiB manifest, 24 MiB record body, 64 MiB journal,
1 MiB report, and unchanged default 011 snapshot/import budgets (16 MiB snapshot,
4096 calls). Validate representability and length/count arithmetic before allocation.
The plan's maximum cutoff must respect the selected visible/core-call budgets;
actual per-decision record and aggregate journal limits are enforced before append.
When a later decision would exceed a resource budget, retain the prior prefix
and return a limit error, without truncating history or advancing its cursor.

There is no automatic compaction, checkpoint rotation, concurrent writer, append
to a changed dataset/plan, external order transaction or measured availability
upgrade in this slice. Full recovery cost includes replaying and recomputing a
bounded prefix. Existing scalar/portable/snapshot/WAL and 014 golden bytes must
remain unchanged; the new run format is not passed to mdc_store::open.

## Acceptance plan for MDC-IMPL-015 (not execution evidence)

| Case | Required result |
|---|---|
| RR01 | Create empty prefix, commit technical decisions, recover in fresh processes, compare full report/core bytes |
| RR02 | Stop/restart at each prefix, continue, match uninterrupted run including next index and NO_INPUT/UNRESOLVED outcomes |
| RR03 | Retry a committed index after a lost reply; same receipt, one record, no duplicated core effect |
| RR04 | Wrong source/model/plan or additional CSV suffix rejected as continuation; new run preserves earlier cutoff core bytes |
| RR05 | Future-in-event-horizon but unavailable bar remains excluded after restart; no implicit State witness leak |
| RR06 | Exact duplicates/permutations yield separate raw-input run identities while eligible core bytes match; raw-input audit fields may differ |
| RR07 | Second writer rejected during create, recovery, idle and commit; lock released after normal exit or process death |
| RR08 | Fault/SIGKILL during create: complete empty run or explicit incomplete-run error; no fabricated HEAD |
| RR09 | Fault/SIGKILL around append/sync/HEAD-temp/rename/dir-sync/response: only certified old/new contiguous prefix recovered |
| RR10 | Full valid, partial and corrupt uncommitted suffix ignored; repair synced before new append |
| RR11 | Truncated/corrupt committed record, HEAD, manifest or source rejected without silent rollback |
| RR12 | Recomputed report/core mismatch rejected even when outer checksums are recomputed |
| RR13 | Unsupported versions, trailing framed data, bad lengths, arithmetic overflow and bounds rejected |
| RR14 | Parse/preflight/limit errors leave old committed prefix unchanged; post-mutation I/O errors fence until reopen |
| RR15 | Completed plan lookup/retry succeeds; skipped/out-of-range index never creates an extra decision |
| RR16 | Existing 014 output and scalar/snapshot/WAL controls unchanged; portable decoder vectors and native Linux recovery tests |

Process termination tests do not prove physical power-cut tolerance. Acceptance
requires actual execution receipts; these rows do not establish a runtime PASS.

## Implementation order

Freeze framing vectors and limits; add strict single-owner create/open/commit and
read-only retained-result access; implement prefix recovery and semantic checks;
then add fault-injection/fresh-process tests and CI artifacts. Reuse existing
production 014/core APIs. Do not place trading calculations or a replacement
State algorithm in the journal, fixture or Python validators.
