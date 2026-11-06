# MDC Core — Contract-to-Implementation Matrix v0.15

2026-11-06, MDC-OPS-006. Extends [v0.14](MDC-CONTRACT-TO-IMPLEMENTATION-MATRIX-v0.14.md).
IMPLEMENTED refers to the declared bounded profile and targets.

| Area | Status | Evidence / boundary |
|---|---|---|
| Scalar core / lifecycle / retention / identity / snapshot | IMPLEMENTED | Previous APIs and frozen controls unchanged |
| Linux core WAL / automatic flush | IMPLEMENTED | Existing bounded Store profile |
| Binance normalization / modeled cutoff isolation | IMPLEMENTED | Fixed 014 controls; no measured receipt claim |
| Durable input/plan, prefix, isolated snapshots and retry | IMPLEMENTED | 015 RR01…RR16; full bytes and 24 SIGKILL points |
| Post-merge 015 main | IMPLEMENTED | ee4d45f, run 42234145381, 16/16 PASS; reviewed tree preserved |
| Capacity observation | PARTIAL | One release sample: 64 complete; 256 limited after 153 committed |
| Reproducible resource harness / statistics | MISSING | CP01…CP12 proposed; pilot is not formal benchmark execution |
| Full-day 1440-decision full-history run | MISSING | CLI/plan/visible bounds and journal capacity |
| Throughput / cold-recovery RTO | UNVERIFIED | Warm/uncontrolled-cache sample does not define an SLO |
| Rotation / compaction / changed retention | MISSING | Separate protocol/history obligations needed |
| Broader temporal availability | PARTIAL | Fixed modeled bars; live receipt/corrections remain open |
| Strategy / market S/T/E / order delivery | MISSING | Technical confirmation and durable ACK are not trading logic |
| Universal power-loss guarantee | UNVERIFIED | Conditional sync; process-kill tests only |

[Post-merge check](MDC-OPS-006-POST-RECOVERY-v0.1.md),
[measurement proposal](MDC-REPLAY-CAPACITY-CONTRACT-v0.1-DRAFT.md),
[pilot](evidence/capacity-pilot-001.json).
