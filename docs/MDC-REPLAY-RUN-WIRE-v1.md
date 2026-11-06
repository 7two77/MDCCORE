# MDC-REPLAY-RUN-001/1 — Durable isolated replay decisions

2026-11-06, MDC-IMPL-015. Binary realization of the
[recovery proposal](MDC-REPLAY-RECOVERY-CONTRACT-v0.1-DRAFT.md).
Scope: bounded Binance modeled final bars, Linux local filesystem, one writer.
Existing MDC-SNAPSHOT-001/1 and MDC-WAL-001/1 are unchanged. These run files are
not mdc_store files and must not be opened by that crate.

## Common framing

Numbers are LE with checked lengths/conversions. A frame is
`magic[16], version:u16=1, body_length:u64, body, SHA256(domain || preceding bytes)`.
Hash covers the complete 26-byte prefix and body. `blob` is u64 length plus exact
bytes. Unknown versions/tags, over-budget lengths and trailing bytes in complete
frames/bodies are errors. Hashes are raw 32-byte values; human hex is lowercase.
Checksums do not attest source/provider authenticity.

| Kind | Magic literal (16 bytes) | Domain |
|---|---|---|
| Manifest | `MDC-RMAN-001\0\0\0\0` | `MDC-REPLAY-RUN-001/manifest/v1\0` |
| Journal header | `MDC-RJNL-001\0\0\0\0` | `MDC-REPLAY-RUN-001/journal/v1\0` |
| Decision | `MDC-RDEC-001\0\0\0\0` | `MDC-REPLAY-RUN-001/decision/v1\0` |
| HEAD | `MDC-RHED-001\0\0\0\0` | `MDC-REPLAY-RUN-001/head/v1\0` |

Files: LOCK, SOURCE (raw CSV), MANIFEST, JOURNAL, HEAD. HEAD.pending is temporary.
Only HEAD certifies a prefix; extra complete journal records do not advance it.
Pathnames/mtime/PID do not enter wire or core identity. Control files are regular,
opened without following symlinks. A canonical run directory has one nonblocking
exclusive Linux flock held through its open LOCK descriptor.

The guard belongs to its creating process. Owner drop explicitly unlocks before
close; a child inheriting a RunStore across fork must reopen, and cannot use or
unlock its parent's instance through the public API.

## Manifest

Body: `snapshot_engine:u16=1, consumer:u16=1, identity:u8=1,
source_bytes:u64, source_sha256[32], profile, limits, cutoff_count:u64, cutoffs[]`.
Identity 1 selects PortableV1; consumer 1 selects unchanged 014 modeled semantics.
Profile is symbol u8 (BTC=0, ETH=1), input-unit u8 (ms=0, us=1), anchor u8
(reported-close=0, minute-end=1), lag_us u64. Lag must be an integer number of ms
and satisfy the 014 final-bar boundary. SOURCE is captured once, parsed atomically
and bound by exact byte length/hash; availability remains MODELED.

Limits are six u64 values: max_decisions, max_manifest_bytes, max_record_bytes,
max_journal_bytes, max_report_bytes, max_snapshot_bytes. Each is positive and at
most its v1 default: 256, 16384, 25165824, 67108864, 1048576, 16777216 respectively.
Manifest cap includes framing; record cap is its body; journal cap is the entire
certified prefix. Cutoffs are nonempty, unique, strictly increasing UTC-us.

Fixed consumer limits: 4 MiB source, 4096 rows, 4096 bytes/line, 256 visible bars.
Fixed 011 limits: field 2 MiB, collection 16384, total items 65536, definitions 4096,
calls 4096, state manifest 8 MiB, and selected snapshot cap. The maximum cutoff is
preflighted for eligibility/call bounds. No plan extension or silent truncation.

