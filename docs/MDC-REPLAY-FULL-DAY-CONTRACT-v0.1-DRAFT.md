# MDC-REPLAY-FULL-DAY-001 — Proposed cumulative replay profile v0.1

2026-11-07. Candidate MDC-IMPL-019 after merged 018 main `5dfd97d`.
Status: **DRAFT / RESOURCE DECISION PENDING / RUNTIME NOT IMPLEMENTED**.
This proposal extends the declared operating envelope. It is not a production
trading SLO or an edit to the existing v1/v2 profile definitions.

## Proposed behavior and retained decisions

Use the pinned official BTCUSDT Spot 1m day 2024-01-01, source CSV SHA-256
`eb9a1b452184ea664e86e87f9cba783639163578d0bed9ff2073d25d897dbe1b`,
plus a synthetic day for reproducible smoke tests. Continue explicit timestamp
units, exact e8 values, MODELED 100 ms close lag and production cutoff derivation.
The existing 2025/us controls remain mandatory.

Build **1440 isolated decisions**, one at each normalized bar availability time.
Decision `k` uses the complete eligible prefix through its cutoff, including all
previous bars of this captured day. Retain the original report plus LF and exact
engine-011 snapshot for every decision. A separate graph per cutoff keeps future
witnesses out of earlier decisions. Market algorithms remain production core API
calls; fixtures/validators implement no replacement State/Measurement logic.

This proposal preserves cumulative history. A different lookback policy would
need its own explicit semantic contract and identity/compatibility decision.

The full-day operating profile is selected explicitly. Legacy RunStore/RunStoreV2,
their CLI defaults, codec choices, admission rules and caps retain their current
behavior, including a 1440-entry rejection. A default core SnapshotLimits remains
4096 operations. Larger caller-selected bounds belong to the new profile.

## Observed boundaries before implementation

| Boundary | Current implementation | Consequence for this proposal |
|---|---|---|
| Plan | v1/v2 ceiling 256 | New explicitly identified operating envelope |
| Eligible set | replay-run adapter max_visible_bars 256 | Thread selected visibility bounds through production normalization/capture |
| Recorded operations | `11*n+1` for `n>0`, zero for empty | 1440 bars need 15841 calls, above 4096 |
| Capture/export | `core_view::capture` and `export_session` use default snapshot bounds | Provide coherent selected limits for build and every export API |
| Recovery | `RunLimits::core` and source preflight cap calls at 4096 | Restore with the same authenticated profile descriptor |
| v2 storage | 64 MiB encoded journal / 256 MiB aggregate decoded blobs | Independently size and validate a new retained-history envelope |
| Per view | report 1 MiB / snapshot 16 MiB / record 24 MiB plus core sublimits | Measure each actual boundary; neither success nor failure is assumed |
| Study | existing output/phase bounds | Explicit measurement budgets; retain incomplete runs and first constraints |

For a successfully constructed view, the operation formula follows production construction: five observations
and five scalar measurements per bar, `n-1` close differences, one composition and
one State. At 372 bars it yields 4093 calls; at 373, 4104. Current replay-run
visibility guards reject these sizes earlier, so these counts do not assert that
the legacy consumer accepts 372 bars. Across 1440 cumulative views the formula
sums to 11414160 build calls, before restore/recomputation; it is not a CPU estimate.

Root restore checks each historical outcome and bounds the retained state after
every operation, then compares the final state manifest. This validation stays
mandatory. Streaming outer frames alone does not remove that work. Changing the
validation strategy would require a separate equivalence contract and tests.

## Required operating decisions

The question sent to the owner asks for RAM, retained disk per run and maximum
full-prefix recovery time (RTO) on a reference host. No numeric answer is recorded
yet. The owner can instead select an explicitly experimental engineering profile.

| Decision | Current status | Required before runtime work |
|---|---|---|
| Cumulative prefix and all exact views | Selected proposal, consistent with current semantics/retention | Confirm in the adopted full-day contract |
| Production target or engineering experiment | PENDING | Label acceptance and measurement claims accordingly |
| Peak process RAM / measurement scope | PENDING | Specify host, process set, unit and measurement method |
| Encoded retained disk / decoded logical history | PENDING | Separate ceilings, storage allocation versus file-length meaning |
| Full committed-prefix recovery time | PENDING | Specify target host/load/cache conditions and treatment of timeouts |
| Source/per-view/core limits and full-day audit descriptor | PENDING | Freeze effective bounds, encoding and allowed ceiling validation |

Logical codec limits do not establish a hard RSS bound. A measured recovery sample
does not establish a cold-start or loaded-service RTO. Every chosen budget must
be present in diagnostics and the immutable run profile. A resource claim needs
its defined measurement scope and all retained samples.

