# MDC-OPS-004 — Post-auto-flush checkpoint and first replay preparation

2026-11-04. Owner merged PR #8. Main:
`1f0b76a2ecf58eba5b1bbc9354b7131300c3cb4e`; parents:
`ee3c3c6b776fd4276f4fe8171d7b0f2afacc6031` and reviewed 013
`06c3af4b0c17344cde752a2b6085b1d5ce54bb14`.
Tree `9a39938199c48659af72300a6e48d4c1f6077df3` matches the reviewed feature tree.
History and current scalar E2E evidence checks pass; neither root was rewritten.

[Post-merge CI](https://github.com/7two77/MDCCORE/actions/runs/41791564548) passed
14/14 jobs: core 213 Rust + 25 Python per native profile, cross 26 Rust each,
host 36 default + 38 fault tests per toolchain. Snapshot/portable controls agree
across ten configurations. All fourteen artifacts are retained.
[Receipt](evidence/post-merge-main-1f0b76a.json) records actual job checks and comparisons.

Storage now includes optional time/count flush. Its soft scheduling and conditional
filesystem/device guarantees remain as described in MDC-IMPL-013; no real-time or
physical power-cut guarantee follows from this checkpoint.

Next priority is the first historical consumer and data availability, rather than
assuming storage durability prevents lookahead. The owner has been asked for the
provider/instrument/event type/file profile. Until that answer, no market source,
time unit, receipt evidence or OHLCV schema is selected on their behalf.

[MDC-TEMP-001 scope draft](MDC-TEMP-001-SCOPE-v0.1-DRAFT.md) identifies current
implicit witness selection and proposes temporal requirements/acceptance cases.
This is requirements preparation only; core, host runtime and persisted formats
are unchanged. A concrete availability/replay contract remains pending source details.
