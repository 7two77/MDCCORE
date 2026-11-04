# Bounded Binance Spot replay

`BINANCE-SPOT-1M-MODELED-001` is an independent std host crate. It reads the
official numeric 12-column, headerless Binance Spot 1m CSV schema. BTCUSDT is
the frozen official fixture; ETHUSDT uses the same explicitly selected schema
but has no independently downloaded fixture in this package. No Deribit,
Parquet, generic quoted CSV, streaming service or trading execution is provided.

## Time and exact values

Select `ms` for pre-2025 Spot archives or `us` for archives from 2025-01-01.
The [official format reference](https://github.com/binance/binance-public-data#klines)
documents this change. No magnitude-based guessing or mixed-unit file is accepted.
Canonical time is unsigned UTC Unix microseconds. Open must be minute-aligned;
close must equal exclusive minute end minus the input timestamp quantum.

`event_timestamp = reported close_time`. All availability is **MODELED**, never
measured historical receipt. Choose an explicit nonnegative integer lag in ms:

- `close`: `available_at = event_timestamp + lag`. This preserves the proposed
  formula. Zero lag is rejected: Binance's inclusive close value precedes the
  end of the finalized minute. A positive integer ms lag clears this boundary.
- `end`: `available_at = open_time + 60s + lag`. Use this for a safe zero-lag
  assumption. It is a different named model; the adapter does not silently
  change the close-anchored formula.

OHLCV and quote/taker volumes are exact nonnegative `i64` with scale 1e8.
Prices must be positive; OHLC geometry and taker-volume bounds are checked.
Signs, NaN, exponents, overflow and nonzero digits past precision 8 are errors.
Additional trailing zeros are harmless. No float or error-to-zero conversion.
All 12 numeric columns are validated; ignored column 12 is not semantic data.
CRLF/LF is accepted; BOM, headers, quotes, whitespace, missing/extra columns and
blank records are errors. Parsing is atomic, including unavailable later rows.

## Decision view

`Dataset::decision(decision_us)` creates a fresh frozen-registry `SnapshotSession`
with only bars satisfying `available_at <= decision_us`. Equality assumes receipt
before the decision at that modeled instant. Sorting uses availability, event,
open time and evidence ID. Equivalent repeated bars deduplicate; conflicting
content for the same minute is rejected rather than interpreted as a correction.

Each visible bar becomes five real core Observations and scalar Measurements,
with source/schema/time-basis versions, e8 units and evidence ID in acquisition
context. Adjacent visible closes feed the core's ordered Difference operator
as `[current, previous]`. There is no assertion that missing minutes are contiguous.
A one-member Composition of the first close and an ObservationAfter State
exercise implicit witness selection: one bar gives Unresolved, a later visible
close gives Pass. This is a **technical confirmation**, not a market regime or
trading signal. State remains Candidate; no market Transition or Evaluation is
invented. With no visible bars there is no State (`NO_INPUT`).

The view exposes only read-only core access. The root Pipeline remains unchanged
and has no general availability gate. Manual admission of `MarketEvidence::observation`
outside this view bypasses the profile. The counterfactual test demonstrates why
filtering explicit references alone is insufficient for current implicit selectors.

Evidence ID is SHA-256 of versioned canonical fields (see the root profile
contract). Decision ID binds profile, cutoff and full eligible evidence. The raw
whole-file hash is separate post-hoc audit metadata: appending a future suffix
changes that hash but not earlier core snapshots or decision IDs. Report origin
is `provided_csv`; parsing alone does not attest that a file is from Binance.

Default limits: 4 MiB input, 4096 rows, 4096 bytes/line, 256 visible bars; exceeding
them errors without truncation. A preflight also enforces the core snapshot call
budget. This is a bounded technical slice, not a full-day performance backtest.
Complexity includes rebuilding and serializing the visible history per decision.
No external cursor/WAL transaction, correction revisions, measured receipt time,
parameter learning, positions, orders or profit calculation is implemented.

## Run from repository root

```bash
cargo fetch --locked --manifest-path adapters/mdc_replay/Cargo.toml
cargo run --locked --offline --manifest-path adapters/mdc_replay/Cargo.toml \
  --bin mdc-binance-replay -- inspect \
  adapters/mdc_replay/tests/fixtures/BTCUSDT-1m-2025-01-01-first3.csv BTCUSDT us 100 close
```

Use new output paths; the CLI never overwrites an existing file:

```bash
MDC_REPLAY_OUT=$(mktemp -d)
cargo run --locked --offline --manifest-path adapters/mdc_replay/Cargo.toml \
  --bin mdc-binance-replay -- decision \
  adapters/mdc_replay/tests/fixtures/BTCUSDT-1m-2024-01-01-first3.csv \
  BTCUSDT ms 100 close 1704067320049000 \
  "$MDC_REPLAY_OUT/decision.json" "$MDC_REPLAY_OUT/core.bin"
```

Expected: one eligible bar and technical `UNRESOLVED`, because bar 2's event
is already in horizon but its modeled receipt is not. Exported core bytes use
the unchanged 011 recorded snapshot format. Restore requires that profile, not
arbitrary graph import. JSON and optional snapshot are ordinary separate artifact
writes, not synced or atomically published as a durable pair. On an output error
the CLI exits unsuccessfully; a previously written artifact may remain.

Verification: `bash .github/ci/replay.sh` in an empty report directory. Native
tests include three fresh CLI processes; CI compares two report/snapshot pairs
across native x86_64, i686 and s390x/QEMU on two Rust versions. Frozen small
official samples and their archive/checksum provenance are in `tests/fixtures/`;
CI performs no network market-data fetch.
