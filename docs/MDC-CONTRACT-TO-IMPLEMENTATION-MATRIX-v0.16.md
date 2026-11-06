# MDC Core — Contract-to-Implementation Matrix v0.16

2026-11-06, MDC-IMPL-016. Extends [v0.15](MDC-CONTRACT-TO-IMPLEMENTATION-MATRIX-v0.15.md).
IMPLEMENTED describes the declared bounded profile, not a trading deployment.

| Area | Status | Evidence / boundary |
|---|---|---|
| Scalar core / lifecycle / retention / identity / snapshot | IMPLEMENTED | Previous APIs and frozen controls retained |
| Linux core WAL / automatic flush | IMPLEMENTED | Existing bounded Store; unchanged |
| Binance normalization / modeled cutoff isolation | IMPLEMENTED | 014 profile and controls; no historical receipt claim |
| Durable replay source/plan/prefix/retry | IMPLEMENTED | 015 protocol and fault controls; unchanged |
| Capacity runner / independent validator | IMPLEMENTED | 016 CP01…CP12, complete retained matrices and raw metrics |
| Default/reduced-budget CI smoke | IMPLEMENTED | Two native Rust versions; actual run-file comparison |
| Large-series opt-in workflow | PARTIAL | Implemented; hosted manual dispatch deferred until workflow is on main |
| Full-day 1440-decision full-history run | MISSING | Existing manifest/visible bounds remain; measured rejection |
| Throughput / cold-recovery RTO / production SLO | UNVERIFIED | Shared host, uncontrolled caches, three samples per point |
| Compaction / changed retention / larger profile | MISSING | Not selected by characterization |
| Broader temporal availability | PARTIAL | Fixed modeled bars; live receipt/corrections remain separate |
| Strategy / market State/Transition/Evaluation / order delivery | MISSING | Technical confirmation is not trading logic |
| Universal power-loss guarantee | UNVERIFIED | Conditional sync, process faults only |

[Contract](MDC-REPLAY-CAPACITY-CONTRACT-v0.1.md), [implementation](MDC-IMPL-016.md).
