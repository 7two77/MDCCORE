# MDC Core — Contract-to-Implementation Matrix v0.11

2026-11-04, MDC-OPS-004, main 1f0b76a after PR #8.
[v0.10](MDC-CONTRACT-TO-IMPLEMENTATION-MATRIX-v0.10.md) remains the 013 preparation checkpoint.
Scope: bounded scalar core and Linux local-filesystem single-writer storage.

| Area | Status | Evidence / limit |
|---|---|---|
| Scalar O/M/R/C/S/T, Evaluation and invalid artifacts | IMPLEMENTED | Existing production APIs and regression suites |
| Lifecycle, retention, portable identity, exact snapshot | IMPLEMENTED | Frozen graph/archive controls retained |
| Recorded replay bridge and WAL v1 | IMPLEMENTED | 012 prefix integrity/replay; wire format unchanged |
| Strict commit/recovery, IDs, fencing, checkpoint/rotation | IMPLEMENTED | Original 18 SIGKILL points remain covered |
| Post-merge 012 main | IMPLEMENTED | Run 41762158648: 14/14 PASS, reviewed file tree preserved |
| Optional count/idle flush | IMPLEMENTED | Single worker, monotonic oldest-pending deadline, no duplicate-ID count |
| Bounded queue / cloned clients | IMPLEMENTED | One filesystem writer; ordered RPCs, no mutable Store escape |
| Qualified snapshot/status and autonomous errors | IMPLEMENTED | Live/durable position, healthy flag, fencing on failed flush |
| Explicit shutdown and lock release | IMPLEMENTED | Flush/join Result; Drop does not provide ACK |
| Local verification | IMPLEMENTED | Core 213 Rust + 25 Python × 6; host 36 default / 38 fault × 2 |
| Post-merge 013 main | IMPLEMENTED | Run 41791564548: 14/14 PASS, actual job counts/comparisons and 14 artifacts retained |
| Temporal/replay requirements | PARTIAL | Draft T01…T12/TA01…TA12 prepared; source profile and runtime not selected/implemented |
| 012 persisted-store compatibility | IMPLEMENTED | Four saved hosted store fixtures consumed successfully |
| Automatic rotation / core history compaction | MISSING | Checkpoint remains explicit; retained history still bounded |
| Hard realtime or production throughput/SLO | UNVERIFIED | max_delay is soft; sync/scheduling/queue latency can exceed it |
| Universal physical power-loss guarantee | UNVERIFIED | Conditional on tested storage stack; no real power-cut testing |
| External cursor/strategy/broker transaction | MISSING | No exactly-once external order delivery |
| Market consumer / historical availability | MISSING | No agreed executable market fixture or available_at/decision_at |
| General unrecorded graph import / migration | PARTIAL | Recorded profile only, no populated-session adoption |
| Reinstatement / Source-wide revoke | MISSING | Outside monotonic lifecycle |
| Public production release readiness | PARTIAL | Consumer, resource envelope and operational delivery remain open |

[Implementation report](MDC-IMPL-013.md), [local evidence](evidence/impl-013-local-verification.json),
[post-merge checkpoint](evidence/post-merge-main-1f0b76a.json),
[temporal scope draft](MDC-TEMP-001-SCOPE-v0.1-DRAFT.md).
IMPLEMENTED refers to this software profile and its declared fixtures/targets.
