# MDC Core — Contract-to-Implementation Matrix v0.12

2026-11-04, MDC-IMPL-014 on main 1f0b76a plus OPS-004. Historical matrices remain
preserved. IMPLEMENTED applies to the declared bounded software profile only.

| Area | Status | Evidence / limit |
|---|---|---|
| Scalar O/M/R/C/S/T, Evaluation, invalid bodies | IMPLEMENTED | Production APIs unchanged; 213 Rust tests per root profile |
| Lifecycle / full-key retention / portable identity | IMPLEMENTED | Historical graph and lifecycle controls unchanged |
| Recorded exact snapshot | IMPLEMENTED | Frozen archive/manifest control unchanged |
| WAL / HEAD / checkpoint / single-owner timed flush | IMPLEMENTED | Host 36 default / 38 fault tests per toolchain; unchanged wire profile |
| Post-merge 013 checkpoint | IMPLEMENTED | Main 1f0b76a, hosted run 41791564548, 14/14 PASS |
| First source/format | IMPLEMENTED | Binance Spot/BTCUSDT/1m numeric 12-column CSV; official ms/us fixtures |
| Exact OHLCV normalization | IMPLEMENTED | Checked i64 e8, explicit unit/era, no float/default-zero |
| Modeled availability | IMPLEMENTED | Explicit lag/anchor, final-bar boundary; reported as MODELED |
| Real historical receipt/publication | MISSING | Not supplied by selected final-bar archives |
| Cutoff-isolated core view | IMPLEMENTED | Fresh session from eligible bars; counterfactual implicit-witness test |
| Permutation/duplicates/future suffix | IMPLEMENTED | Earlier core bytes and decision ID preserved; raw-file audit hash separate |
| Technical consumer | IMPLEMENTED | OHLCV reads, visible-close differences, Composition and confirmation State |
| Market strategy / market S/T/E | MISSING | Technical State is not a trading signal or profitability result |
| ETHUSDT normalization | PARTIAL | Same schema supported; official downloaded fixtures cover BTCUSDT only |
| Generic root availability semantics | MISSING | Adapter boundary only; unrestricted Pipeline remains unchanged |
| MDC-TEMP-001 T01…T12 / TA01…TA12 | PARTIAL | Implemented subset mapped in 014 report; corrections/cursor cases remain open |
| Provisional bars / late corrections | MISSING | Conflicting same-minute content rejected, not silently revised |
| Replay cursor + durable core prefix | MISSING | Exported core snapshot excludes external cursor; no exactly-once source resumption |
| Bounds / diagnostics | IMPLEMENTED | Input/line/row/visible/call limits reject without truncation |
| First large-dataset throughput / production SLO | UNVERIFIED | Per-cutoff rebuild; no benchmark or rolling-history compaction |
| Cross-target replay controls | IMPLEMENTED | CI requires native/i686/s390x × two Rust versions, frozen bytes and exact six-cell metadata |
| General real-market causal conformance | UNVERIFIED | Fixed modeled latency and known registry assumptions only |
| Real power-cut / live trading readiness | UNVERIFIED | Linux durability depends on FS/hardware; orders outside scope |

[Profile contract](MDC-BINANCE-REPLAY-CONTRACT-v0.1.md),
[implementation and acceptance mapping](MDC-IMPL-014.md),
[local execution receipt](evidence/impl-014-local-verification.json).
Hosted results bind an exact PR head and are reported separately, not inferred
from workflow configuration or local tests.