Run ID is `SHA256("MDC-REPLAY-RUN-001/run-id/v1\0" || manifest_body)`.
Manifest file hash is SHA-256 of the entire framed file, distinct from run ID and
its trailing frame digest. Run ID is an audit namespace, excluded from 014 IDs
and Registry context. Different full inputs/plans may retain identical earlier
eligible core bytes while their audit identities differ.

## Journal and HEAD

Header body: `snapshot_engine:u16=1, run_id[32], manifest_file_hash[32]`.
Exactly one header starts JOURNAL. The first record chains from its frame digest.

Decision body, in order: `run_id[32], index:u64, cutoff_us:u64, decision_id[32],
report_sha256[32], snapshot_sha256[32], previous_frame_digest[32], report:blob,
core_snapshot:blob`. Indices are contiguous from 1 and match the plan. Report is
exactly 014 report_json plus LF; core snapshot is exact engine-011 export. Every
decision uses a separate eligible session. Blob hashes and recomputation agree.

Bundle digest is `SHA256("MDC-REPLAY-RUN-001/bundle/v1\0" || decision_body[0..144])`.
This binds run/index/cutoff/decision/report/snapshot hashes. The record frame
additionally binds framing, blobs and previous frame digest.

HEAD body: `run_id[32], manifest_file_hash[32], committed_bytes:u64,
durable_count:u64, tip_frame_digest[32], latest_bundle_digest[32]`.
It is one complete frame at exact EOF (202 bytes). Empty HEAD selects header
length 124, count 0, header frame digest and zero latest bundle digest. Otherwise
it selects exactly records 1…count, ending offset and final record/bundle hashes.
There is no independent commit marker or generation selection in v1.

## Publication and recovery

Create validates source/plan/bounds before directory creation, then syncs parent,
SOURCE, MANIFEST, journal header and directory. It publishes empty HEAD through
HEAD.pending write/sync, same-directory rename and directory sync. Existing or
partially created run directories are never silently overwritten/recreated.

RunStore::step(index) computes the isolated bundle and checks budgets before I/O,
appends/syncs the record, then publishes/syncs HEAD. Successful return acknowledges
the prefix through that index, conditional on filesystem/device sync guarantees.
Committed-index retry returns the exact retained bundle without append; skipped
or out-of-range indices error. Completed next_index is None. NO_INPUT, UNRESOLVED
and PASS are technical outcomes, distinct from durable acknowledgment.

Post-mutation I/O errors return Indeterminate and fence access until reopen.
Pre-I/O validation/limit errors leave the old prefix usable. Lost replies resolve
by reopening and retrying the same run/index. Public validate_prefix checks
logical certification in memory; it is not a filesystem durable ACK.

Open holds the lock, validates source/manifest/HEAD and reads only the bounded
certified range. It checks all record bindings and 011 restoration plus production
014 recomputation. Full report/snapshot bytes must match. Optional expected
RunManifest must agree before repair; CLI continuation uses captured input rather
than rereading the original source path. Changed CSV/model/plan/limits need a new run.

After validation, open truncates uncommitted suffix, removes HEAD.pending, and
always syncs journal and directory before exposing results. This also seals a
previous interrupted truncation or indeterminate HEAD rename. Corrupt/missing
committed data, source/manifest/HEAD or unknown versions fail without fallback.
No longest-file/mtime/temporary-file heuristic is a recovery authority.

## Controls and exclusions

Independent Python struct/hashlib vectors in
`adapters/mdc_replay/tests/fixtures/replay-run-wire-v1.tsv` fix manifest, run ID,
empty header and HEAD. Native continuous/restarted runs compare every full file
and exported bundle. Python independently parses frames and decision IDs and
checks the unchanged 014 late-witness control. Wire tests execute on native,
i686 and s390x; filesystem runtime verification is native Linux.

No rotation/compaction, volatile decision mode, plan append, live source, measured
receipt, strategy or order delivery. Recovery/reconstruction has bounded but
potentially high cost; no hard allocator/time SLA is claimed. SIGKILL tests do
not establish physical power-cut tolerance or universal hardware durability.
