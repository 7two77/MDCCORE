# MDC Core — Contract-to-Implementation Matrix v0.17

2026-11-06, MDC-OPS-007. Extends [v0.16](MDC-CONTRACT-TO-IMPLEMENTATION-MATRIX-v0.16.md).
IMPLEMENTED refers to the declared bounded profile, not a production trading SLO.

| Area | Status | Evidence / boundary |
|---|---|---|
| Core / lifecycle / retention / portable identity / exact snapshot | IMPLEMENTED | Preserved APIs, history and frozen controls |
| Linux store WAL / scheduled flush | IMPLEMENTED | Existing bounded single-owner profile, unchanged |
| Binance normalization / cutoff isolation | IMPLEMENTED | MODELED availability; unchanged 014 controls |
| Durable replay prefix / retry / exact recovery | IMPLEMENTED | 015 framing and recomputation, unchanged |
| Reproducible resource harness / raw validation | IMPLEMENTED | 016 CP01…CP12 and saved local full series |
| Default/reduced-journal regular CI smoke | IMPLEMENTED | Native matrix and complete retained-file comparisons |
| Merged 016 main | IMPLEMENTED | 6b33a43; post-merge CI 42247801358, 16/16 PASS; reviewed tree/history retained |
| Opt-in workflow execution | IMPLEMENTED | Explicit dispatch 42248409893, 3/3 PASS, 24 synthetic measurements |
| Full official resource study on hosted workflow | UNVERIFIED | 42 official samples were measured locally; dispatch used short synthetic series |
| Exact-work reduction (017) | MISSING | Contract draft EW01…EW14; no runtime change |
| Attribution of dominant CPU bottleneck / speedup | UNVERIFIED | Static duplicate calls and 016 timings are not CPU profiling or paired speedup evidence |
| Full-day 1440-decision full-history run | MISSING | Existing plan/visible/journal limits preserved |
| Compaction / larger profile / changed retention | MISSING | No new representation or history-loss policy selected |
| Broader temporal availability | PARTIAL | Fixed modeled bars; measured receipt/corrections remain separate |
| Strategy / market State/Transition/Evaluation / orders | MISSING | Technical confirmation and saved decisions do not constitute trading logic |
| Cold-recovery RTO / universal power-loss guarantee | UNVERIFIED | Uncontrolled caches and conditional filesystem/device sync |

[Post-merge evidence](MDC-OPS-007-POST-CAPACITY-v0.1.md),
[next contract](MDC-REPLAY-EXACT-WORK-CONTRACT-v0.1-DRAFT.md).
