# MDC Core — Contract-to-Implementation Matrix v0.8

MDC-OPS-003, 2026-11-03. Main 27fbc0a после PR #5. Область: существующий scalar core и новый
`MDC-SNAPSHOT-001/1` recorded-session profile. Статусы механизмов отделены от
результата конкретной CI-среды. [v0.7](MDC-CONTRACT-TO-IMPLEMENTATION-MATRIX-v0.7.md)
сохранена как snapshot подготовки 011 до hosted verification.

| Область | Статус | Evidence / граница |
|---|---|---|
| Scalar O/M/R/C/S/T, Evaluation, invalid artifacts | IMPLEMENTED | Существующие production APIs и полные regression suites |
| Lifecycle и Evaluation retention | IMPLEMENTED | Monotonic propagation, immutable history, full keys и pinned aliases |
| Portable identity | IMPLEMENTED | Старый frozen graph control совпал; legacy default сохранён |
| Обе Git histories и milestones | IMPLEMENTED | History guard проверяет integration bridge; main не переписана |
| Выбор первого consumer | IMPLEMENTED | Владелец выбрал historical replay → service и exact restart restoration |
| Сам trading/replay adapter | MISSING | Нет согласованных market input/output contracts и executable consumer |
| Запись session с пустого Pipeline | IMPLEMENTED | Девять mutators, все requests/outcomes, frozen Registry, readonly escape |
| Exact recorded snapshot roundtrip | IMPLEMENTED | Новый candidate, original-order replay, per-outcome bytes и полный retained manifest |
| Registry restore | IMPLEMENTED | Все 14 maps и unused versions, dependency validation, canonical order |
| Alias, invalid, lineage, lifecycle и audit restore | IMPLEMENTED | Typed equality, private collision tests, late inputs и continuation scenarios |
| Snapshot limits / malformed input | IMPLEMENTED | Семь logical limits, проверка lengths/counts/UTF-8/tags/versions/EOF; hard CPU/OOM bound не заявлен |
| Native execution 011 | IMPLEMENTED | 209 Rust + 25 Python × 6; producer и два consumer process; local evidence |
| Cross-target snapshot execution | IMPLEMENTED | Hosted CI на main: 4 cross jobs × 22 tests; archive/manifest совпали между 10 configurations |
| Post-merge main verification | IMPLEMENTED | Run 41716728041: 12/12 PASS, native 209 Rust + 25 Python × 6, history guard и 12 artifacts |
| Durable recovery contract | PARTIAL | Draft с fault scenarios подготовлен; storage и ACK boundary ожидают решения владельца, runtime отсутствует |
| Export произвольного незаписанного Pipeline | MISSING | API требует запись всех изменений с создания session |
| Общий persisted structural importer / migration | PARTIAL | Есть self-contained recorded profile; нет arbitrary graph import или engine migration |
| Durable recovery | MISSING | Нет storage commit boundary, WAL/fsync, atomic publication или crash recovery |
| Temporal availability | MISSING | Нет available_at/decision_at; horizon не доказывает causal replay eligibility |
| Reinstatement и Source-wide revoke | MISSING | За пределами monotonic lifecycle |
| Resource envelope длительного сервиса | UNVERIFIED | Live history растёт, replay O(history), compaction/production scale не измерены |
| Public release readiness | PARTIAL | API/contracts/CI есть; delivery, consumer и release policy ещё требуют решений |

[Snapshot contract](MDC-SNAPSHOT-CONTRACT-v0.1.md),
[implementation report](MDC-IMPL-011.md),
[local receipt](evidence/impl-011-local-verification.json),
[frozen snapshot vector](../tests/fixtures/snapshot-roundtrip-v1.json).
[Post-merge evidence](evidence/post-merge-main-27fbc0a.json),
[следующий recovery contract](MDC-RECOVERY-CONTRACT-v0.1-DRAFT.md).
Hosted PASS ограничен записанными commit/run, profiles и targets; production
power-loss durability и causal market replay этим не устанавливаются.
