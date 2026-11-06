# MDC-IMPL-018 — Bounded lossless replay storage v2

2026-11-06. Based on merged 017 `ac364f88d8e696abf2d865524b537408418e2450`.
[Contract](MDC-REPLAY-STORAGE-V2-CONTRACT-v0.1.md).
Runtime is implemented; hosted execution and official resource measurements must
be recorded separately before review completion.

## Result and API

The independent Linux `recovery_v2` profile adds `RunManifestV2`, `StorageLimits`,
`RunStoreV2` and `codec::{encode_blob,decode_blob}`. Existing root runtime, no_std,
mdc_store, normalization, v1 APIs/CLI, wire controls and default limits stay exact.
A new directory/API/CLI selects v2 explicitly; automatic migration is absent.

Each report and engine-011 snapshot has one independent raw/zlib envelope with
bounded lengths and decoded SHA-256. The pinned writer is pure Rust
miniz_oxide=0.9.1, with-alloc only, level 6 (adler2=2.0.1 locked). Raw wins ties.
Reader acceptance does not depend on recompression equality: a valid single
bounded dictionary-free RFC1950 stream must end exactly at encoded EOF and
produce the exact declared output/hash.

The physical journal ceiling is still 64 MiB. V2 explicitly permits at most
256 MiB total decoded blobs; it does not disguise this as the old logical
budget. Every other per-view/plan/core limit is retained. Physical digests and
audit run IDs are version-distinct; decoded bytes, decisions, lineage and status
are the original logical objects.

Open streams every frame, checks decoded lengths/aggregate budget before
inflation, and recomputes/restores every recorded decision. Only a bounded index
of offsets/lengths/digests survives recovery. Owned reads re-read and decode the
identical certified frame without a permanent decoded cache or another semantic
replay. Errors fence all certified operations until reopen. Captured manifest and
fencing metadata remain inspectable. An owned result remains valid after store
closure; old-index retry does not append.

Commit keeps the original append/sync/HEAD.pending sync/rename/directory-sync
sequence. No ACK precedes sealing. Invalid source/manifest/frames never cause a
fallback HEAD or tail repair. Recovery seals the selected prefix before exposure,
even after interrupted truncation/rename. Fork ownership and explicit flock
release are preserved. This is not a hardware-independent power-loss guarantee.

## Executable acceptance map

| Cases | Checks |
|---|---|
| LS01…LS03 | Python struct/hashlib/zlib wire TSV, miniz writer TSV, native and i686/s390x vector tests |
| LS04…LS05 | Every manifest byte/truncation, unknown versions/tags, FDICT, extra stream/bytes, Adler/hash failure, false decoded sizes, integer/aggregate limits |
| LS06 | Correctly resealed forged report/snapshot/index rejected by full semantic recomputation |
| LS07…LS09 | Real v1/v2 bundles compare exactly; cross-format open rejects; old controls/pinned 016 continuation remain |
| LS10 | Streaming index/work-count test, oldest and permuted owned reads after restart; RSS measured separately |
| LS11 | Retry/idempotency, independent physical/encoded/decoded/aggregate limits, including after a nonempty prefix |
| LS12 | Unit I/O-error fencing plus separate-process SIGKILL: 13 creation, 9 commit, 2 repair points; sync-failure sealing checks |
| LS13 | Changed and correctly resealed certified frames fence owned reads and continuation |
| LS14 | Persisted eight limits, physical/decoded diagnostics, cursor and actual files checked by independent validator |
| LS15…LS16 | Paired release study retains raw files, logs, binaries, CPU/RSS/API/wall data and exact common-prefix comparison; official execution still to record |
| LS17 | Protected root/store/v1 source audit, host-only pinned manifest/lock addition structurally checked by 016 comparison |
| LS18 | 1440 plans remain an expected rejection; no order/strategy/real receipt-time claims |

## Verification and resource methodology

Regular CI keeps its prior sixteen jobs. Native replay jobs add a paired synthetic
release study with three repetitions at 1, 3 and 1440; Python negative tests reject
missing/reused phases, false completion/cursors/metrics, limit changes and tampered
frames. The final comparison also checks both native compilers' full v2 files.
Cross jobs execute the six new wire/codec tests and pinned writer vectors.

The opt-in `MDC capacity study` workflow (`compare_storage_v2=true`) selects synthetic or one of the
pinned official Binance days. Points 1,64,256,1440 compare v1/v2 on the same host
and source with three saved repetitions. Results retain all original bytes and
first constraints. V1 may hit its original 64 MiB limit while v2 continues; only
common decoded prefixes are compared across formats. Recovered v2 decisions are
all independently rebuilt in Rust, including those beyond the v1 prefix.

CPU/RSS/wall include process launch and diagnostic hashing. API step/open clocks
are separate; recovery hash/owned-read work is labelled separately. Runs are
sequential/grouped with uncontrolled caches; these are descriptive observations,
not isolated codec microbenchmarks, p95/p99, cold RTO or universal speedup. The
prior Python size probe is not substituted for Rust measurements.

1440 decisions, durable migration, distributed writers, strategy/execution and
measured historical receipt-time remain separate tasks. A larger logical profile
requires explicit memory/recovery/storage targets and another contract.
