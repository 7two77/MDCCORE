# MDC Core — Contract-to-Implementation Matrix v0.9

2026-11-04, MDC-IMPL-012, base main 737f5b6. Scope is the scalar core and bounded
Linux local-filesystem single-writer storage. [v0.8](MDC-CONTRACT-TO-IMPLEMENTATION-MATRIX-v0.8.md)
retains the pre-implementation recovery checkpoint.

| Area | Status | Evidence / limit |
|---|---|---|
| Scalar O/M/R/C/S/T, Evaluation, invalid artifacts | IMPLEMENTED | Production APIs, existing E2E and regression suites |
| Lifecycle / Evaluation retention / portable identity | IMPLEMENTED | Historical semantics retained; frozen graph/snapshot controls unchanged |
| Original roots and implementation milestones | IMPLEMENTED | Integration-history guard; no rewritten main |
| Recorded snapshot roundtrip | IMPLEMENTED | 011 full Registry/history/state and exact replay |
| Bounded immutable replay bridge | IMPLEMENTED | New request/outcome/query API and prepare_call isolation; 4 public tests |
| WAL v1 framing and integrity | IMPLEMENTED | SHA-256 frame chain, fixed wire fields, 8 independent vectors, corruption tests |
| Linux single-writer commit/recovery | IMPLEMENTED | WAL sync + HEAD sync/rename/directory sync; whole committed prefix |
| Buffered vs durable receipts | IMPLEMENTED | Explicit Mode and barrier, qualified live view; suffix can be lost |
| Host ID deduplication / indeterminate errors | IMPLEMENTED | Full request equality, retained receipts, fenced instance until reopen |
| Checkpoint/rotation and orphan cleanup | IMPLEMENTED | New synced pair before HEAD, cleanup only after directory sync |
| Process-crash recovery and faults | IMPLEMENTED | 18 SIGKILL points plus simulated I/O failures; native Linux only |
| Local verification | IMPLEMENTED | 213 Rust + 25 Python × 6 core configurations; host 23 default / 25 fault tests × 2 toolchains |
| New hosted CI | UNVERIFIED | 14-job workflow prepared; actual result recorded separately after publication |
| Universal hardware power-loss guarantee | UNVERIFIED | Depends on tested filesystem/controller/device prerequisites; not established by process-kill tests |
| Durable long-running production service | PARTIAL | Bounded adapter exists; no measured production SLO or compaction |
| Automatic timed/N-event batch scheduler | MISSING | Explicit barrier/checkpoint available; scheduling remains a consumer responsibility |
| External cursor/strategy/broker state transaction | MISSING | Not part of wire v1; no exactly-once order delivery claim |
| Market replay consumer / temporal availability | MISSING | No agreed executable market fixture or available_at/decision_at semantics |
| General unrecorded graph import / migration | PARTIAL | Recorded 011 profile only; no populated-session adoption or engine migration |
| Reinstatement / Source-wide revocation | MISSING | Beyond monotonic lifecycle |
| Public release readiness | PARTIAL | Runtime and CI exist; delivery, consumer and operational requirements remain |

[Implementation report](MDC-IMPL-012.md), [host crate](../adapters/mdc_store/README.md),
[wire contract](MDC-WAL-WIRE-CONTRACT-v1.md),
[local evidence](evidence/impl-012-local-verification.json).
IMPLEMENTED refers to this limited software profile, not to all trading/storage deployments.
