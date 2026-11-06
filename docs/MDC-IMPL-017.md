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
| EW10 | Paired 32768-byte probe; full default-budget official series retains the same 153-decision prefix |
| EW11 | Two fingerprint domains, full files and sample summaries; eleven Python checks for pair evidence and Git-ignored baseline build placement |
| EW12 | Native Rust/profile/core/store/replay and cross-target CI matrices; Format/Clippy/examples/E2E |
| EW13 | Explicit protected-source comparison and unchanged root/store/nested/lockfiles/wire controls |
| EW14 | Current matrix/contract and explicit daily-capacity/SLO boundaries |

[Local verification](evidence/impl-017-local-verification.json) records the native
checks and paired smoke. The official hosted A/B execution below is separately bound to its runtime
revision and input; it is not inferred from static counts or old 016 timings.

## Interpretation

The required correctness claim is exact bytes, references, cursors, statuses,
errors/limits and durability behavior. Fewer pure operations are tested directly;
any resource benefit must be measured. Three grouped repetitions under uncontrolled
caches do not justify p95/p99, a cold-start RTO or a universal percentage speedup.
This package does not make 1440-decision full-history runs fit the unchanged v1
profile. A larger storage/retention representation still needs a separate contract.

## Hosted A/B execution

The [paired official study](https://github.com/7two77/MDCCORE/actions/runs/42255016865)
passed **3/3 jobs**, validating **84 variant samples** on Rust 1.85.1/1.98.0.
Input: pinned official BTCUSDT Spot 1m 2024-01-01 (1440 rows), modeled 100 ms
close lag. Each variant used points 1,16,32,64,128,256,1440 with three saved repetitions.
Baseline is merged 016 `6b33a43`; measured runtime candidate is `3b884a0`.
Evidence-only additions preserve implementation fingerprint
`e6d277fc9ff80465cd401dd55671efbad24ab244194d30d4ad2085d59435931a`.

| Rust | Requested / retained | Median recovery API seconds A → B | Median recovery peak RSS KiB A → B |
|---|---:|---:|---:|
| 1.85.1 | 64 / 64 | 1.984 → 1.868 | 31712 → 29828 |
| 1.85.1 | 128 / 128 | 14.498 → 13.590 | 103580 → 100560 |
| 1.85.1 | 256 / 153 | 24.320 → 22.697 | 144092 → 141044 |
| 1.98.0 | 64 / 64 | 1.804 → 1.773 | 31728 → 29772 |
| 1.98.0 | 128 / 128 | 13.077 → 12.744 | 104432 → 100464 |
| 1.98.0 | 256 / 153 | 21.849 → 21.351 | 143376 → 140504 |

For the 153-decision prefix, observed recovery medians were approximately 6.7%
lower on the 1.85.1 host and 2.3% lower on the 1.98.0 host, with roughly 3 MiB
less median peak RSS. These are descriptive observations under grouped execution
and uncontrolled caches, not universal improvement or a compiler comparison.
One 1.98.0 candidate recovery sample took 22.435 s, above that baseline range
(21.834–21.968 s); all samples and CPU/RSS/clock data are retained, not filtered.

Requests through 128 completed. Every 256-entry plan stopped before 154 with
153 certified decisions; 1440 was rejected before run creation. All full retained
files agree across variants/repetitions/toolchains. The 153-decision journal
remains 67,091,458 bytes. Capacity and durability semantics are unchanged.

The [regular CI](https://github.com/7two77/MDCCORE/actions/runs/42255010433)
passed **16/16 jobs** at runtime revision `3b884a0`; actual logs confirm the
declared counts and all sixteen artifact sets. Six comparison receipts preserve
the frozen controls and validate the paired synthetic probe.

[Full measurement receipt and all samples](evidence/impl-017-hosted-ab.json),
[runtime CI receipt](evidence/impl-017-hosted-ci.json).
The raw ZIPs, binary/log hashes, both source domains and acquisition checks are
bound in those receipts; the full downloaded raw files were also compared locally.
No code, CI configuration, dependencies or limits changed when recording results.