## Proposed format and compatibility obligations

Give the full-day run its own explicit storage/profile namespace. Current v1/v2
readers continue to enforce their original ceilings. A separate full-day API/CLI
must distinguish its files and reject unknown profile versions before body
allocation. The exact wire layout/domain and independent vectors must be frozen
before implementation; no new deployed format is declared by this draft.

Reuse the bounded raw/zlib blob semantics from 018, including pinned writer
profile, strict single-stream EOF, decoded SHA-256 and independent encoded,
decoded, per-record and aggregate checks. An owned read authenticates its actual
frame length and physical digest against its certified index before blob parsing
or decoding, as fixed in 018 review 001.

Bind the complete effective source/replay bounds, all seven SnapshotLimits
fields, storage bounds, source/model/plan and codec descriptor in the immutable
manifest/audit identity. Validate file-declared values against the selected caller
profile ceilings before allocation. File contents cannot authorize larger budgets
than that selected envelope. Ensure per-blob and snapshot bounds are coherent.
Build, export, root restore, fresh recomputation and resumed lookup must use the
same effective limits; no default-bound fallback is allowed on any path.

Operating budgets stay outside unchanged logical decision/context identity.
For the same full source, model and a cutoff supported by v2, a new full-day view
must have identical decision IDs, report bytes, root snapshot bytes and lineage.
Larger explicit bounds need no engine-011 archive-format change: the existing
archive encodes registry, operations/outcomes and state, while limits are supplied
by the caller. This compatibility target still requires actual byte comparison.
Audit identities differ for the new operating envelope/plan.

A new run preserves the original source and plan with strict append/sync/HEAD
publication and directory sealing. Preserve single-owner/fork checks, fencing,
old-index idempotency, no older-HEAD fallback and validation before tail repair.
Keep only a bounded certified index; decoded history remains owned by callers.
Every committed snapshot is restored/recomputed before exposing a recovered run.

## Delivery stages and acceptance

1. Freeze resource mode, reference host, effective ceilings and the full-day wire
   descriptor; check source-derived work counts against real production operations.
2. Thread an explicit profile through normalization/capture/export/store/recovery
   without changing the existing profiles or default core bounds.
3. Execute bounded growth probes at plan/core boundaries and larger prefixes,
   retaining the first failure, full files and exact current-v2 prefix comparison.
4. Execute all 1440 decisions and recover them in fresh processes under the chosen
   envelope; collect every report/snapshot/cursor and all resource samples.
5. Check fault/forgery behavior and compile/target matrices; publish the exact
   implementation/host/profile/measurement bindings and unresolved limits.

| Case | Required result |
|---|---|
| FD01 | Explicit full-day selection; legacy 1440 requests retain their rejection |
| FD02 | All 1440 cutoffs derive from the actual normalizer with declared modeled availability |
| FD03 | Common supported v2 cutoffs match decoded report/snapshot/IDs/lineage in full bytes |
| FD04 | Incomplete/invalid budget descriptors reject before directory creation or unsafe allocation |
| FD05 | Capture, all exports and restore use one consistent selected limit descriptor |
| FD06 | Real operation counts match the production formula; boundaries around 256/257 and 372/373 are tested |
| FD07 | Encoded, decoded, per-view, aggregate, core-item and work limits reject independently without advancing HEAD |
| FD08 | Every 1440-decision run completes and recovers the entire certified prefix before full-day acceptance |
| FD09 | Earliest/middle/latest owned views and old-index retries remain exact after restart |
| FD10 | Wrong source/model/plan/profile fails; changed future data stays out of earlier core views |
| FD11 | Independent framing/budget vectors cover unknown tags, lengths, overflow and cross-target encoding |
| FD12 | Resealed corruption cannot reach owned decoders; full recovery still rejects semantic forgery |
| FD13 | Commit/create/repair/lost-response failures preserve fencing, prefix acknowledgement and old snapshots |
| FD14 | All repetitions, failures, binaries, CPU/RSS/disk/time data and actual first constraints are retained |
| FD15 | Required resource targets pass on the declared host, or experimental observations remain labelled without an SLO claim |
| FD16 | Current root/no_std/v1/v2 controls pass; old files remain readable and continuable by their original API |

Measurement coverage and full-day acceptance are separate results. A complete
measurement can observe a limit, but full-day acceptance requires 1440 committed
and fully recovered decisions under the chosen envelope. A rejected or partial
run cannot earn that acceptance. Do not remove samples or relax the chosen
budget to change its reported result.

The next implementation is blocked on the operating decisions and frozen wire
profile above. This document changes no runtime, default, codec or feature.
Strategy logic, execution, measured historical receipt-time and an unbounded
long-running service remain separate contracts.
