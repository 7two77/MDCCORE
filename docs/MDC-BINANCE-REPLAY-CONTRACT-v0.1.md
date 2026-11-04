# MDC-IMPL-014 — Binance modeled-availability contract v0.1

2026-11-04. Selected first executable slice of MDC-TEMP-001. Owner proposed
Binance Spot or Deribit, BTCUSDT or ETHUSDT, 1m OHLCV, CSV or Parquet and modeled
lag. This implementation selects **Binance Spot / BTCUSDT / 1m / numeric CSV**.
The choices are engineering scope, not new MDC-01…06 normative requirements.

## Envelope, time and admission

Profile `BINANCE-SPOT-1M-MODELED-001` freezes the rules described in the
[adapter reference](../adapters/mdc_replay/README.md). Data availability has quality
MODELED; no publication/receipt field is supplied by these final-bar archives.
Host processing time is not event or availability time. The fixed registry is
an assumption of this technical fixture, not historically learned parameters.

Explicit MS input is accepted for open timestamps before 2025-01-01 UTC; explicit
US input for timestamps on/after that date, matching the official Spot change.
Normalization uses checked multiplication to UTC Unix microseconds. Final-bar
event is inclusive `close_time`; availability uses the declared ReportedClose or
MinuteEnd anchor plus checked integer lag. There is no event-time fallback for
an unknown model. The profile has no API claiming uncertain/measured quality.

At cutoff D, the only admitted data are finalized rows with A <= D. Equal A/D
assumes modeled receipt before decision. Within a file symbol/unit/model are fixed;
different timestamps sort by `(A, event, open, evidence_id)`. Same-minute duplicates
with equal canonical content deduplicate; conflicting same-minute content errors.
No provisional bars or correction revisions are supported in v1. Invalid input
rejects the entire dataset with line/column/reason, including invalid future rows.

## Canonical envelope and provenance

All fields use LE, with no platform-sized integers. Encoding:

| Field | Encoding |
|---|---|
| Profile | literal `BINANCE-SPOT-1M-MODELED-001` + NUL |
| Symbol | u8: BTCUSDT=0, ETHUSDT=1 |
| Input unit | u8: ms=0, us=1 |
| Lag anchor | u8: ReportedClose=0, MinuteEnd=1 |
| Lag, open, event, availability | four u64 in microseconds |
| Open/high/low/close/base volume | five nonnegative i64, scale 1e8 |
| Quote volume | nonnegative i64, scale 1e8 |
| Number of trades | u64 |
| Taker base/quote volumes | two nonnegative i64, scale 1e8 |

SHA-256 of these bytes is evidence_id. This envelope is outside the MDC calculus;
its digest is not a replacement for core portable identity. Raw CSV spellings,
line numbers and ignored field 12 are not in semantic identity. The full raw file
SHA is retained in the report for post-hoc audit, outside decision/core identity.
Sample provenance binds official archive URL/SHA/checksum and selected bytes.
The runtime labels any caller-provided CSV `provided_csv`, without authenticity
attestation. Original bytes remain the audit input; the CLI does not rewrite them.

Each eligible candle projects five signed-i64-le sources, versioned schema and
operators through public core APIs. Units distinguish quote/base prices and base
volume, both scaled e8. The source time basis is UTC/unix/microseconds v1.
Acquisition context binds profile/evidence_id/field. The report carries the full
canonical envelope so these IDs are independently auditable, including modeled
availability. Decision ID is SHA-256 over the literal profile/decision/v1 + NUL,
encoded profile, cutoff u64, eligible count u64 and each length-prefixed full
canonical envelope. It deliberately excludes unavailable suffix and full-file hash.

## Isolation and technical consumer

Every decision builds a fresh SnapshotSession containing only eligible bars.
No populated unrestricted Pipeline is reused for a historical cutoff. All explicit
refs, recursive lineage and implicit witness searches in this view therefore see
only the eligible graph. No mutable session escape is supplied. Manually using
the scalar projection elsewhere has no availability guarantee.

The consumer computes OHLCV scalar values and adjacent visible close differences,
then Composition/State technical confirmation. It exercises real ObservationAfter
selection with horizon D: a missing later available witness stays Unresolved.
State Pass still means Candidate. No market strategy, Transition or market
Evaluation is specified. Derived output is computed at D from data eligible at D;
no earlier output publication or real computation latency is claimed.

Decision snapshots are immutable and independent. Later knowledge, changed model
or changed visible content creates a different context rather than repairing an
old view. Current lifecycle revocations of a shared live graph are not modeled;
these fresh views start from the same fixed registry and source fixture.

## Acceptance boundary

The implementation tests exact normalization, parsing failures/overflow, ms/us,
inclusive close/end, cutoff before/equal/after, duplicates/conflicts, permutation,
late suffix and operation limits. A counterfactual contaminated core demonstrates
the implicit-witness leak that isolated views prevent. Fresh process and cross
target controls bind output bytes, not merely a digest-only assertion.

MDC-TEMP-001 remains PARTIAL: TA07 correction processing, TA10 external replay
cursor/restart, arbitrary availability quality/time-varying config, provisional
updates and market S/T/E expectations are not implemented. Existing scalar E2E,
snapshot v1, identity protocol and WAL v1 remain separate unchanged profiles.
Standalone replay artifact writes are not a durable store protocol. Integrating
the source cursor and decision boundary with WAL requires another contract.
