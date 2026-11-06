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
Standalone views do not include a cursor/WAL transaction. RunStore below supplies
source/plan recovery. Correction revisions, measured receipt time, parameter
learning, positions, orders and profit calculation remain outside this profile.

## Durable runs (MDC-IMPL-015)

`recovery::RunManifest` binds captured CSV bytes/hash, model, explicit increasing
cutoffs and limits. Linux `recovery::RunStore` creates/opens one writer, commits
plan indices and retains complete reports/core snapshots. `lookup`, `durable_count`
and `next_index` expose only a validated/synced prefix. No mutable core escapes.
The portable `validate_prefix` decoder checks logical certification in memory,
without itself acknowledging physical durability.

An open RunStore belongs to its creating process. A child inheriting it across
fork must reopen independently; inherited access returns WrongProcess. Owner
drop explicitly unlocks even if a forked child still holds a duplicate descriptor.

Source/plan/model are immutable. `open(path, Some(&expected))` rejects changed
expectations before repair; `open(path, None)` uses retained SOURCE. Changing the
original input pathname cannot replace captured bytes. Changed raw input or a
future suffix requires a new run, preserving earlier 014 cutoff core identity.

`step(index)` returns after journal sync and atomic HEAD publication/directory
sync. Committed-index retry returns the same bundle without append. Errors after
mutation fence the writer until reopen; validation/limit errors do not advance it.
Recovery restores recorded 011 sessions and recomputes every isolated 014 decision,
then syncs the selected prefix/HEAD and discards uncertified tail. No silent rollback.

[Wire format](../../docs/MDC-REPLAY-RUN-WIRE-v1.md) is separate from the existing
mdc_store WAL. New defaults: 256 plan entries, 64 MiB journal, 24 MiB record body,
1 MiB report and 16 MiB snapshot; smaller positive limits are retained in the
manifest. A run can hit capacity before consuming its entire source. There is
no rolling-history compaction or production-scale throughput result.

Use an existing parent directory and a NEW run path:

```bash
cargo run --locked --offline --manifest-path adapters/mdc_replay/Cargo.toml \
  --bin mdc-replay-run -- create /path/to/new-run \
  adapters/mdc_replay/tests/fixtures/BTCUSDT-1m-2024-01-01-first3.csv \
  BTCUSDT ms 100 close 1704067260099000,1704067320049000,1704067320099000
cargo run --locked --offline --manifest-path adapters/mdc_replay/Cargo.toml \
  --bin mdc-replay-run -- step /path/to/new-run 1
cargo run --locked --offline --manifest-path adapters/mdc_replay/Cargo.toml \
  --bin mdc-replay-run -- finish /path/to/new-run
cargo run --locked --offline --manifest-path adapters/mdc_replay/Cargo.toml \
  --bin mdc-replay-run -- status /path/to/new-run
```

`export DIR INDEX REPORT SNAPSHOT` copies a committed bundle to new ordinary
paths; these copies are not a second durable authority. Sync guarantees depend
on the Linux filesystem/device. The opt-in `fault-injection` feature is test-only;
production builds ignore its environment hooks. Standalone writes retain their
014 behavior; source-run recovery is supplied only by RunStore.

## Standalone decision from repository root

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
