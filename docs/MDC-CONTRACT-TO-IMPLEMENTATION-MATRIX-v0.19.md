# MDC Core — Contract-to-Implementation Matrix v0.19

2026-11-06, MDC-OPS-008. Extends [v0.18](MDC-CONTRACT-TO-IMPLEMENTATION-MATRIX-v0.18.md).
IMPLEMENTED refers to the declared bounded profile, not a production trading SLO.

| Area | Status | Evidence / boundary |
|---|---|---|
| Core / lifecycle / retention / identity / exact snapshot | IMPLEMENTED | Existing root implementation and frozen controls retained |
| Linux store / scheduler | IMPLEMENTED | Unchanged bounded WAL profile |
| Binance normalization / modeled cutoff isolation | IMPLEMENTED | Same final-bar profile and explicit modeled availability |
| Durable replay v1 / exact-work reduction | IMPLEMENTED | 017 byte equality, old-run continuation and actual work-count tests |
| Merged 017 main | IMPLEMENTED | ac364f8, reviewed tree/history/source retained; post-merge receipt |
| Hosted A/B resource observations | IMPLEMENTED | Prior 84 samples retain their original runtime/input/host binding |
| Offline independent-blob size probe | PARTIAL | One 153-record prefix roundtrips; Python size evidence, not a Rust store |
| Experimental storage v2 / codec / streaming API | MISSING | Draft LS01…LS18; runtime not implemented |
| Full-day 1440-decision profile | MISSING | Existing plan/visible/core-work bounds require separate design |
| General compression ratio / performance SLO / cold RTO | UNVERIFIED | Neither one prefix nor grouped timings establish a universal bound |
| Broader temporal availability | PARTIAL | Modeled bars; measured receipt/corrections remain separate |
| Strategy / market states / order delivery | MISSING | Technical confirmation and exact stored views are not trading execution |
| Universal power-loss guarantee | UNVERIFIED | Existing conditional filesystem/device synchronization |

[Checkpoint and size evidence](MDC-OPS-008-POST-EXACT-WORK-v0.1.md),
[next contract](MDC-REPLAY-STORAGE-V2-CONTRACT-v0.1-DRAFT.md).
