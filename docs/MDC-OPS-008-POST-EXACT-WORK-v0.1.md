# MDC-OPS-008 — Verified exact-work main and lossless storage proposal

2026-11-06. Owner merged PR #12. Main
`ac364f88d8e696abf2d865524b537408418e2450` has parents
`6b33a4375384ed4301695ea879e641a20f5493b9` and reviewed 017
`377f351b232b8d06e69886bf9fb04416519a163d`.
Its tree `f36a23147eadf233f3bb16926e7a7dc9a8b9697f` exactly matches the reviewed
tree. History/scalar evidence checks pass; both original roots remain intact.
The implementation source fingerprint remains
`e6d277fc9ff80465cd401dd55671efbad24ab244194d30d4ad2085d59435931a`.

[Post-merge CI](https://github.com/7two77/MDCCORE/actions/runs/42284985481)
passed **16/16 jobs**. Actual logs confirm core 213 Rust + 25 Python per six
native cells, replay 55 default + 58 fault Rust + 55 Python per compiler,
cross-target 50 Rust per four cells and store 36 default + 38 fault Rust per
compiler. All sixteen artifact sets were uploaded. The downloaded comparison
ZIP digest matches GitHub; all six receipts bind this main/source and preserve
the frozen controls. [Receipt](evidence/post-merge-main-ac364f8.json). The preceding
[017 A/B study](evidence/impl-017-hosted-ab.json) remains bound to its measured
runtime revision; this checkpoint does not re-label old timings as a new run.

## Storage observation

In one retained 153-record journal, logical payload is **67,053,084 bytes**:
6,179,739 report bytes and 60,873,345 snapshot bytes. The full v1 journal is
67,091,458 bytes. Snapshot content accounts for about 90.8% of payload; reducing
CPU alone cannot make these bytes smaller.

An offline Python zlib probe compressed each report and snapshot independently,
retaining raw bytes if compression did not shorten them. Every decompression was
compared against the original bytes; input outer ZIP and frame/blob hashes were
checked. [Complete size probe](evidence/lossless-codec-size-probe-001.json).

| Probe level | Selected encoded report+snapshot payload bytes |
|---:|---:|
| 1 | 12,495,545 |
| 6 | 11,449,197 |
| 9 | 11,355,983 |

These are compressed payload sizes from Python zlib 1.3.2, not a deployed
journal format, Rust encoder golden bytes or compression-time result. New
framing/HEAD/manifest overhead is excluded. One prefix is not a general ratio.

## Selected next package

[Proposed MDC-IMPL-018](MDC-REPLAY-STORAGE-V2-CONTRACT-v0.1-DRAFT.md) combines:

- opt-in versioned raw/zlib blobs with exact decoded digests and semantics;
- independent encoded/decoded/per-record/aggregate limits;
- streaming semantic recovery and owned lookup, with a bounded certified index;
- original strict prefix publication, fencing and complete history retention.

Compression alone could allow more decoded data than the old in-memory bundle
Vec can safely retain. Therefore the new profile needs its own streaming memory
interface, rather than simply compressing bytes around that Vec. v1 stays intact.
The proposed v2 aggregate decoded cap is explicitly 256 MiB; the physical journal
cap remains 64 MiB, plan/visible bounds remain 256. This is a new experimental
profile proposal, not a changed v1 default or a product memory/RTO guarantee.

LS01…LS18 cover vectors, malformed streams, bounded decompression, semantic
forgery, decoded v1/v2 equality, old v1 compatibility, owned reads, fault recovery
and measurements. No runtime/dependency/default changes were made here.

A 1440-decision full-day profile still needs separate plan/visible/core-work and
resource decisions. Existing exact retention remains required; physical storage
reduction must not be presented as proof that full-day replay is already supported.
