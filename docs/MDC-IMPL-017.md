# MDC-IMPL-017 — Exact replay work reduction

2026-11-06. Extends merged 016 main `6b33a43`; development remains in PR #12.
[Contract](MDC-REPLAY-EXACT-WORK-CONTRACT-v0.1.md).

## Implementation and API

A private owned capture retains the snapshot already exported during production
view construction. Report/hash/bundle reuse those exact bytes, replacing three
exports with one, without caching snapshots in public DecisionView values.
Private typed manifest/source preparation removes repeat normalization during
open. Public raw prefix validation still checks all inputs independently.

No public Rust API, core/store algorithm, dependency/lockfile, default budget or
persisted format changes. The original nested package is retained. Root snapshot
restore, every per-operation bound check and every bundle recomputation remain
active. The sync/repair/acknowledgement protocol and fault points are unchanged.

`scripts/exact_work.py` validates the pinned baseline using its own frozen source
and validator; candidate evidence must match candidate source. It retains zipped
raw runs, logs and binaries, preserving failed samples on errors. Real old→new CLI
continuation starts at one old committed decision and ends with three, identical
to a new uninterrupted run. CI's reduced-journal probe also preserves first limits.
The manual capacity workflow's `compare_016` option runs paired studies against
its explicitly selected input. No fingerprint check is disabled to compare A/B.

## EW01…EW14 mapping

| Case | Evidence |
|---|---|
| EW01 | Pinned clean baseline source map, candidate source/CI/binary bindings and separate validators |
| EW02 | Actual bundle-path test: original public-method path exports three times; owned capture exports once and keeps all bytes |
| EW03 | Real open/public raw validator counters: one manifest/CSV preparation; every bundle restored |
| EW04 | BTC/ETH × ms/us × before/equal/later cutoff test, legacy report/snapshot equality and frozen 014 control |
| EW05 | Old CLI creates/commits; new CLI retries/continues; exact final run equality and retained old prefix |
| EW06 | Existing forged checksummed report/snapshot/index and malformed wire/source/version tests |
| EW07 | Captured report/snapshot limits reject before restore or journal mutation; expected/source error precedence tests; unchanged root bounds |
| EW08 | Existing 24 SIGKILL points and all fencing, tail and interrupted recovery checks |
| EW09 | Private constructors, immutable inputs, profile/cutoff isolation tests; no unchecked public hash API |
| EW10 | Paired 32768-byte probe; full default-budget official series is reported separately when executed |
| EW11 | Two fingerprint domains, full files and sample summaries; eleven Python checks for pair evidence and Git-ignored baseline build placement |
| EW12 | Native Rust/profile/core/store/replay and cross-target CI matrices; Format/Clippy/examples/E2E |
| EW13 | Explicit protected-source comparison and unchanged root/store/nested/lockfiles/wire controls |
| EW14 | Current matrix/contract and explicit daily-capacity/SLO boundaries |

[Local verification](evidence/impl-017-local-verification.json) records the native
checks and paired smoke. Full official hosted A/B measurements are a separate
execution and are not inferred from static counts or the old 016 timing table.
Their run/artifact identity and results are recorded in the PR after execution.

## Interpretation

The required correctness claim is exact bytes, references, cursors, statuses,
errors/limits and durability behavior. Fewer pure operations are tested directly;
any resource benefit must be measured. Three grouped repetitions under uncontrolled
caches do not justify p95/p99, a cold-start RTO or a universal percentage speedup.
This package does not make 1440-decision full-history runs fit the unchanged v1
profile. A larger storage/retention representation still needs a separate contract.
