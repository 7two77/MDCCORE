# MDC-CAPACITY-001 — Resource characterization v0.1

2026-11-06, MDC-IMPL-016. Implements the
[proposal](MDC-REPLAY-CAPACITY-CONTRACT-v0.1-DRAFT.md) on the unchanged 015
bounded Dataset/RunStore and recorded-session formats. This is an engineering
measurement contract, not a trading strategy or a new capacity guarantee.

## Execution and retained authority

`mdc-replay-capacity` is a separate Linux measurement binary. Its `create` command
captures/parses a bounded CSV and derives an increasing plan from production
`Dataset::evidence().available_at()`, then invokes `RunManifest::new` and
`RunStore::create`. It does not recreate the availability/State algorithms.
`commit` opens once and calls `step` continuously. `recover` opens in a new
process and inventories each committed bundle. `retry` calls an old index and
checks that HEAD and journal length are unchanged. All output is diagnostic JSON;
only the existing RunStore certification protocol acknowledges durability.

The Python runner creates a new run per point/repetition. Its report binds the
exact input, profile, cutoff plans, effective v1 limits, source/CI fingerprints,
checkout revision/tree/dirty status, release command/flags, copied executable
digests, compiler, compiled target, host CPU/kernel/filesystem and process logs.
A dirty local execution is labeled; hosted comparison requires a clean matching
checkout/run. Metadata and checksums are evidence binding, not signatures.

No new defaults: 64 MiB journal, 256 plan entries and visible bars, original
record/report/snapshot/core call bounds. The optional reduced-journal probe uses
an already supported smaller `RunLimits` value and labels it explicitly. It never
raises a budget or substitutes a shorter window. Schema/identity/store algorithms,
retention, isolated views and recomputation are unchanged.

## Measurements and labels

The default series is 1,16,32,64,128,256,1440 cutoffs, three independent saved runs
per point. The same captured day/model is used throughout. The current manifest
rejects 1440 entries before creating a run. That result is `PreflightRejected`;
it does not claim a full-day replay was completed. Source insufficiency or parse
failure is a different unsuccessful measurement, not that expected rejection.

Rust `Instant` times input preparation, manifest validation, create/open, each
step and retry separately. Diagnostic bundle hashing is outside recovery API time.
Python `perf_counter_ns` records whole child duration, including launch and up to
5 ms polling. Linux `wait4` records user/system CPU in integer microseconds and
process peak RSS in KiB. RSS includes process baseline; it is not live-heap or
allocation accounting. File sizes are logical bytes; allocated disk usage and
physical writes are not inferred. Fresh processes use uncontrolled OS caches;
no claim of physical cold recovery is made and host caches are never flushed.

All individual samples are retained; summaries include outcomes and the first
constraint, with min/median/max and underlying samples. Three repetitions do not
support p95/p99. No latency threshold is imposed on shared hosts or unlike machines.
CPU/RSS/wall data are process-level; `commit_api_ns` sums only step API intervals.

Default per-phase deadline is 120 s (configurable 0.05..600 s), output budget
1536 MiB (128..4096 MiB). New samples require room for an entire bounded run plus
slack. No existing evidence is pruned to meet that budget. Build has its own 600 s
budget. Invalid parameters/old output paths fail without overwriting prior results.
The runner owns/reaps each child; a timeout kills only its new process group.

## Limits, failures and acceptance

A pre-I/O step limit records the failing index/error, compares HEAD bytes and
journal length immediately around that step, then independently reopens the
retained prefix. An old-index retry must leave certification unchanged. A lost
commit response or timeout triggers a separate recovery attempt; the certified
prefix determines what was saved. A missing/failed recovery is unmeasured.
Timeout, unexpected error, missing sample or exhausted measurement storage leaves
`INCOMPLETE`, even if an available partial prefix is retained successfully.

A complete measurement matrix may include expected `Limited` and
`PreflightRejected` samples. Acceptance means the requested observations were
executed and verified, not that all requested decisions were committed, not a
semantic PASS, and not a performance SLO.

The independent validator reads bounded v1 frames, validates manifest/source/model,
HEAD/record chains, cursor, bundle/report/snapshot hashes and eligibility. The
production fresh open additionally restores/recomputes semantics. Retained full
SOURCE/MANIFEST/HEAD/JOURNAL files must agree across repetitions and native Rust
versions for the same configuration. No alternative graph implementation is used.
Duplicate/missing cells, stale source/CI identity, modified binaries/logs/run files,
wrong units/clocks, biased summaries and incomplete executions are rejected.

## Delivery and next decisions

Regular CI uses synthetic data, points 1,2,3,1440 × three, and a second 32768-byte
journal probe that must retain a nonempty limited prefix. It never fetches market
data. An opt-in workflow supports the full series and explicitly selected official
2024/2025 days. Acquisition checks the pinned remote checksum, ZIP/member bounds
and captured CSV hash; modeled lag is never represented as historical receipt.

CP01…CP12 map to executable evidence in [MDC-IMPL-016](MDC-IMPL-016.md).
Any optimization, larger profile, changed retention/checkpoint authority or SLO is
a later contract. A faster measurement alone does not authorize lost history or
relaxed semantic validation.
