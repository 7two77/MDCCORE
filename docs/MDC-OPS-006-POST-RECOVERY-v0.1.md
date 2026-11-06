# MDC-OPS-006 — Verified recovery main and capacity preparation

2026-11-06. Owner merged PR #10. Main
`ee4d45f263b476d7ba62df31405edb23462bd6c0` has parents
`9f21fe7a81c393dc5f9d3cffc96d3d0871ec48da` and reviewed 015
`a783952751ec238c4f2df3d97817e1a78de3b57f`.
Tree `3e14c4a3fd6614dde312069de573b8c1bf3c206f` matches the reviewed tree.
History and current scalar evidence checks pass; neither original root was rewritten.

[Post-merge CI](https://github.com/7two77/MDCCORE/actions/runs/42234145381):
**16/16 PASS**. Actual logs confirm core 213 Rust + 25 Python per native profile
(six cells), replay 48 default + 50 fault Rust and 9 Python per toolchain, cross
50 Rust per cell (four cells), and old Linux store 36 default + 38 fault per
toolchain. All sixteen artifact sets were uploaded. Portable/snapshot controls
agree across ten configurations, 014 replay across six, and recovery files across
two native configurations. The comparison ZIP digest was checked against GitHub;
all four comparison receipts match this main, fingerprint and prior controls.
[Receipt](evidence/post-merge-main-ee4d45f.json).

## Initial capacity observation

Runtime and limits are unchanged. A Rust 1.98.0 release build used the official
BTCUSDT Spot 1m CSV for 2024-01-01 (1440 rows). Source SHA and commands are in the
[pilot receipt](evidence/capacity-pilot-001.json). Its standalone control matched
the frozen 014 report/core bytes before measurement.

| Requested cutoffs | Observed result | Journal bytes | Reopen wall time |
|---|---|---:|---:|
| 64 | All committed and verified | 12,042,693 | 1.65 s |
| 256 | Journal limit before decision 154; 153 committed and verified | 67,091,458 | 23.74 s |
| 1440 | CLI cutoff argument rejected before directory creation | — | — |

64 MiB is 67,108,864 bytes. The second run's retained prefix successfully reopened;
no budget was raised and no certified history was discarded. 153 is specific to
this dataset/plan, not a universal bar limit. A daily plan also exceeds the current
256-entry/visible-history bounds; changing CLI argument transport alone is insufficient.

These are one-host, one-sample observations on an overlay filesystem. Wall time
includes up to 50 ms parent polling latency. Reopen uses a fresh process with
uncontrolled/warm OS cache; RSS includes fork/exec baseline. There is no p95/p99,
cold-start RTO, throughput SLO or device durability claim.

Next: [MDC-CAPACITY-001](MDC-REPLAY-CAPACITY-CONTRACT-v0.1-DRAFT.md), a reproducible
measurement harness before choosing optimization, retention or a larger profile.
This checkpoint changes documents only. The formal harness, compaction and limit
changes are not implemented here.
