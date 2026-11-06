# MDC-REPLAY-EXACT-WORK-001 — Proposed work reduction v0.1

2026-11-06. Proposed MDC-IMPL-017 after verified MDC-IMPL-016.
Historical proposal. Implemented in [MDC-IMPL-017](MDC-IMPL-017.md);
[current contract](MDC-REPLAY-EXACT-WORK-CONTRACT-v0.1.md).
The original pre-implementation scope follows.

## Selected scope

Reduce repeated preparation of the same immutable decision and captured source.
Retain the existing core, replay v1 wire format, all certified history, exact
restoration/recomputation and all default limits. The first delivery is a bounded
adapter refactor, with measured outcomes and a comparison against reviewed 016.
It is not a larger-capacity profile or a promise of a full-day backtest.

016 measured the current representation: on the pinned 2024 day, all three
256-entry requests on each compiler retained 153 decisions before the 64 MiB
journal limit. All 128-entry requests completed. A 1440-entry plan is rejected
before creation. Faster computation cannot change these unchanged logical file
sizes or plan limits; those remain separate design obligations.

## Findings and candidate operations

The following are static call-path observations, not CPU attribution or a claim
that these calls dominate the measured recovery time:

1. `DecisionBundle::build` calls `DecisionView::report_json`, whose
   `snapshot_sha256` exports the session, then separately calls `snapshot_bytes`
   for the bundle. The same immutable view is exported twice.
2. `RunStore::open` decodes MANIFEST and obtains a validated Dataset, then calls
   public `validate_prefix`, which decodes the same captured manifest and parses
   the same captured SOURCE again. Both paths use the production normalizer.
3. `SnapshotSession::restore` checks state bounds after every operation and the
   final state at the end. This work contributes to the static recovery path,
   but those checks enforce observable bounds. Changing them is **outside 017**.

The implementation may reuse one captured snapshot to format its report and
construct its DecisionBundle, and may share an already validated immutable
manifest/Dataset between the private open and prefix-validation paths. Apply
these changes separately so that reduced work and any performance effect can be
attributed. A timing change within noise must be reported as such.

## Invariants and API boundary

- Snapshot/report capture remains fallible and owns its resulting bytes. The
  report's core hash is computed from the exact snapshot carried by the bundle.
  No public API accepts an unchecked caller-supplied hash/snapshot as authority.
- Existing `report_json`, `snapshot_bytes`, `snapshot_sha256`, public
  `validate_prefix`, RunStore operations and their error behavior remain
  compatible. Prefer private helpers; any additive public API needs explicit
  documentation and ordinary correctness tests.
- Public prefix validation still completely checks untrusted raw arguments.
  Internal reuse is permitted only for values derived from the same captured
  immutable SOURCE/MANIFEST/profile/limits, in this operation. It is not a
  caller assertion that data were validated elsewhere.
- No cross-cutoff, cross-source, cross-profile or cross-run semantic cache is
  introduced. Every isolated decision still derives eligibility through
  production Dataset APIs and rebuilds its own recorded core session.
- Every certified saved bundle is still compared against recomputation, with
  exact report/snapshot bytes. Snapshot restoration and per-operation outcome,
  state and resource-bound checks remain in place. Hash validity alone is not
  semantic validation.
- Missing/corrupt references, forged reports/snapshots, mismatched manifests and
  unsupported versions continue to fail closed. Preserve first observable
  validation/limit error precedence on existing public paths.
- SOURCE, MANIFEST, JOURNAL and HEAD bytes, run/decision identities, cursors,
  lineage, statuses, limit points and retry behavior remain exact for identical
  requests. All old snapshots/runs remain readable without migration.
- Validation precedes repair. Sync/rename/directory-sync order, one-owner lock,
  process ownership, fencing, tail handling and durable acknowledgement do not
  change. No old-HEAD fallback or new meaning of a successful reply is added.
