# MDC Core — Contract-to-Implementation Matrix v0.3

MDC-IMPL-009, 2026-11-03, база 70f37fc. IMPLEMENTED / PARTIAL / MISSING /
UNVERIFIED имеют прежний scoped смысл [матрицы v0.2](MDC-CONTRACT-TO-IMPLEMENTATION-MATRIX-v0.2.md).
Основа STP-MDC v0.1 DRAFT / NORMATIVE и проектные профили не переутверждаются.

| Область | Статус | Свидетельство / граница |
|---|---|---|
| MDC-01…06 scalar construction | IMPLEMENTED | Старые механизмы и 145 прежних tests сохранены; full-key Evaluation не допускается как новый semantic input |
| Conformance computation | IMPLEMENTED | Criteria/result/status/context/provenance не меняются при выборе storage handle |
| Evaluation collision retention | IMPLEMENTED | Authoritative full keys, pinned aliases, все члены digest bucket; 16 принудительных collision scenarios |
| Canonical reference codec | IMPLEMENTED | Новый ObjectRef tag 14 с полным key; tags 0…13 сохранены; отдельный codec test |
| Canonical keyed API | IMPLEMENTED | Все шесть target types, exact lookup/projection, idempotent same-key retention; 6 public integration scenarios |
| Legacy references | PARTIAL | Ordinary references совместимы и не перепривязываются; ownership colliding numeric alias зависит от первого reservation |
| Provenance и invalid capture | IMPLEMENTED | Точный full key сохраняется и разрешается; чужой bucket member не подставляется |
| Lifecycle 008 | IMPLEMENTED | Aliases нормализуются к одному primary artifact; collision не распространяет invalidation на чужой результат |
| Canonical compatibility | IMPLEMENTED | Старый 16-node graph/lineage и lifecycle event bytes совпали с 008 во всех шести jobs |
| Core E2E | IMPLEMENTED | 23/23 cases в двух процессах каждого local job; collisions проверяются отдельными tests, а не именем старого gate |
| CI для 009 | PARTIAL | 168/168 Rust и 17/17 Python × 6 local jobs; два normal keyed example runs; hosted execution 009 NOT_EXECUTED |
| Полный lifecycle / durable journal | PARTIAL | Monotonic invalidation; нет reinstatement, Source-wide revoke, durable recovery или полного creation-time replay |
| Portable identity protocol | MISSING | Native Rust Hash/FNV остаётся; full canonical keys не являются новым cross-platform hash protocol |
| Global u64 uniqueness | UNVERIFIED | Локальное disambiguation не доказывает инъективность digest |
| available_at / decision_at | MISSING | Lifecycle epoch не является временем доступности знания |
| External Source authenticity / payload truth | UNVERIFIED | Declared metadata и explicit revocation reasons не удостоверяют внешние факты |
| Production scale / persistence | UNVERIFIED | Copying full keys и map/bucket scans; большой persisted graph не проверен |

Подробности и применение: [MDC-IMPL-009](MDC-IMPL-009.md).
Контракт: [MDC-EVALUATION-RETENTION-001/1](MDC-EVALUATION-RETENTION-CONTRACT-v0.1.md).
Исходные документы и evidence прежних пакетов сохранены как история.
