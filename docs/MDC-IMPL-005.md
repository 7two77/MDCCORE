# MDC-IMPL-005 — Conformance Boundary: Evaluation

Дата: 2026-11-02. Выполнен ограниченный evaluation-профиль MDC-SCALAR-001
по CB-01…09 предоставленного STP-MDC v0.1 DRAFT / NORMATIVE.
Полный Conformance/Projection Boundary и MDC-E2E-CONFORMANCE-001 не объявляются завершёнными.

## Контракты и API

Registry регистрирует точные версии ConformanceCriterionContract и ConformanceContract.
Контракт содержит target type (Observation, Measurement, Relation, Composition,
State либо Transition), обязательные criteria, их порядок, DynamicsContext с
версиями calculus/domain и политику отсутствующего target Fail/Unresolved.
Пустой набор, дубликаты, неразрешимые версии, неприменимый MemberCount и отсутствие
TargetExists/LineageComplete отвергаются при регистрации. Версия не перезаписывается.

Pipeline::evaluate(EvaluationRequest { contract, target, context }) сохраняет
immutable ConformanceEvaluation и возвращает ObjectRef::Evaluation. Есть
resolve_evaluation(), evaluation_count(), canonical basis и recursive trace.
Legacy Evaluation остаётся raw-структурой и не является новым artifact API.
Тег Evaluation — 7; старые теги 0…6 сохранены. Расширенные ObjectRef/RegistryError
требуют обновления внешних exhaustive matches.

Встроенные критерии:

| Критерий | Реальная проверка |
|---|---|
| TargetExists | Target существует в admitted store нужного типа |
| LineageComplete | Полный trace разрешается до зарегистрированных Sources |
| IdentityCoherent | Captured basis согласуется с сохранённым ID |
| ConstructionPass | Для C/S/T переносится вычисленный aggregate; O/M/R успешно прошли admission/construction |
| MemberCount | Реальное число Composition members входит в min…max |

Наличие ConstructionPass не подразумевается автоматически: PASS означает
соответствие именно объявленному набору criteria. Runtime не принимает готовые
результаты от caller. Все результаты вычисляются до общей агрегации
FAIL > UNRESOLVED > PASS. Версии и context записываются в Evaluation; scope
покрывает все Observation facts target lineage. Для R/C требуется точное
совпадение frame, для S/T — DynamicsContext с версиями. Для O/M совпадают domain,
time basis и допустимый scope; их собственная модель не содержит calculus version.
Version descriptors остаются объявленной конфигурацией, не внешним удостоверением.

## Result, lifecycle и invalid artifacts

- Pass: Evaluation.status=Conformant; target остаётся без изменений.
- Fail или Unresolved при корректном выполнении: Evaluation.status=Candidate.
- Неверный target type/context, неполный lineage либо отсутствующий target:
  сохраняется Evaluation.status=Invalid с diagnostics и результатами criteria.
- При отсутствующем target применяется объявленная Fail/Unresolved policy;
  отсутствующий объект не создаётся, trace такого artifact не объявляется полным.
- Если contract не разрешён, сохраняется Invalid diagnostic artifact с result=None.
  Это отсутствие semantic evaluation, а не четвёртый ConformanceResult и не
  автоматически Unresolved. Request/context и snapshot существующего target
  сохраняются, criteria не выдумываются.

Evaluation хранит definition контрактов/criteria, их results, aggregate, context,
target reference и captured bytes, provenance и diagnostics в canonical content.
Повторный идентичный результат дедуплицируется; локальная collision guard
охватывает и evaluations. Valid Evaluation trace включает artifact, target и
его Source lineage. Invalid diagnostics остаются доступны через getters;
trace явно сообщает неполноту вместо синтеза missing references.

CB-08 retention Evaluation реализован через реальные invalid artifacts.
Полноценные invalid semantic objects O/M/R/C/S/T по-прежнему не реализованы:
для них сохраняются rejected attempts. Нельзя объявлять этот пробел закрытым.
Переходы lifecycle target objects, projection и interpretation здесь не добавляются.
PASS не означает Truth. Native Rust Hash/FNV-u64 limitations сохранены.
Horizon/arrival-time ограничения MDC-IMPL-004 остаются прежними.

## Проверки и поставка

Rust 1.85.1, x86_64-unknown-linux-gnu. 101/101 тестов в default, std,alloc и
no_std+alloc; 95 прежних плюс 6 новых integration tests. Format и Clippy обоих
профилей проходят. Без features — ожидаемая диагностика alloc.
Пример `cargo run --locked --offline --example conformance` проходит в обоих
профилях и создаёт Evaluation для полной текущей O/M/R/C/S/T-цепочки.

Новые тесты проверяют все шесть target types, snapshots всего upstream graph,
реальные Fail/Unresolved, dedup/lineage, обе missing-target policies, unknown
contract/version, context/type errors, порядок criteria и ошибки регистрации.
Старые два E2E-теста остаются smoke-проверками.

Evidence:

- [Команды, stdout/stderr и результаты](evidence/impl-005.json).
- [Compatibility probe](evidence/impl-005-compatibility.json): один и тот же код
  против 004/005, 15 semantic objects O/M/R/C/S/T, bytes/IDs/trace. Это контрольный
  fixture на указанной среде, не универсальная переносимость hash.
- [Source manifest](evidence/impl-005-source-manifest.json): границы изменений.

Вложенный пакет, Cargo.toml, lockfiles и исторические отчёты не изменены.
Codespaces не изменялся, Git не создавался, зависимости не добавлялись.
Патч impl-005 применяется к impl-004; from-original — к первоначальному архиву.
Они альтернативны; применение и побайтовая реконструкция проверены.

Следующий этап: полное представление invalid semantic artifacts и обязательные
E2E assertions, negative suite, projection non-interference. До их исполнения
MDC-E2E-CONFORMANCE-001 остаётся NOT EXECUTED / BLOCKED.
