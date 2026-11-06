# MDC-OPS-005 — Post-replay main and durable consumer preparation

2026-11-04. Owner merged PR #9. Main is
`9f21fe7a81c393dc5f9d3cffc96d3d0871ec48da`, parents
`1f0b76a2ecf58eba5b1bbc9354b7131300c3cb4e` and reviewed 014
`fb7625d05cd9e925a4d49ef5d1bc1ba764024f66`.
Tree `5a8adc68d50818cf07e9eb9e8bd7e141c9a75969` exactly matches the reviewed tree.
Both original histories, integration milestones and the current scalar E2E
evidence checker remain valid.

[Post-merge CI](https://github.com/7two77/MDCCORE/actions/runs/41831572645) passed
**16/16 jobs**. Actual logs confirm:

- Core: 213 Rust + 25 Python per native configuration, six configurations.
- Replay: 22 Rust + 5 Python on each of Rust 1.85.1 and 1.98.0.
- Cross: 43 Rust per i686/s390x configuration, four configurations.
- Linux store: 36 default + 38 fault tests per toolchain.
- Ten core portable/snapshot controls agree; six modeled replay report/snapshot
  pairs agree, retaining technical Unresolved when the next event is unavailable.

All sixteen artifacts were uploaded. The comparison ZIP was downloaded and its
SHA-256 verified against GitHub's artifact digest. All three comparison receipts
were validated against this exact main/source fingerprint and frozen controls.
[Checkpoint receipt](evidence/post-merge-main-9f21fe7.json).

014 remains bounded technical replay with modeled receipt time. Market strategy,
correction revisions and the external source/run recovery transaction are open.
Existing standalone JSON/snapshot writes do not atomically advance a replay cursor.

The next proposal is [MDC-REPLAY-RECOVERY-001](MDC-REPLAY-RECOVERY-CONTRACT-v0.1-DRAFT.md),
preparing MDC-IMPL-015. Because each cutoff has an isolated Registry/session,
the durable unit is a completed decision bundle. A run journal and certified HEAD
must bind the retained source, immutable plan, next decision index and exact
per-cutoff snapshots. No automatic adoption of a populated view by mdc_store or
reuse of an unfiltered later graph is assumed.

This checkpoint changes documents only. The new recovery contract is a proposal,
not implemented functionality or new execution evidence. Existing source/profile
fingerprint and all previously frozen formats remain unchanged.
