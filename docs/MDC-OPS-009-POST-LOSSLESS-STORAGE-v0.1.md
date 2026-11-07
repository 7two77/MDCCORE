# MDC-OPS-009 — Verified lossless-storage main and next profile boundary

2026-11-07. Owner merged PR #13. Main
`5dfd97da6e6bae0b5d45b14dc040396d67d2885e` has ordered parents
`ac364f88d8e696abf2d865524b537408418e2450` and reviewed 018
`a9da07b97040e835c08294bb62f39b720f1e5f73`. Its complete tree
`107a7cd7f0f3fa56e7fbd2c1d2d99e7fc0d5985c` matches the reviewed tip exactly.
The original roots and implementation milestones remain ancestors. History and
current scalar-evidence checkers pass; the cloud checkout was fast-forwarded to
this tree and its worktree/index are clean. Codespaces was not modified by this
checkpoint.

The implementation fingerprint is
`9bac573e13f1ee7703cce2e91d94ff022bad3a5de026a199fe1ca5368c2a3e19`.
[Post-merge CI](https://github.com/7two77/MDCCORE/actions/runs/42358309266)
passed **16/16 jobs** on this main commit. Actual logs confirm:

| Scope | Observed tests per configuration |
|---|---:|
| Root, two Rust versions × three feature profiles | 213 Rust + 25 Python |
| Replay host, each Rust version | 88 normal + 93 fault-injection Rust; 78 Python |
| Linux store, each Rust version | 36 normal + 38 fault-injection Rust |
| Cross, two Rust versions × i686/s390x | 56 Rust |

All sixteen ZIPs were downloaded; their full SHA-256 and ZIP CRC checks passed.
Fourteen clean metadata sets bind the actual main commit, source files and CI
configuration. All seven comparison receipts pass and preserve frozen controls.
The same-length, resealed malformed-zlib regression passed in four native test
executions, covering normal/fault-injection modes on Rust 1.85.1 and 1.98.0.
[Complete source-bound receipt](evidence/post-merge-main-5dfd97d.json).

The earlier official 48-variant resource study remains bound to `5888df2`.
It observed 256 retained v2 decisions, a 33,290,447-byte encoded journal and
186,361,156 decoded blob bytes. Those timings and resource samples are preserved
with their original source/host identity; this checkpoint does not re-label them
as a new main benchmark. V1/v2 defaults and current 1440-entry rejection remain.

## Next candidate: cumulative full-day replay

The [019 draft](MDC-REPLAY-FULL-DAY-CONTRACT-v0.1-DRAFT.md) proposes 1440 isolated,
cumulative decisions with every exact report/snapshot retained. It preserves the
existing temporal model and makes the larger operating profile explicit.

Changing only the plan count is insufficient. Current guards include:

- 256 plan entries and 256 visible bars;
- 4096 core operations in capture, export and run recovery;
- separate per-view, physical-journal and aggregate decoded limits;
- benchmark phase/output budgets and the cost of full semantic restoration.

For `n > 0` visible bars, the production builder uses `11*n + 1` recorded core
operations: 2817 at 256 bars, **15841 at 1440**. This is a source-derived operation
count, not a successful 1440-bar execution or a performance extrapolation.

Runtime work starts after the operating envelope is explicit: RAM, retained disk,
full-prefix recovery target and reference host, or a separately labelled
engineering experiment. These values are pending the owner's response. This
checkpoint and draft change documentation only; current caps/codecs remain intact.
