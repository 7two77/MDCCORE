# MDC-OPS-007 — Capacity main and exact-work preparation

2026-11-06. Owner merged PR #11. Main
`6b33a4375384ed4301695ea879e641a20f5493b9` has parents
`ee4d45f263b476d7ba62df31405edb23462bd6c0` and reviewed 016
`0c30c79af778742ce0d6a3b4e60b63515d7e4b5e`.
Its tree `c6718a393d1fd830f049767dd7e9933562654938` exactly matches the reviewed
tree. Both original roots and the implementation milestones remain ancestors;
current history and scalar evidence checks pass.

The implementation fingerprint remains
`cca5b3a70dea13130f8fc25f91152b5fd4dfecf71fabd731eb1cd0f11ae20de4`.
The following proposal changes root documents only, leaving that fingerprint,
all algorithms, dependency/lockfiles, wire formats and default limits unchanged.

## Verification

[Post-merge main CI](https://github.com/7two77/MDCCORE/actions/runs/42247801358)
passed **16/16 jobs**, with all sixteen artifact sets uploaded. Actual logs
confirm core 213 Rust + 25 Python in each of six native cells, replay 51 default
+ 54 fault Rust + 44 Python per compiler, cross-target 50 Rust in each of four
cells, and store 36 default + 38 fault Rust per compiler. The downloaded
comparison ZIP digest matches GitHub. All five comparison receipts bind this
main/source and preserve the frozen controls; the capacity matrix verifies
48 synthetic observations. [Receipt](evidence/post-merge-main-6b33a43.json).
The receipt distinguishes completed execution from assertions about the next
runtime package; it does not turn a contract draft into implementation evidence.

The opt-in workflow was also explicitly dispatched on this main with synthetic
input, points 1,2,3,1440 and three repetitions. The
[dispatch](https://github.com/7two77/MDCCORE/actions/runs/42248409893) passed
**3/3 jobs**, including both native Rust versions and comparison. All three ZIP
artifact digests match GitHub. Downloaded run files validate and agree byte for
byte across toolchains: **24/24 measurement cases**, including six expected
1440-entry preflight rejections. Build/source/CI/run metadata bind this main.
[Dispatch evidence](evidence/capacity-dispatch-main-6b33a43.json).

This proves the manual workflow path using a short synthetic series. It is not
a hosted execution of the full official 42-sample local resource study. Those
[recorded 016 measurements](evidence/impl-016-capacity-study.json) remain local,
with their original source/binary/host fingerprints and raw archive identity.

## Selected next slice

[Proposed MDC-IMPL-017](MDC-REPLAY-EXACT-WORK-CONTRACT-v0.1-DRAFT.md) targets two
static sources of repeated work: exporting one immutable snapshot twice during
bundle construction, and normalizing captured SOURCE/decoding MANIFEST twice
while opening a run. Existing semantic recomputation, snapshot restore and
per-operation bound checks remain mandatory.

The proposal includes EW01…EW14, exact before/after file comparison, independently
bound baseline/candidate fingerprints and repeated resource measurements.
Static call counts do not establish a dominant bottleneck or promise a speedup.
Runtime optimization is **not implemented** in this checkpoint.

The 64 MiB journal still limits the pinned 256-entry plan after 153 decisions;
1440 entries still exceed v1 plan/visible bounds. Reducing CPU work will not
change those logical sizes. A larger daily-capacity/retention representation
requires its own evidence and contract; neither history pruning nor a window
was selected here.
