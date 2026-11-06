# MDC Core — Contract-to-Implementation Matrix v0.18

2026-11-06, MDC-IMPL-017. Extends [v0.17](MDC-CONTRACT-TO-IMPLEMENTATION-MATRIX-v0.17.md).
IMPLEMENTED refers to the declared bounded profile, not a trading deployment SLO.

| Area | Status | Evidence / boundary |
|---|---|---|
| Core / lifecycle / retention / identity / recorded snapshot | IMPLEMENTED | Root implementation and frozen controls retained |
| Linux store / scheduler | IMPLEMENTED | Unchanged single-owner WAL profile |
| Binance normalization / cutoff isolation | IMPLEMENTED | Existing MODELED profile and controls |
| Durable replay / exact prefix / old-run continuation | IMPLEMENTED | Original v1 format; old CLI→new CLI and full file equality |
| Owned capture and prepared manifest/source (017) | IMPLEMENTED | Actual exports 3→1, open decode/parse 2→1; mandatory restore/recompute retained |
| Pinned A/B validator / raw evidence archives | IMPLEMENTED | Each variant validated against its own reviewed fingerprint; negative validator tests |
| Official hosted resource benefit | UNVERIFIED | Requires the separately executed paired study; static counts are insufficient |
| Full-day 1440-decision full-history profile | MISSING | Existing plan/visible/journal bounds unchanged |
| Compaction / larger budgets / changed retention | MISSING | Separate representation/contract not selected |
| Broader temporal availability | PARTIAL | Modeled final bars; measured receipt/corrections remain separate |
| Strategy / market states / orders | MISSING | Technical confirmation and stored views are not trading logic |
| Performance SLO / cold RTO / universal power-loss guarantee | UNVERIFIED | Uncontrolled caches and conditional filesystem/device sync |

[Implementation](MDC-IMPL-017.md), [contract](MDC-REPLAY-EXACT-WORK-CONTRACT-v0.1.md).
