# MDC Core — Contract-to-Implementation Matrix v0.14

2026-11-06, MDC-IMPL-015. Extends the historical v0.13 checkpoint. Status applies
to the declared bounded profile, target set and tests, not arbitrary live trading.

| Area | Status | Evidence / limit |
|---|---|---|
| Scalar graph, Evaluation, invalid bodies, lifecycle and retention | IMPLEMENTED | Prior production APIs and suites unchanged |
| Portable identity / recorded exact snapshot | IMPLEMENTED | Frozen graph and engine-011 controls retained |
| Linux core WAL / automatic time-count flush | IMPLEMENTED | Existing mdc_store; no format/source change in 015 |
| Binance exact CSV / modeled availability / cutoff isolation | IMPLEMENTED | Prior 014 bytes retained; pure and CLI tests |
| Immutable source/model/plan run manifest | IMPLEMENTED | Exact capture, namespace, versioned bounds; expectation checks |
| Durable completed-decision journal and cursor | IMPLEMENTED | HEAD-certified prefix, strict commit, one writer, retained reports/snapshots |
| Retry / indeterminate error handling | IMPLEMENTED | No append on old index; fencing and sealed recovery before access |
| Restart equivalence / semantic validation | IMPLEMENTED | Restore/recompute every view; continuous vs resumed full byte equality |
| Corruption and interrupted publication | IMPLEMENTED | RR01…RR16 coverage, 24 SIGKILL points, independent validators/vectors |
| Cross-target run codec | IMPLEMENTED | Native/i686/s390x pure wire vectors; filesystem runtime tested on native Linux |
| Wider historical availability / TEMP requirements | PARTIAL | Fixed modeled final bars; no measured receipt or correction processing |
| Whole-day service / production resource envelope | UNVERIFIED | Per-decision full snapshots, bounded reconstruction and journal |
| Journal rotation / compaction / plan append | MISSING | No automatic deletion or extension of retained history |
| Market strategy, market S/T/E and order delivery | MISSING | Technical confirmation and durable ACK do not authorize trading |
| Real power-loss guarantee on arbitrary storage | UNVERIFIED | Conditional sync semantics; process kill is not a physical power cut |

[Implementation](MDC-IMPL-015.md), [wire contract](MDC-REPLAY-RUN-WIRE-v1.md),
[post-merge 014 checkpoint](MDC-OPS-005-POST-REPLAY-v0.1.md).
Exact hosted head/run/results are supplied on the implementation PR, separate
from these preparation and local execution records.
