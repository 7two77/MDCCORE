# MDC-TEMP-001 — Historical availability and first replay scope

2026-11-04. REQUIREMENTS DRAFT / FIRST MODELED SLICE IMPLEMENTED IN MDC-IMPL-014.
Base: main 1f0b76a after MDC-IMPL-013. Historical replay is the selected first
consumer. This document prepares its requirements; it is not a market strategy,
a new edition of STP-MDC, or a claim that current E2E establishes causal backtesting.
VAIXLNS-Builder and MA-000 are not imported as MDC normative requirements.

## Selected first slice

The owner subsequently proposed Binance Spot/Deribit, BTCUSDT/ETHUSDT, 1m OHLCV,
CSV/Parquet and modeled latency. MDC-IMPL-014 selects Binance Spot / BTCUSDT /
1m / CSV and isolated per-decision views. Its executable boundaries are in the
[profile contract](MDC-BINANCE-REPLAY-CONTRACT-v0.1.md). This is MODELED availability,
not observed receipt history or a complete implementation of TA01…TA12.
The earlier design exploration below is retained as requirements context; statements
about unselected options apply to that preparation, not to the new bounded adapter.

## Input questions used to scope the first profile

One concrete dataset/profile is needed: provider/exchange, instrument, event kind
(trades/quotes/bars/other), file/schema and timestamp fields/units. No exchange,
BTC instrument, bar interval or CSV column layout is chosen on the owner's behalf.
The owner has been asked for these details. A small reproducible fixture can then
be defined from that format, without inventing a trading pattern or signal.

Availability evidence also matters: does the source provide a publication/receipt
timestamp, or only event/open/close time? If actual availability is absent, a
declared inferred policy may be useful, but cannot be labelled verified historical
availability. A downloaded final OHLCV bar is not known at its opening time.

## Confirmed current boundaries

Current ObservationRequest has source, optional timestamp, scalar payload and
acquisition_context. DomainContext supplies a versioned time basis. There is no
available_at/decision_at eligibility or receipt schema. Root scalar schemas are
SignedI64Le; arbitrary OHLCV, decimal normalization and float schemas do not exist.

State/Transition use observation_horizon over source timestamps. In particular:

- `src/pipeline/dynamics.rs::observation_facts` collects stored usable observations.
- `src/model/state.rs::observation_after` filters by source/version, timestamp,
  horizon and scope, not historical publication/receipt availability.
- Transition's ObservationAfterTarget uses the same witness mechanism.

Thus guarding only explicit input refs is insufficient if a future-but-stored
observation can be selected implicitly. Scheduler monotonic time and lifecycle
revision do not supply the missing market-availability semantics.

WAL/snapshot preserve accepted core history. They do not currently atomically
persist an external replay cursor, input-file identity or broker/strategy state.
Stable request IDs help retries but alone do not prove correct source resumption.

## Proposed temporal requirements

| ID | Requirement / design boundary |
|---|---|
| T01 | Distinguish event time, information availability, decision cutoff and host processing time. Define comparable clock basis, epoch, unit and normalization version |
| T02 | Missing/uncertain availability must not silently become event_at or a semantic Pass. Exclusion/Unresolved/admission diagnostics must be fixed in the executable contract |
| T03 | Admit to a decision's visible set only evidence available by its cutoff; use available_at <= decision_at only with declared timestamp and tie-order semantics |
| T04 | Enforce visibility for explicit refs AND implicit witness selection, recursive lineage, State, Transition and Evaluation; filtering after derivation is too late |
| T05 | For bars, distinguish provisional updates from finalized bars and the end/publication boundary. For prices/volume, define exact scalar encoding, units and overflow rules from the chosen input |
| T06 | Corrections are appended with record/revision identity and availability; they do not overwrite old Observation/State/Evaluation snapshots or repair old missing inputs |
| T07 | A later-full graph cannot be used for an earlier cutoff without a proven isolated view. Backdated queries require explicit handling, not reuse of current Store state |
| T08 | Equal timestamps need a declared source/receipt/decision order. Ambiguous sub-order must be diagnosed; source-file order cannot be silently assumed to be physical availability order |
| T09 | Bind evidence to stable source row/revision, raw dataset hash, normalization/config version and time basis. Preserve the evidence in the replay artifact/provenance contract, not only in a transient filter |
| T10 | Define source resumption against committed core prefix: source identity, cursor or repeat-from-start + stable-ID mapping, and behavior when source bytes/revisions change |
| T11 | Version temporal context/identity and persistence changes explicitly. Preserve existing scalar/identity/snapshot/WAL profiles; no reinterpretation of old artifacts |
| T12 | Replaying a fixed input/config in the declared order is deterministic; adding late evidence must not change earlier decision snapshots/results under the same cutoff |

