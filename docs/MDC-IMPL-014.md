# MDC-IMPL-014 — First Binance CSV and modeled historical replay

2026-11-04. Base main `1f0b76a2ecf58eba5b1bbc9354b7131300c3cb4e` after PR #8.
The preceding OPS-004 checkpoint records post-merge 013 CI and preserved ancestry.
This package adds runtime code in independent `adapters/mdc_replay`, not in the
root scalar model or the Linux store. It implements a bounded first technical
consumer from the owner's proposed 1m OHLCV dataset.

## Result and decisions

Selected Binance Spot / BTCUSDT / 1m / headerless numeric CSV. ETHUSDT has the
same selected-schema mapping but not its own downloaded fixture here. Deribit,
Parquet and general CSV parser behavior are deferred. Dependency SHA-256 is pinned
to sha2 0.10.9 in this host crate; root manifest/lock and nested original package
are preserved. No serde/bincode or floating-point price conversion is introduced.

Official Binance timestamps changed from ms to us on 2025-01-01 for Spot archives.
The selected unit is explicit and era-validated. The source close value is an
inclusive interval end. The close-anchored 100 ms example preserves the owner's
formula; zero-lag final bars instead require the separately named MinuteEnd model.
See [exact profile contract](MDC-BINANCE-REPLAY-CONTRACT-v0.1.md) and
[API/CLI reference](../adapters/mdc_replay/README.md).

Availability is always MODELED. A CSV parser cannot reconstruct actual network
receipt time or attest provider authenticity. The runtime reports provided_csv
origin, profile/model version, clock unit, lag, raw-file audit hash and evidence
canonical bytes. Checked-in samples have independent archive/checksum provenance.

Each decision builds a fresh SnapshotSession with only eligible evidence. Five
real core O/M scalars per bar retain exact e8 OHLCV. Core Difference computes
current-close minus previous-visible-close. Composition and a technical State
exercise implicit ObservationAfter selection without making up a market regime.
No market Transition, Evaluation, strategy, execution or profitability is claimed.

The decisive negative case sets cutoff after bar 2's event but before its modeled
availability. The isolated view admits bar 1 only and yields Unresolved. A test
restores that view, manually contaminates the core with bar 2 and demonstrates
that unrestricted current selectors yield Pass. The original view remains
unchanged. This proves the filter covers a real implicit-witness path, not only
explicit references or a hand-written fixture algorithm.

## Acceptance mapping against the wider TEMP draft

| Cases | Result in this slice |
|---|---|
| TA01 / TA02 | Real core O/M/Composition/State APIs and before/equal/after cutoff tests |
| TA03 | Explicit modeled quality/unit/anchor required; invalid config is an error; general measured/uncertain quality unsupported |
| TA04 / TA05 | State witness late-publication negative with contamination counterfactual; no claim for arbitrary global Pipeline or market Transition |
| TA06 | Final rows only, exclusive minute-end floor; provisional updates unsupported |
| TA07 | Conflicting same-minute rows rejected; late correction processing remains MISSING |
| TA08 | Stable sorting, exact duplicates, numeric equivalence and input permutation tests |
| TA09 | Fixed predeclared registry, outputs computed at cutoff from visible inputs; learned/time-varying config unsupported |
| TA10 | External source cursor/restart transaction remains MISSING; core snapshot export uses existing recorded format |
| TA11 | Report binds raw input and config; conflicts diagnosed; no continuation protocol across changed files |
| TA12 | Future suffix leaves prior core bytes/decision ID unchanged; independent process comparisons |

Neither the complete TEMP gate nor observed historical market causality is declared
PASS. The earlier scalar conformance gate remains a separately scoped regression.

## Executed local verification

Full evidence: [local receipt](evidence/impl-014-local-verification.json),
[source manifest](evidence/impl-014-source-manifest.json),
[official input inspection](evidence/impl-014-source-inspection.json).

- Rust 1.85.1 and 1.98.0, root default/std-alloc/alloc: **213 Rust + 25 Python**
  per configuration; Format, Clippy, examples, E2E 23 cases in each of two processes.
- Replay crate: **22 Rust + 5 Python** per toolchain, Format/Clippy and two
  separate production CLI processes. Native Rust tests include a third-process
  equality check. Frozen golden case has one eligible bar and Unresolved State.
- Linux store unchanged: **36 default + 38 fault tests** per toolchain and
  three fresh-process recovery cases including the scheduled owner.
- Two complete official daily archives (2024-01-01 ms and 2025-01-01 us),
  SHA/checksum verified, 1440 rows each parsed. Only first-three-row fixtures
  are committed. Full-day parsing does not establish full-day derivation capacity.
- Prior portable graph, rich snapshot archive/manifest and replay output bytes
  match their frozen controls. New replay snapshot SHA-256 is
  `bdf0a775f3b72f9f6b0ac2cc4815833b267da6f8af7395aa2f042ec04d41c900`.

CI adds two native replay jobs and extends four cross jobs with 17 pure replay
tests and two CLI executions each. Existing cross tests remain 26, total **43**
per cross configuration. Comparison requires all **six** replay configurations
and all **ten** prior core/snapshot configurations with matching exact checkout,
source fingerprint, complete matrix, clean worktree and CI-file hashes. Workflow
now contains 16 jobs; actual hosted completion is recorded separately on the PR.

## Limits and next boundary

Default input 4 MiB / 4096 rows / 4096 bytes per line / 256 visible bars;
core-call budget is checked before construction. Limits error without truncation.
Each cutoff rebuilds a bounded history, including snapshot serialization; this
package contains no throughput result, rolling window or whole-day strategy run.
Malformed rows reject the complete dataset even if after the selected cutoff.

Two output artifacts are ordinary create-new writes, not an atomic/synced durable
pair. Core snapshots do not contain the original full CSV, replay cursor or trading
state; the report and source input remain required audit companions. No cursor
recovery is inferred from existing core WAL support. Corrections, provisional
updates, measured availability and lifecycle interactions across independent
as-of views require later contracts. Orders, live exchange access and position
accounting are not implemented.

The next implementation should first decide the durable source/run manifest and
cursor boundary, then prove restart at an exact committed decision prefix. Market
pattern and S/T/E semantics can be developed after defining their evidence and
acceptance rules; no invented strategy is added to this technical fixture.
