# MDC-IMPL-013 — Single-owner automatic flush scheduler

2026-11-04. Base main `ee3c3c6b776fd4276f4fe8171d7b0f2afacc6031` after PR #7.
[Post-merge CI](https://github.com/7two77/MDCCORE/actions/runs/41762158648) passed
14/14 jobs. Main's tree exactly matches reviewed 012 commit 29aa8c2; both original
Git roots and milestones remain preserved. [Receipt](evidence/post-merge-main-ee3c3c6.json).

This package adds optional automatic time/count flushing to mdc_store. Core
algorithms, identity defaults, root Cargo manifest/lock, nested source copy and
MDC-WAL-001/1 bytes are unchanged. No new dependency is introduced.

## API and contract

`ScheduledStore::create/open` require Limits and explicit BatchPolicy:

| Policy field | Validation / meaning |
|---|---|
| max_pending | Nonzero, at most Limits.max_operations; counts new uncommitted core calls |
| max_delay | Positive representable monotonic duration; soft maximum age before attempted flush |
| queue_capacity | 1…1024 queued commands; synchronous admission blocks when full |

There is no global one-second setting or implied loss-window SLA. Invalid policy
is rejected before filesystem creation/open. A Store is opened/created by the
owner, then moved into one worker; no mutable Store escapes. Its Linux flock
remains held until the worker exits. A thread-spawn error after creation does
not authorize silently resetting a now-existing store directory.

`owner.client()` returns cloneable StoreClient. Its synchronous RPCs are submit,
lookup, barrier, checkpoint, snapshot and status. Concurrent clients are accepted
in queue order; arrival order between independent producer threads is not fixed.
The bounded queue limits queued command count, not all caller memory or threads.
An abandoned reply does not cancel a queued mutation. Stable request IDs retain
their role in resolving lost responses and preventing duplicate core effects.

One worker serializes all access to raw Store. Count threshold flush happens
before returning the triggering receipt; a successful receipt then reports
DurableThrough. Existing-ID retries consume no new sequence or pending slot.
Semantic failures which are recorded core calls DO count. Request-ID conflict or
pre-I/O input/budget errors do not fence a healthy store or reset pending age.

The oldest-pending deadline is established before delivering its volatile receipt.
It is not reset by further buffered calls, duplicates or status/lookup traffic.
The worker wakes independently while idle and checks the same deadline between
commands when traffic is continuous. It does not busy-loop or rewrite HEAD when
there are no pending calls. Explicit Mode::Durable/barrier still commits all prior
inputs. Checkpoint remains explicit; successful flush/checkpoint clears pending age.

Time uses std::time::Instant only in the host. It is unrelated to Observation
timestamp or historical available_at. Scheduling, request preparation, a full
queue and blocking sync can delay the attempted flush and response. No real-time
deadline, bounded fsync latency or hard shutdown timeout is claimed.

## Receipts, reads and failure handling

An earlier AcceptedVolatile value does not mutate in the caller's memory after
an automatic flush. Use lookup/status or explicit barrier for current durability.
A durable barrier remains storage evidence, not conformance, lifecycle usability,
order authorization or exactly-once delivery to an exchange.

`status()` returns live_seq, durable_seq, healthy and last_error. When unhealthy,
sequences are diagnostic last-known values rather than a new durable ACK.
`snapshot()` returns owned 011 bytes plus status from one worker position. The
bytes can contain volatile state. It provides no raw pipeline/Store reference.

Autonomous barrier failure records last_error and fences the scheduler. Further
mutations, lookup and snapshot reads fail RecoveryRequired. A successful status
RPC alone is not evidence of health: inspect healthy. Recovery requires shutting
down/dropping the owner, releasing flock, then explicit reopen. No background
retry silently changes the selected committed prefix or repairs a failed store.

The public host Error enum gains InvalidBatchPolicy, SchedulerStopped and
WorkerPanicked; external exhaustive matches need to account for these variants.
Store I/O and semantic core outcomes remain distinct.

`shutdown(self)` processes the queued prefix before Stop, flushes, joins the worker
and returns final status or error. The lock is released before return. Client
clones cannot continue using the stopped worker. Concurrent commands queued after
Stop may fail without execution; clients must resolve uncertain outcomes by ID.
Drop attempts shutdown/join, but cannot report a flush failure or provide an ACK.
Explicit shutdown and checking its Result are required for an observed clean close.

## Validation

Local Rust 1.85.1 / 1.98.0:

- Core: 213 Rust tests + 25 Python checks in each of six feature configurations.
- Host: 36 default-build and 38 fault-injection tests per toolchain, including
  13 new scheduler tests. Format and Clippy with warnings denied pass.
- Idle flush is witnessed by reading HEAD publication without sending another
  worker command. Busy duplicate traffic cannot postpone the oldest deadline.
- Tests cover threshold receipts, explicit strict prefix commit, serialized
  checkpoint, bounded-queue cloned clients, invalid policy, clean shutdown/lock
  release, autonomous failure visibility and error fencing.
- Existing 18 SIGKILL points and raw Store corruption/replay tests remain passing.
- New-process verification now has WAL, checkpoint and scheduled-write cases,
  each with one producer and two consumers. The scheduler case automatically
  flushes at seven pending calls, witnesses live=38/durable=35, then shutdown
  commits the suffix. Plain Store consumers recover the same complete graph.
- The frozen core archive remains SHA-256
  `349f902ecd41fec5da8c158eba913fe8812bace0dcfb9283547c3a250f796928`.
- Four saved hosted 012 stores (two toolchains × WAL/checkpoint) were opened by
  the new consumer: [compatibility receipt](evidence/impl-013-compatibility.json).

[Local evidence](evidence/impl-013-local-verification.json) contains versions,
counts, logs and process receipts. [Source manifest](evidence/impl-013-source-manifest.json)
binds the entire core/host code and framing vectors. Hosted execution is recorded
separately; the local working copy was dirty before commit and is identified as such.

## Remaining boundaries

The raw Store API still has explicit flush only; scheduling is opt-in through
ScheduledStore. There is no automatic rotation, compaction, multi-writer filesystem
protocol, async cancellation or network service. Total history/resource limits
of 012 still apply; a bounded queue does not make a perpetual history unboundedly
safe. Production resource/throughput/SLO validation remains open.

Tests cover process termination and software I/O failures. Real power-loss
guarantees still depend on the filesystem/controller/device sync stack. No physical
power-cut validation is inferred from timers or SIGKILL. External source cursors,
strategy/broker state, reinstatement and historical availability are not added.

The next trading-consumer requirements remain data source/schema, useful replay
outputs and available_at/decision_at semantics. Storage scheduling must not be
treated as protection from lookahead in a market backtest.
