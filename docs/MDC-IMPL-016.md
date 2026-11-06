# MDC-IMPL-016 — Reproducible replay resource characterization

2026-11-06. Extends reviewed main `ee4d45f` in PR #11. Production core, store,
replay library, manifests/lockfiles and original nested copy remain unchanged.

## Change

A separate Linux Rust driver calls the existing normalization/RunStore APIs, with
phase timings outside canonical bytes. The Python runner retains copied release
binaries, source, logs, complete runs, per-process CPU/RSS, plans and file lengths.
The validator independently reads existing v1 framing, validates the complete
matrix and compares full retained files. No Serde or new Rust dependency is added.
[Contract](MDC-REPLAY-CAPACITY-CONTRACT-v0.1.md),
[commands and interpretation](../adapters/mdc_replay/README.md#resource-characterization-mdc-impl-016).

Regular CI adds default and explicit smaller-journal smoke to the two existing
native replay jobs and compares all four configurations. A separate manual
workflow supports larger studies; it does not run on pull requests. CI metadata
now fingerprints every workflow and CI script, including the optional workflow.

## Verification mapping

| Case | Executable check / artifact |
|---|---|
| CP01 | run_capacity build/source/host metadata; actual binary/source/log digests checked by capacity_report.validate |
| CP02 | Release frozen 014 report/core control in every study |
| CP03 | Unique new per-cell directories; production plan/source/run binding and zero initial cursor |
| CP04 | Separate commit/recover child receipts, Rust API times and parent wait4 data |
| CP05 | Actual SOURCE/MANIFEST/HEAD/JOURNAL byte equality across repetitions/toolchains; fresh recovery recomputes views |
| CP06 | Real reduced-budget smoke and default-budget full series; before/after HEAD and journal length plus reopen/retry |
| CP07 | 1440 normalized cutoffs rejected by manifest without creating a run |
| CP08 | Rust lost-reply fault test with recovery; Python owned-process timeout test; timeout cannot earn COMPLETE acceptance |
| CP09 | Integer clocks/CPU/RSS units validated against raw receipts and actual logical files |
| CP10 | Negative validator tests for missing/duplicate/stale/tampered/escaped/reused artifacts |
| CP11 | Summary rebuilt from every row; no suppressed failures or invented percentiles/SLO |
| CP12 | Core/store/replay regression matrices and unchanged frozen controls/source checks |

Local verification and resource series are recorded in
[evidence](evidence/impl-016-local-verification.json). Hosted CI is separately
reported in the pull request; a local test is not hosted execution evidence.
Full retained local datasets/runs are external study artifacts, not Git fixtures.
The checked-in study summary retains identities and all timing/resource samples;
revalidation of full framing requires the raw run artifacts as documented.

## Boundaries

This package measures existing complexity and first constraints. It does not add
compaction, shared snapshots, a rolling window, history pruning, relaxed recovery,
new durability acknowledgement or larger production limits. The daily plan remains
unsupported by v1. Input availability is MODELED. Fresh process measurements use
uncontrolled cache state; sync depends on the filesystem/device. RSS includes
fork/exec baseline and is not allocator accounting. A measurement acceptance is
not a successful full-day replay, a trading backtest, a cold-start RTO or an SLO.

The optional large-study workflow requires an explicit dispatch after it is
available on main; pre-merge local execution is labeled separately. Normal CI
covers its shared runner/validator with small retained matrices.

## Recorded local series

Official BTCUSDT Spot 1m, 2024-01-01; 1440 captured rows, ms, modeled 100 ms close lag.
Two toolchains on the same x86_64 Linux overlay host, sequential studies, three fresh runs per point.
The working tree was marked dirty during local execution; exact source/binary/CI digests are retained.
These data are local evidence, not hosted benchmarks or a causal compiler speed comparison.

| Requested | Committed | Journal bytes | Median recovery API seconds (1.85.1 / 1.98.0) |
|---:|---:|---:|---:|
| 1 | 1 | 11078 | 0.009 / 0.008 |
| 16 | 16 | 850426 | 0.062 / 0.055 |
| 32 | 32 | 3140982 | 0.308 / 0.270 |
| 64 | 64 | 12042693 | 1.911 / 1.672 |
| 128 | 128 | 47127775 | 13.395 / 11.719 |
| 256 | 153 | 67091458 | 22.233 / 20.400 |
| 1440 | 0 | — | — / — |

All 42 observations validate. Requests through 128 complete. Each 256-entry plan
stops before decision 154 and recovers all 153 certified bundles; 1440 is rejected
before run creation. Full SOURCE/MANIFEST/HEAD/JOURNAL bytes agree across all
repetitions and both toolchains. No budget or retained history changed.
The measured growth motivates a later optimization/profile decision; it does not
establish a universal 153-bar capacity or a full-day performance result.

[All samples and identities](evidence/impl-016-capacity-study.json).

The external raw-study archive contains 512 stream-verified files (136,886,222 bytes),
SHA-256 `63746713db6e633df103e1caa56ba2255b8b3e642302dfbde7ad84723a404d3c`.
It is supplied separately; full journals and binary artifacts are not Git fixtures.
