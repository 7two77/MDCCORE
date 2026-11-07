# MDC-IMPL-018 review 001 — Authenticate owned frames before decoding

2026-11-07. Review base: `92e35921ef622efee2fdc5277e59d8474ac99426`.

The review correctly identified an ordering discrepancy in `lookup_owned()`.
It decoded a frame before comparing its length and verified physical digest to
its certified index entry. Inflation limits and fencing already applied, and a
changed result never escaped, but an independently resealed replacement could
reach a blob decoder before rejection.

The owned path now checks length, verifies framing/checksum and compares the
physical SHA-256 to the certified index before parsing blob envelopes or calling
either blob decoder. Shared body decoding follows that gate. Recovery still
verifies and recomputes every committed decision. The framing hash is computed
once on each path; there is no extra full-frame hash or new decoded cache.

The new regression uses a real committed frame. It preserves its length and both
blob lengths, corrupts a compressed report's zlib header and recomputes its outer
checksum. A direct decode proves that this fixture passes framing and fails in
the codec. The owned read must return `Integrity("v2 owned frame changed")` with
zero blob-decoder entries and must fence subsequent access. The counter exists
only in library tests. The existing resealed decision-ID test remains in place.

The regression was first run against the original lookup flow with the new test
and test-only observation hook: it failed with one decoder entry instead of zero.
The corrected flow passes with zero entries. Full verification is recorded in the
review execution receipt after completion.

Public APIs, v1 code, dependencies, wire format, limits and successful encoded
bytes are unchanged. Previous official resource observations remain bound to
`5888df2` and its original source fingerprint; they are preserved as historical
measurements. This correction does not claim new performance measurements.

Local verification passed on Rust 1.85.1 and 1.98.0: **88 normal / 93
fault-injection replay tests** per compiler, Format and Clippy with warnings denied
in both feature modes. The default and alloc scalar E2E bundles each passed
23 cases in two processes; all 17 report-validator tests passed.

[Local execution receipt](evidence/impl-018-review-001-local.json),
[current source/CI manifest](evidence/impl-018-review-001-source-manifest.json).
The current hosted result is the PR check for the review-fix head. Earlier 018
hosted/resource receipts retain the earlier revisions explicitly recorded in them.
