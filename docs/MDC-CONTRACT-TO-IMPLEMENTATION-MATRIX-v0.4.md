# MDC Core — Contract-to-Implementation Matrix v0.4

MDC-IMPL-010, 2026-11-03, parent d33bb97. Scoped значения статусов и нормативная
область сохраняются из [v0.3](MDC-CONTRACT-TO-IMPLEMENTATION-MATRIX-v0.3.md).

| Область | Статус | Свидетельство / граница |
|---|---|---|
| MDC-01…06 scalar construction | IMPLEMENTED | Реальные операции и contracts прежних пакетов; opt-in portable graph |
| Conformance / Evaluation retention | IMPLEMENTED | Legacy и portable hash policies; full keys и pinned aliases; exact lookup |
| Lifecycle | IMPLEMENTED | Immutable bodies, monotonic invalidation и captured history сохраняются |
| Portable digest protocol v1 | IMPLEMENTED | Fixed preimage, continuous FNV1a64, six independent byte/hash vectors |
| Portable semantic graph profile | IMPLEMENTED | Все admitted levels, invalid artifacts, keyed Evaluation и lifecycle получают IDs одной policy |
| Protocol tagging / unknown versions | IMPLEMENTED | ProfiledIdentity и explicit Pipeline constructor; нет fallback неизвестных имён |
| Legacy byte artifact migration | IMPLEMENTED | Verified native legacy capture → tagged portable digest тех же bytes |
| General persistent graph migration | PARTIAL | Explicit fixture replay и mapping; общего importer/durable replayer нет |
| Legacy compatibility | IMPLEMENTED | Контрольный graph/lineage/lifecycle event сравниваются с 009, старые snapshots не меняются |
| Local native CI matrix | IMPLEMENTED | 176 Rust + 21 Python × 6; separate portable validator negative controls |
| Cross-target portability verification | UNVERIFIED | В подготовленной копии hosted i686/s390x NOT_EXECUTED; workflow и comparison gate добавлены |
| Legacy 23-case E2E | IMPLEMENTED | Старый gate выполняется; portable scenario имеет собственную область проверки |
| Global digest uniqueness | UNVERIFIED | FNV64 не инъективен; действуют collision guards/full canonical keys |
| arbitrary schemas / float normalization | MISSING | v1 относится к admitted scalar basis; legacy Debug Observation не расширен |
| Full lifecycle / persistence | PARTIAL | Нет reinstatement, Source-wide revoke и durable recovery |
| Temporal availability | MISSING | Нет available_at/decision_at |

Hosted receipt уточняет cross-target execution status для конкретного published
commit/run; эта матрица фиксирует состояние подготовки и не подменяет receipt.
