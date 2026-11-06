# MDC-REPLAY-STORAGE-002 — Proposed bounded lossless storage v0.1

2026-11-06. Proposed MDC-IMPL-018 after merged 017 main `ac364f8`.
Status: CONTRACT DRAFT / RUNTIME NOT IMPLEMENTED.

## Objective and selected boundary

Retain every decision, exact report bytes and exact engine-011 snapshot bytes,
while reducing physical journal size with independently encoded blobs. Recovery
still restores/recomputes the entire certified prefix. New physical bytes and
audit run IDs are versioned separately from unchanged logical decision/core IDs.

Add an explicitly selected Linux v2 store/profile. Existing RunStore, defaults,
errors, v1 files/readers/writers and CLI behavior remain unchanged. No in-place
conversion or automatic format switch. Root no_std, mdc_store, normalization,
MODELED availability, eligibility and snapshot semantics stay unchanged.

The [offline size probe](evidence/lossless-codec-size-probe-001.json) verified
153 real records from the pinned 017 official-day study. Their report/snapshot
payload totals 67,053,084 bytes; snapshots account for 60,873,345 bytes. Independent
Python zlib level-6 streams total 11,449,197 payload bytes and decode exactly.
This excludes new framing/control files and is not a Rust implementation, latency
benchmark or promised compression ratio for other data. Levels 1/9 are retained
as observations; level 6 is the proposed writer setting, not a proved CPU optimum.

## Logical and physical identities

SOURCE remains the exact captured CSV. Each decoded report is the original 014
report_json plus LF; each decoded snapshot is the original 011 export. Their
SHA-256, decision ID, core identities, outcomes/statuses, context and lineage must
match v1 for the same source/model/cutoff, independent of storage encoding.

v2 MANIFEST/JOURNAL/HEAD and frame digests necessarily differ from v1. A v2 audit
run ID binds source, plan, consumer/engine, limits and storage profile under a new
domain. It never enters Registry context or earlier decision identity. Do not
claim equality of physical v1/v2 files; compare decoded bundles and cursor/history.
There is no audit alias that silently treats an old v1 run as a v2 run.

## Proposed wire grammar

All wrapper integers are LE, checked before conversion/allocation. Compressed
stream internals follow RFC1950/1951, including their own bit/byte ordering.
Frame layout remains
`magic[16], version:u16=2, body_length:u64, body, SHA256(domain || prefix || body)`.
Magic values are `MDC-RMAN-002\0\0\0\0`, `MDC-RJNL-002\0\0\0\0`,
`MDC-RDEC-002\0\0\0\0`, `MDC-RHED-002\0\0\0\0`.
Domains are `MDC-REPLAY-RUN-002/<manifest|journal|decision|head>/v2\0`.
Unknown versions/tags and trailing bytes in complete frames/bodies are errors.

Manifest body: engine:u16=1, consumer:u16=1, identity:u8=1, source_length:u64,
source_sha256[32], existing 11-byte 014 profile, eight u64 limits in the table's
order, codec_profile:u8=1, writer_level:u8=6, cutoff_count:u64, cutoffs:u64[].
Run ID is SHA256(`MDC-REPLAY-RUN-002/run-id/v2\0` || manifest_body).
Header body is engine:u16=1, run_id[32], manifest_file_sha256[32].

Decision body: run_id[32], index:u64, cutoff:u64, unchanged decision_id[32],
previous_physical_frame_digest[32], report_blob, snapshot_blob. Each blob is
`tag:u8, decoded_length:u64, encoded_length:u64, decoded_sha256[32], encoded_bytes`.
Tag 0 is exact raw bytes; tag 1 is one RFC1950 zlib/DEFLATE stream. A raw blob's
encoded/decoded lengths must agree. No external/preset dictionary, concatenated
streams or unexplained trailing bytes are accepted. A decoder must terminate at
exact encoded EOF, produce exactly decoded_length and verify its SHA-256.

HEAD body retains explicit run_id, manifest_file_sha256, physical_offset:u64,
count:u64, physical_tip_digest[32], decoded_bundle_digest[32]. The latter hashes
run_id/index/cutoff/decision_id/report_sha256/snapshot_sha256 under
`MDC-REPLAY-RUN-002/decoded-bundle/v2\0`. HEAD selects one contiguous physical
prefix. Its offsets never refer to decoded sizes or temporary decoded caches.
Independent little-endian vectors must freeze the exact layouts before runtime
implementation; this draft is not a deployed file format.

## Compression profile

One independent stream per report/snapshot; no cross-record dictionary or delta
reference. The writer selects compressed data only if it is strictly shorter;
ties use raw. Level/window/strategy and the pure Rust backend version/features
must be pinned in the implementation/lockfile and tested on supported targets.
A pure Rust RFC1950 backend is the candidate; no native C/C++ dependency or core
codec change is proposed. Python probe bytes are not Rust writer golden vectors.

Writer output must be deterministic for the pinned profile across supported
builds, with separate vectors. Reader correctness is defined by valid bounded
streams, hashes and exact decoded semantics, not re-compression equality with a
particular library. Physical checksums certify actual encoded bytes; they are
not source authenticity signatures or logical semantic identity.

## Proposed experimental v2 budgets

These are engineering profile proposals, not business SLOs or changed v1 defaults.
The new aggregate bound explicitly increases permitted decoded history only in
the opt-in v2 profile; it must not be disguised as the old 64 MiB logical envelope.

