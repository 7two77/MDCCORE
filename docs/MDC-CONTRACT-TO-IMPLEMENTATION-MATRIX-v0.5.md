# MDC Core — Contract-to-Implementation Matrix v0.5

MDC-INTEGRATION-001, 2026-11-03. Scalar profile и утверждения прежних contracts
сохраняются. Это обновление учитывает фактические hosted executions после подготовки
пакетов. Исторические [v0.4](MDC-CONTRACT-TO-IMPLEMENTATION-MATRIX-v0.4.md) и ранние
матрицы сохранены без переписывания.

| Область | Статус | Evidence / граница |
|---|---|---|
| Source → O/M/R/C/S/T | IMPLEMENTED | Versioned contracts, реальные операции и проверки ограниченного scalar profile |
| Conformance Boundary | IMPLEMENTED | PASS/FAIL/UNRESOLVED, criteria/context/version/lineage и immutable target snapshots |
| Typed invalid artifacts | IMPLEMENTED | Все шесть types, semantic fields и captured failure lineage |
| Lifecycle 008 | IMPLEMENTED | Monotonic invalidation, propagation, replay revision и сохранение прежних Evaluations; hosted 6/6 |
| Evaluation retention 009 | IMPLEMENTED | Full canonical keys, pinned aliases, forced-collision scenarios; hosted 6/6 |
| Portable protocol 010 | IMPLEMENTED | Explicit immutable Pipeline policy, fixed preimage, versioned codec/hash и vectors |
| Portable cross-target evidence | IMPLEMENTED | Hosted 010: 11/11 jobs, 10 configurations, x86_64/i686/s390x-QEMU, Rust 1.85.1/1.98.0; frozen control MATCH |
| Legacy 23-case E2E | IMPLEMENTED | Native configurations: 176 Rust + 21 Python, E2E 23 cases в двух процессах; portable scenario имеет отдельную область |
| Current integration ancestry | IMPLEMENTED | Hosted integration candidate: 12/12 jobs, оба roots, bridge parents, 157 blobs/modes, main archive и original README проверены |
| Применение integration к main | PARTIAL | Проверенный кандидат и PR подготовлены; remote main ещё не перемещена |
| General graph migration | PARTIAL | Explicit replay fixture и captured-byte rebinding; нет общего durable importer |
| Full lifecycle/persistence | PARTIAL | Нет reinstatement, Source-wide revoke и durable recovery |
| Global u64 uniqueness | UNVERIFIED | FNV64 не инъективен; guards/full canonical keys остаются обязательны |
| Arbitrary schemas/float normalization | MISSING | Portable profile ограничен admitted scalar bodies; legacy Debug Observation не расширен |
| Temporal availability | MISSING | Нет available_at/decision_at |
| Production scale | UNVERIFIED | Nested full keys и большой persisted graph не проверены |

Actual receipts: [baseline](evidence/baseline-hosted-ci.json),
[008](evidence/impl-008-hosted-ci.json), [009](evidence/impl-009-hosted-ci.json),
[010](evidence/impl-010-hosted-ci.json).
[Integration receipt](evidence/integration-hosted-ci.json) фиксирует 12 jobs
для 9eace82 и bridge c53279a. Последующий archival commit меняет только документацию;
финальная CI вершины кандидата публикуется в integration PR.
Эти статусы не являются универсальной сертификацией STP-MDC вне declared fixtures.
