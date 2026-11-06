# MDC Core — Contract-to-Implementation Matrix v0.13

2026-11-04, MDC-OPS-005 after merged PR #9. Extends the
[v0.12 matrix](MDC-CONTRACT-TO-IMPLEMENTATION-MATRIX-v0.12.md); historical statuses
are preserved as checkpoints, not silently rewritten.

| Area | Status | Evidence / boundary |
|---|---|---|
| Scalar graph, lifecycle, retention, identity and snapshot | IMPLEMENTED | Existing bounded profiles and frozen vectors unchanged |
| Linux WAL, strict commit/recovery and timed/count flushing | IMPLEMENTED | Existing Store profile; no external replay cursor |
| Binance 1m numeric CSV / exact e8 / declared ms-us | IMPLEMENTED | Merged 014, official BTCUSDT samples and strict parser tests |
| Modeled cutoff isolation and technical State | IMPLEMENTED | 014 fresh views; implicit-witness counterfactual test |
| Post-merge 014 main | IMPLEMENTED | 9f21fe7, hosted run 41831572645, 16/16 PASS; reviewed tree preserved |
| Cross-target replay determinism | IMPLEMENTED | Six configurations; same report/core snapshot bytes |
| Wider historical availability / TEMP requirements | PARTIAL | Modeled fixed final bars, not real receipt time or correction processing |
| Durable replay source/plan/cursor contract | PARTIAL | New recovery proposal and RR01…RR16 acceptance plan |
| Durable completed-decision journal / replay resume | MISSING | Planned 015; ordinary 014 output files do not provide this transaction |
| Measured receipt, provisional bars, corrections | MISSING | Requires separate source/evidence contract |
| Market S/T/E, strategy and order execution | MISSING | No strategy/profitability claim from technical confirmation |
| Whole-day derivation throughput / production SLO | UNVERIFIED | 014 remains bounded per-cutoff reconstruction |
| Universal hardware power-loss guarantee | UNVERIFIED | Sync depends on filesystem/device; no physical power-cut test |

[Post-merge checkpoint](MDC-OPS-005-POST-REPLAY-v0.1.md),
[recovery contract proposal](MDC-REPLAY-RECOVERY-CONTRACT-v0.1-DRAFT.md).
IMPLEMENTED always refers to the declared profile, tests and target set.
