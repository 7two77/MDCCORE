# MDC-CAPACITY-001 — Replay resource characterization, draft v0.1

2026-11-06. Proposed MDC-IMPL-016, based on verified main ee4d45f after 015.
Historical proposal. Implemented by [MDC-IMPL-016](MDC-IMPL-016.md);
current [contract](MDC-REPLAY-CAPACITY-CONTRACT-v0.1.md). The following text
retains the original pre-implementation proposal.
The next slice measures resource use before changing scale or retention. It
does not authorize loss of history or invent a production SLO.

## Purpose and scope

RunStore retains a full isolated snapshot per decision and revalidates the prefix
during open. A CSV that parses does not imply all requested decisions fit journal,
visible-history or snapshot limits. The [pilot](MDC-OPS-006-POST-RECOVERY-v0.1.md)
completed 64 cutoffs but hit the journal cap after 153 in a 256-cutoff plan.
Those observations motivate a repeatable series, not a general capacity formula.

First candidate input is one official BTCUSDT Spot 1m day (1440 rows), an engineering
measurement scope rather than an owner-specified SLO. Current v1 cannot execute
1440 full-history decisions. Do not reset history, select a rolling window, change
cutoffs or raise budgets to make a run appear successful.

## Required measurement record

- Exact commit/tree/fingerprint, binary SHA, build profile and selected flags;
  rustc -vV, Cargo, target, OS/kernel, CPU and filesystem information where available.
  Do not dump credential environment values.
- Input provider/profile, archive/raw CSV SHA, byte/row count, declared units,
  modeled lag/anchor and retained exact cutoff plan or canonical plan bytes.
- All effective source, plan, visible, core, record and journal budgets.
- Repetition, phase, exit/typed outcome, timeout, cache conditions, clock method,
  process launch/polling overhead and artifact digests.

Official input is an explicit local file with checked provenance. A small CI smoke
can use frozen or clearly labeled synthetic data, without claiming synthetic rows
are independent market evidence. No implicit network acquisition in correctness jobs.
Derive eligibility/cutoffs through production normalization/model APIs, not a second
Measurement/State implementation. Full-file audit identity remains outside earlier
eligible core identity.

## Series and metrics

Proposed native release series: 1, 16, 32, 64, 128 and 256 requested cutoffs using
the same captured day/model. Include existing before/equal/after-cutoff controls.
Record 1440 as unsupported under unchanged limits, distinguishing CLI argument
size from plan/visible-history bounds. No failed sample is silently removed.

At least three saved repetitions per point; report each plus min/median/max.
Three samples do not justify p95/p99. Both supported native Rust versions must
agree on correctness/bytes. Timing across unlike hosts needs separate labels.

| Phase / metric | Required distinction |
|---|---|
| Validate/create | Capture/parsing vs existing-run recovery |
| Continuous commit | One open writer; no hidden reopen per decision |
| Fresh-process recovery | Certified prefix size/count and declared cache state |
| Retry/failure | Old-index deduplication vs new recorded work |
| Capacity | Requested/committed/next index and first limiting budget |
| Resources | Per-process user/system CPU and peak RSS, units and baseline |
| Storage | Logical source/manifest/head/journal and per-report/snapshot lengths |

Host phase instrumentation uses monotonic time outside canonical bytes. Parent
duration includes launch/wait overhead; RSS is not allocator accounting or a hard
bound. Logical file size is not necessarily physical allocated storage. Do not
flush host caches or call a fresh process a controlled physical cold start.

## Correctness and failures

Existing portable graph, snapshot, 014 and 015 controls must remain unchanged.
For comparable completed prefixes, compare full reports/core bytes across continuous
and restarted runs and verify source/manifest binding. A logical limit is an
expected rejection/retained prefix, not semantic Pass or completion of the full request.

Capture the prefix immediately before a failing step; pre-I/O budget rejection
must not advance HEAD or append. Reopen/lookup verifies acknowledged decisions.
Timeout/lost response remains indeterminate until recovery identifies the certified
prefix. Recovery has its own measurement budget; an unfinished sample remains
unmeasured, without guessing rollback or success.

No automatic deletion, window selection, older-HEAD fallback, availability change
or relaxed semantic recomputation is part of this harness.

## Acceptance plan (not execution evidence)

| Case | Required result |
|---|---|
| CP01 | Saved input/build/host/limits manifest identifies the exact sample |
| CP02 | Release control matches frozen report/core bytes |
| CP03 | Fresh run per repetition; source/plan/budgets remain explicit |
| CP04 | Commit and fresh-process recovery measured separately |
| CP05 | Comparable full bytes match across repetitions/toolchains/restarts |
| CP06 | Limit records counts/first budget; retained prefix verifies |
| CP07 | Unsupported daily request is rejection, not incomplete success |
| CP08 | Timeout/lost response resolved or remains explicitly indeterminate |
| CP09 | CPU/RSS/storage units and clock precision match actual process/files |
| CP10 | Missing/duplicate/stale/tampered artifacts fail validation |
| CP11 | Summary retains every sample/failure; timing noise is not semantic failure |
| CP12 | Existing regressions pass; no limit/identity/wire-format change |

## Delivery and later decisions

Implement a host driver/runner and independent artifact validator. Correctness
smoke belongs in regular CI; larger series is opt-in with bounded time/storage
and retained artifacts. Avoid unstable shared-runner latency gates without an SLO.

Then choose an evidenced next change: eliminate redundant work preserving exact
bytes, define a larger bounded profile, or specify a new retention/checkpoint
protocol. Rotation alone does not reduce retained bytes or prove equivalent
semantic validation. Dropping views, shortening a window or trusting checkpoints
changes obligations and requires its own contract.

Open product constraints for that later decision: history length/decision frequency,
recovery-time objective, hot-storage budget, and immediate availability of all old
full views. No numeric production targets or history-loss policy are invented here.