- Root no_std APIs, dependencies/lockfiles, original nested copy, mdc_store,
  Binance normalization/time model and all default budgets remain unchanged.

## Evidence and execution plan

First establish a fresh baseline at merged 016 main. Record the exact two source
revisions/fingerprints, binary hashes, release flags and host information; preserve
input and plan identities. Old local timing numbers are context, not a paired
performance baseline for a different host or binary.

Use the 016 runner for two correctness-equivalent variants, with at least three
saved repetitions per point/toolchain. Include the frozen before/equal/after
availability and empty-prefix controls, the reduced-journal probe and a nonempty
recovered prefix. Keep all samples, first constraints and individual measurements.

For the official release series, retain the pinned 2024 source and existing
1,16,32,64,128,256,1440 plan counts. Execute before/after builds sequentially on the
same host, interleaving/reversing variant order where feasible; explicitly label
cache state and order. Compare API/parent wall, user/system CPU, peak RSS, logical
bytes and mechanical call counts separately. Counters used to prove eliminated
calls must observe the real operation path and stay outside persisted/canonical
data. Tests must not substitute their own normalization, State or replay engine.

A baseline report binds its reviewed baseline fingerprint; a candidate report
binds its candidate fingerprint. Validate each against its own reviewed source
revision, then compare exact retained files. Do not re-label the old fingerprint,
turn off fingerprint checks or use a current-only validator to declare stale
baseline evidence current. Artifact comparison must reject unknown or mixed
revision/configuration pairs.

No fixed percentage improvement or latency SLO is assumed. Acceptance requires
fewer redundant expensive operations, exact correctness and an honest measured
before/after result, including unchanged or worse timing. If resource use regresses
materially, record it and narrow/revise the implementation before presenting a
performance claim. Do not suppress failed or timed-out samples.

## Acceptance cases

| Case | Required result |
|---|---|
| EW01 | Reviewed baseline/candidate revisions, source/CI/binary/input/model/plan/limits and host are independently bound |
| EW02 | Actual bundle construction captures one owned snapshot and reuses it for report/hash/bundle; existing report bytes remain exact |
| EW03 | Open reuses only its own validated captured manifest/Dataset; public raw prefix validation remains complete |
| EW04 | Empty, before/equal/after cutoff, duplicate-row, ms/us and modeled lag controls retain exact outcomes |
| EW05 | Old 015/016 runs open, retry and continue; unchanged requests produce identical full retained files |
| EW06 | Forged snapshot/report/outcomes and source/manifest/run/version mismatches are rejected, including re-framed data |
| EW07 | Public error/limit precedence, record/report/snapshot bounds and per-operation restore bounds remain enforced |
| EW08 | Commit failure, lost response, partial tail and recovery interruption retain old fencing/repair/sync behavior; all existing kill points pass |
| EW09 | Caller cannot substitute a cached future or different-profile decision, snapshot or hash |
| EW10 | Reduced-journal and real default limit preserve the same certified prefix, first failing index and unchanged HEAD/journal on rejection |
| EW11 | Baseline/candidate samples retain every outcome; byte comparison is independent of noisy timings and requires both source bindings |
| EW12 | Both native Rust versions × three core profiles, native/fault replay/store tests, cross-target controls and CI validators pass |
| EW13 | Source review confirms unchanged persisted formats, dependencies, root/store/nested implementations and default limits |
| EW14 | Documentation/matrix distinguish implemented work reduction from unchanged daily capacity and unverified production SLOs |

## Explicit later work

Compression/deduplication, larger limits, rolling windows, shared mutable graphs,
checkpoint trust, skipped restoration or deferred semantic checks are separate
contracts. 017 does not silently choose one to fit 1440 decisions.

For a later daily-capacity profile, record the target history/decision plan,
recovery-time and memory/storage budgets, and the requirement for immediate exact
access to old views. Existing preservation of history remains the default; a
lossy retention policy is not inferred from a benchmark. No new product decision
is required to prepare the bounded work-reduction refactor above.