| Encoded manifest limit order | Proposed ceiling | Meaning |
|---|---:|---|
| max_decisions | 256 | Existing plan bound retained |
| max_manifest_bytes | 16 KiB | Entire framed file |
| max_encoded_record_bytes | 24 MiB | Physical decision body |
| max_physical_journal_bytes | 64 MiB | Entire certified encoded journal |
| max_report_bytes | 1 MiB | One decoded report |
| max_snapshot_bytes | 16 MiB | One decoded 011 snapshot |
| max_decoded_record_bytes | 24 MiB | Sum decoded blobs plus 192-byte logical envelope |
| max_total_decoded_blob_bytes | 256 MiB | Checked sum of decoded report+snapshot lengths through certified prefix |

Smaller explicit positive values may be selected and retained in the manifest.
SOURCE 4 MiB, rows/line 4096, visible bars 256, root calls 4096 and other current
011 per-view limits remain fixed. Encoded, decoded, aggregate and operation
budgets are checked independently before allocating/decoding or advancing HEAD.
A short compressed input is not permission for unbounded decoded work.

## Memory, recovery and API

Keeping every decoded bundle in a Vec would defeat the proposed memory boundary.
v2 must stream one bounded record at a time during open, retain a bounded index
of certified offsets/lengths/digests, and release transient views before the next
record. It must not materialize the entire decoded prefix or trust a checkpoint
instead of restoring/recomputing each bundle. Root per-operation bounds stay active.
Memory scales with SOURCE, the bounded index, one record/view and core workspace;
a hard RSS/RTO target still requires measurement, not this complexity statement.

Use a separate owned-result interface, e.g. `RunStoreV2::lookup_owned`, rather
than changing v1's borrowed lookup. Lookup reads only its certified frame, checks
it against the index, decodes and verifies returned bytes. Reuse of completed
semantic validation is limited to the identical frame from this successfully
validated immutable prefix; no cross-run/cutoff semantic cache is permitted.
An owned operation may require mutable access for I/O/error fencing. No unbounded
hidden decoded cache is added. Final Rust signatures must document this choice.

Commit builds and semantically validates the decoded bundle first, encodes under
bounds, checks physical and decoded aggregate budgets, then follows the existing
append/sync/HEAD.pending sync/rename/directory-sync protocol. Only a successful
seal acknowledges the prefix. Pre-I/O limit failure leaves HEAD/journal unchanged.
I/O/indeterminate/certified-read corruption fences further access until reopen.
Old-index retry returns the same decoded bundle without append.

Open acquires the single-owner lock and validates source/manifest/HEAD, all physical
frames, decoded lengths/digests and recomputed/restored bundles before tail repair
or exposure. No older-HEAD fallback. Sealing, fork ownership, symlink/regular-file
checks and the conditional filesystem/device durability boundary are preserved.
Lazy owned lookup is a memory interface, not deferred semantic recovery.

v1 remains readable/continuable through its existing implementation. Any later
migration creates a new explicitly selected run and proves every decoded bundle;
it does not rewrite or delete the original. Migration itself may be deferred from
018; no implicit migration is part of successful v2 open.

## Acceptance scenarios

| Case | Required result |
|---|---|
| LS01 | Independent v2 frame/manifest/HEAD/blob vectors bind all tags, lengths, domains and limits |
| LS02 | Raw/compressed blobs decode exactly; zero/small/incompressible inputs obey explicit raw-fallback rules |
| LS03 | Pinned writer vectors agree across native toolchains and supported cross targets |
| LS04 | Truncation, extra streams/bytes, dictionaries, unknown tags/versions and malformed checksum streams reject |
| LS05 | Claimed/actual output excess, integer overflow and aggregate budget violations reject before unbounded allocation |
| LS06 | Correctly checksummed but semantically forged decoded reports/snapshots still fail recomputation/restore |
| LS07 | v1/v2 decoded reports/snapshots/IDs/lineage/statuses match for every tested cutoff; audit identities are version-distinct |
| LS08 | Different decoded bundles/contexts never alias through storage encoding or owned lookup |
| LS09 | All old v1 controls/files/API/error behavior remain unchanged; no in-place conversion |
| LS10 | Stream recovery retains bounded index/workspace; old views remain exactly retrievable after restart |
| LS11 | Old-index retry and failed physical/decoded limits leave the certified prefix unchanged |
| LS12 | Creation/append/sync/rename/recovery-tail/lost-reply failure matrix preserves one-owner fencing and acknowledgement |
| LS13 | Modified certified frames after open are detected by owned reads and fence continuation |
| LS14 | Every selected physical/decoded/aggregate limit and cursor is visible in diagnostics and validation |
| LS15 | Paired official studies retain every sample and compare decoded bundles, not physical v1/v2 hashes |
| LS16 | Codec CPU/RSS/size/recovery results and failed limits are measured on pinned binaries, with no inferred compression SLO |
| LS17 | Source audit confirms root/no_std/store/v1 defaults and normalization/availability unchanged; dependency changes are host-only and pinned |
| LS18 | Documentation distinguishes experimental 256-decision v2 storage from unsupported 1440-decision daily profile |

## Delivery and later requirements

Implement codec/vector checks first, then the streaming reader/owned API, strict
writer/recovery and fault matrix, then the official size/resource comparison.
A 256-cutoff official plan is a useful engineering target, subject to all stated
budgets and measurement; do not raise a budget to turn its rejection into success.

1440 decisions still require a separate explicit plan/visible/core-work profile
and resource study. For that later profile the recovery-time, RAM and storage
targets remain to be set; exact retention of all decision views is already the
existing requirement. No order delivery, strategy logic or historical receipt-time
claim is introduced by this storage contract.
