# MDC Core — Contract-to-Implementation Matrix v0.2

MDC-IMPL-008, 2026-11-02. Основа — STP-MDC v0.1 DRAFT / NORMATIVE, MDC-01…06,
CB и проектный профиль [MDC-LIFECYCLE-001/1](MDC-LIFECYCLE-CONTRACT-v0.1.md).
Термины IMPLEMENTED / PARTIAL / MISSING / UNVERIFIED сохраняют определения
[матрицы v0.1](MDC-CONTRACT-TO-IMPLEMENTATION-MATRIX-v0.1.md): IMPLEMENTED означает
проверенную функцию объявленного профиля, а не универсальную сертификацию MDC.

| Область | Статус | Изменение и свидетельство |
|---|---|---|
| MDC-01…06: scalar construction | IMPLEMENTED | Прежние механизмы; revoked inputs отвергаются, same-ID resurrection запрещён; 124 старых tests + новые lifecycle scenarios |
| Canonical semantic snapshots | IMPLEMENTED | Старые bytes/identity не меняются; полный контрольный 16-object graph совпал с baseline в двух toolchains |
| Typed invalid construction artifacts | IMPLEMENTED | Сохранён механизм 006; InputInvalidated дополнен frozen revision и точными event keys |
| Subsequent invalidation | IMPLEMENTED | `Pipeline::invalidate`, шесть target types, versioned policy/context/reason/evidence; [tests](../tests/lifecycle.rs) |
| Propagation по declared inputs | IMPLEMENTED | Target и реальные descendants, включая witnesses и Evaluation; независимые ветки не затрагиваются |
| Текущий и исторический lifecycle | IMPLEMENTED | Current view и replay lifecycle epochs для retained objects; admission epoch, optimistic revision, idempotent requests |
| Полный lifecycle | PARTIAL | Monotonic invalidation; нет target promotion, reinstatement, Source-wide revoke, durable journal import или полного creation-time replay |
| Conformance Boundary | IMPLEMENTED | Новая Evaluation отозванного target сохраняет Invalid/Fail и lifecycle context; unknown contract остаётся без semantic result |
| Projection isolation | IMPLEMENTED | Историческая Pass Evaluation отозванного target не допускает новую projection; external snapshot не переписывается |
| Provenance и audit | IMPLEMENTED | Старые objects, definition snapshots, affected/evidence bytes и captured closure refs сохраняются; precondition failures имеют отдельный audit |
| Core E2E gate | IMPLEMENTED | A01…A12/N01…N11: 23/23 в двух процессах каждого из шести local jobs |
| Lifecycle verification | IMPLEMENTED | 20 integration scenarios, 1 journal-key test, два independent example runs в каждом job |
| std / std,alloc / no_std+alloc | IMPLEMENTED | 145/145 Rust tests и 17/17 Python checks на Rust 1.85.1 и 1.98.0 x86_64 hosted |
| CI для пакета 008 | PARTIAL | Driver дополнен lifecycle checks; шесть local jobs PASS; hosted execution нового commit ещё NOT_EXECUTED |
| Evaluation collision-retention | MISSING | Отдельный full-key namespace для Evaluation не добавлен; коллизия даёт отказ без перезаписи |
| Identity protocol | PARTIAL | Legacy native Rust Hash/FNV u64; journals используют full keys, общий portable protocol не принят |
| Cross-platform determinism / formal uniqueness | UNVERIFIED | Fixture совпал между двумя toolchains одного target; других платформ и инъективности u64 это не доказывает |
| available_at / decision_at / as-of data | MISSING | Epoch invalidation не является исторической доступностью данных |
| External trust / payload truth | UNVERIFIED | Причина invalidation — explicit caller decision под policy, не удостоверение внешних фактов |
| Production scale / crash recovery | UNVERIFIED | In-memory graph scan и полные snapshots/keys; scalability и durable recovery не проверены |

Подробные границы и фактические результаты: [MDC-IMPL-008](MDC-IMPL-008.md).
Матрицы и reports прежних этапов сохранены как история. VAIXLNS-Builder,
STSA-MC, market projections и temporal availability extensions не добавлены.
