# MDC Core — Contract-to-Implementation Matrix v0.20

2026-11-06, MDC-IMPL-018. Extends [v0.19](MDC-CONTRACT-TO-IMPLEMENTATION-MATRIX-v0.19.md).
IMPLEMENTED applies to declared bounded profiles. Execution receipts retain their
own exact runtime, source, compiler and input binding.

| Area | Status | Evidence / boundary |
|---|---|---|
| Root core / lifecycle / retention / identity / exact snapshot | IMPLEMENTED | Root runtime and frozen controls preserved |
| Linux store / scheduler / normalized modeled replay | IMPLEMENTED | Existing host/core profile preserved |
| Durable replay v1 and old-run continuation | IMPLEMENTED | Same files/defaults/API; pinned 016 comparison retained |
| Explicit v2 raw/zlib storage | IMPLEMENTED | Independent frames/decoded hashes; pinned host-only codec; LS01…LS09 |
| Streaming validation / owned reads / fencing | IMPLEMENTED | Per-decision semantic replay, bounded index and certified reads; LS10…LS14 |
| Resource study tooling | IMPLEMENTED | Separate release drivers, exact common-prefix comparison, retained samples |
| Official 256-decision capacity and CPU/RSS | UNVERIFIED | Requires actual Rust study; see 018 execution receipt when available |
| Full-day 1440-decision profile | MISSING | Existing plan/visible/core-work bounds intentionally retained |
| General compression ratio / performance SLO / cold RTO | UNVERIFIED | Measurements do not establish universal guarantees |
| Broader temporal availability | PARTIAL | MODELED bar availability; measured receipt/corrections require another contract |
| Strategy / market states / order delivery | MISSING | Technical confirmation is not trading execution |
| Universal power-loss guarantee | UNVERIFIED | Conditional Linux/local filesystem/device synchronization |

[Implementation and evidence](MDC-IMPL-018.md),
[wire, limits and acceptance contract](MDC-REPLAY-STORAGE-V2-CONTRACT-v0.1.md).
