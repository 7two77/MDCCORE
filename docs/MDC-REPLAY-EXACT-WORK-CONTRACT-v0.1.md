# MDC-REPLAY-EXACT-WORK-001 — Exact work reduction v0.1

2026-11-06, MDC-IMPL-017. Implements the bounded
[draft](MDC-REPLAY-EXACT-WORK-CONTRACT-v0.1-DRAFT.md). The retained replay v1
format, all default budgets and exact validation obligations are unchanged.

## Owned capture

Production decision construction already exports once to enforce snapshot bounds.
The former bundle path discarded that export, exported again for the report hash,
and exported a third time for bundle bytes. The private CapturedDecision now owns
that original construction export and consumes the view to return identity,
report and the same snapshot bytes. No additional public API accepts a digest or
view/snapshot pair, and no long-lived/cross-cutoff snapshot cache is introduced.
The public DecisionView methods retain their previous behavior and ownership.
The original view is released before restoring the owned bundle snapshot.

## Prepared input

Private PreparedManifest can only decode captured raw manifest bytes, retaining
their exact hash. Binding captured SOURCE validates its digest and normalizes it
into a private PreparedRun. Open checks expected manifest before source binding,
then uses this prepared value for complete prefix validation and for its returned
store. Public validate_prefix independently performs the same complete preparation
from its own raw arguments. It has no caller-supplied "already validated" flag.

Each saved bundle is still fully recomputed, byte-compared and restored. Per-call
outcomes and state/bound checks in SnapshotSession are untouched. HEAD validation,
chain/EOF checks, fault points, lock/process ownership, repair, synchronization,
publication and acknowledgement order are unchanged. Preparation may eliminate
only repeated pure processing of the same already captured bytes.

## Evidence binding and measurements

The A/B tool pins merged 016 `6b33a4375384ed4301695ea879e641a20f5493b9` and
baseline source fingerprint
`cca5b3a70dea13130f8fc25f91152b5fd4dfecf71fabd731eb1cd0f11ae20de4`.
It constructs a clean sparse baseline checkout, verifies all source files and
executes that revision's original validator there. The current validator checks
the candidate against its own source/CI fingerprint. Source/input/profile/limits,
compiler/host, raw artifacts and copied release binaries remain explicit.

Full SOURCE/MANIFEST/HEAD/JOURNAL files must match across variants/repetitions and
native toolchains. Real CLI compatibility evidence starts a nonempty run with the
old binary, then retries/continues it with the new binary. Its final files must
match a new uninterrupted run; the original one-decision prefix is retained.
Separate baseline, candidate and compatibility ZIPs bind all bytes and logs.
Failures retain partial artifacts and INCOMPLETE diagnostics, never successful
pair acceptance. Unknown/mixed fingerprints and modified artifacts fail checks.

Mechanical counts observe actual adapter call boundaries in unit-test builds;
these counters are absent from production binaries and persisted data. They
establish exports 3→1 per bundle, and manifest decode/CSV parse 2→1 per open;
restore/recomputation remains one per certified bundle. Counts alone do not
identify a dominant bottleneck or guarantee a wall-time improvement.

At least three samples per point are retained, with min/median/max. Variants are
grouped sequentially with order reversed across the two toolchains, not randomized;
cache state is uncontrolled. Timings/CPU/RSS are descriptive, not a compiler speed
comparison or production SLO. Both correctness and all measured outcomes must be
reported, including noise or regression. See [implementation](MDC-IMPL-017.md).

The full-day plan remains unsupported: 1440 exceeds the existing plan/visible
bounds, and unchanged journal bytes still consume unchanged storage budgets.
Compaction, larger profiles, pruning, deferred validation and changed checkpoint
trust remain separate contracts.