These are proposed requirements for the new profile, not a finding of violation
of old MDC contracts. Exact wire/API fields and result/error mappings remain
unfrozen until the source and its available evidence are known.

Separate post-hoc dataset integrity from information available to the decision.
A full-file hash can identify audit inputs without being a historically known
strategy input. Adding an unavailable suffix must not silently rebind old artifacts
through a changed global-file hash. If a profile intentionally versions the whole
dataset/run and therefore changes IDs, the cutoff-independence check compares the
declared eligible semantic content while retaining original artifacts untouched.
The identity/manifest boundary must be stated before claiming cross-run ID equality.

## Implementation options to evaluate

**Monotonic availability replay:** order and admit source receipts by the declared
availability key, insert decisions at their own ordered cutoffs, and keep future
receipts out of the core graph. This works only if backward decisions are rejected
or isolated, and the timestamp-tie rule is complete. Sorting by event_at alone
is insufficient for late arrivals/corrections.

**Isolated per-decision view:** build a bounded fresh decision graph from only its
eligible receipt versions, or add a versioned visibility mechanism covering all
core selectors. This can support historical cutoffs, but must prove completeness
of visibility enforcement and preserve creation-time snapshots.

The preparation did not select an option. MDC-IMPL-014 now selects the isolated
bounded view for its final-bar modeled profile. Tests deliberately place a late observation
in storage that would satisfy ObservationAfter by event timestamp; it must still
be unavailable to the earlier decision.

For derived artifacts, availability cannot precede required inputs. Operator/
model/config knowledge must also be declared if time-varying. The current frozen
scalar Registry may be assumed fixed for a narrowly stated fixture; it is not
proof that future fitted models or parameters were known historically. Offline
processing does not reproduce real execution latency without an explicit model.

Current lifecycle and historical availability are distinct. A later withdrawal
can invalidate current use while old snapshots/Evaluation remain historical.
The new profile must say how as-of information and lifecycle revisions interact;
it must not rewrite earlier artifacts or label old as-of knowledge as present truth.

## Planned acceptance cases (not execution evidence)

| Case | Required observation |
|---|---|
| TA01 | Fixed eligible source fixture builds through public core APIs with explicit data/clock/config provenance |
| TA02 | Available just before/at/after cutoff follows the declared equality and sub-order rules |
| TA03 | Unknown or incomparable availability is diagnosed without event-time fallback |
| TA04 | Event timestamp is in horizon but publication is late: excluded from earlier State/Transition witnesses |
| TA05 | Hidden future observation in storage cannot leak through implicit selection or recursive lineage |
| TA06 | Provisional/final bar variants follow the selected source contract, not opening-time assumptions |
| TA07 | Late correction affects only later eligible contexts; old bytes, identity and snapshots remain unchanged |
| TA08 | Timestamp ties and permuted reading of fixed source fragments follow the unchanged manifest and declared ordering key |
| TA09 | Derived output is not eligible before its required data/config knowledge |
| TA10 | Restart/retry resumes the correct source and committed decision boundary without duplicate core effects |
| TA11 | Modified dataset/schema/config is detected instead of silently continuing a different history |
| TA12 | Adding a late suffix leaves all earlier cutoff results unchanged; double-process replay agrees |

Negative harness success means the specified rejection/exclusion/Unresolved result
is observed, not that every semantic case returns Pass. An inferred latency
fixture can establish conformance to its assumption; it cannot prove the real
provider published the historical data at that time.

## Next concrete slice

After source selection: freeze its normalized row/envelope, time and availability
evidence quality, correction/tie rules, identity/version boundary and restart
policy. Define one bounded technical consumer fixture using production core APIs;
add market State/Transition expectations only when their domain meaning is agreed.
Then implement the chosen visibility/replay mechanism and TA01…TA12. No new
causal replay PASS, trading result or domain profitability is claimed at this stage.
