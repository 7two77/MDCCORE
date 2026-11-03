# MDC-IMPL-004 — State и Transition

Дата: 2026-11-02. Статус: **реализован и проверен в пределах MDC-SCALAR-001**.

Нормативное основание — предоставленный `One MDC.md`, STP-MDC v0.1,
**DRAFT / NORMATIVE**, MDC-05 и MDC-06. Реализуется ограниченный профиль
`MDC-SCALAR-001`; полное соответствие MDC-01…06 и Conformance Boundary не
объявляется. Исторические отчёты MDC-IMPL-001…003 сохраняются отдельно.

## Исходная точка и публичный API

Основа — архив MDC-IMPL-003 и отдельная контрольная копия. Изменяется активный
корневой пакет; вложенный `mdc_core/`, lockfiles и зависимости сохраняются.
Git-репозиторий не создаётся, пользовательский Codespaces не изменяется.
Файловые границы фиксируются в [source manifest](evidence/impl-004-source-manifest.json).

Legacy `State` и `Transition` с публичными полями сохранены как raw API.
Они не допускаются в pipeline автоматически. Новые immutable `AdmittedState`
и `AdmittedTransition` создаются через `Pipeline::derive_state()` и `transition()`;
чтение выполняется через `resolve_state()` и `resolve_transition()`.
Результаты — typed references, а не неявно созданные targets.

`ObjectRef`, `RejectedRequest`, `PipelineError` и `RegistryError` расширены.
Внешние exhaustive matches необходимо обновить. Теги Source/O/M/R/C сохранены;
State и Transition получили новые теги `5` и `6`.

## Контракты и определение State

Registry хранит точные versioned IDs для State criteria/contracts и Transition
conditions/contracts. Зарегистрированная версия не перезаписывается, другие
версии не используются как fallback. После передачи registry в Pipeline
публичной операции изменения контрактов нет.

State Contract определяет исходный Composition Contract, typed definition
`CompositionAbstraction`, обязательные criteria, их ordering и `DynamicsContext`.
Definition означает абстракцию принятой Composition под явными критериями;
произвольное рыночное имя не становится новой State semantics.

DynamicsContext содержит полный frame context и явные calculus/domain versions.
State frame точно совпадает с frame исходной Composition. Version descriptors
являются объявленной семантической конфигурацией; runtime не удостоверяет внешнюю
аутентичность этих ссылок или причинность causal descriptor.

| State criterion | Вычисляемая семантика |
|---|---|
| `Cardinality` | Число members исходной Composition входит в `min..max` |
| `HasRelation` | Composition содержит Relation точного требуемого contract |
| `ConstraintPassed` | У указанного constraint исходной Composition результат `Pass` |
| `MaxTemporalSpan` | Длина фактического Observation interval Composition не превышает `ticks` |
| `ObservationAfter` | Есть допустимые принятые Observation заданного Source после конца interval и не позже horizon |

Mandatory criteria должны существовать и иметь точные версии. Для
`ConstraintPassed` соответствующий constraint обязан входить в Composition
Contract; отсутствующая definition не превращается в default-pass.

Все criteria вычисляются до агрегации. Используется общий механизм
`Fail > Unresolved > Pass`. При `Fail` запрос отклоняется с полным набором
результатов. При `Pass` или `Unresolved` создаётся `Candidate`, сохраняющий
явный aggregate. Construction не переводит его в `Conformant`.

## Horizon, witnesses и неизменный snapshot

`ObservationAfter` — open-world проверка наличия допустимого свидетельства.
Без horizon или подходящего факта результат `Unresolved`, а не `Fail`.
Вычисление использует только реальные admitted Observation с точным Source
ID/version, совместимыми domain/time basis и timestamp внутри declared scope.
Свидетельства должны иметь `timestamp > interval.end` и `timestamp <= horizon`.
Caller не передаёт готовый boolean или произвольный результат criterion.

Явный horizon не может быть раньше используемых input facts и должен находиться
в scope контракта. Он фильтрует **объявленный source timestamp**, а не ingestion
time, `available_at` или `decision_at`. Пакет не вводит исторический as-of store
и не доказывает общую защиту backtest от lookahead.

Результаты criteria/conditions и отсортированные witness references сохраняются
в canonical content и provenance. Один и тот же request/horizon после позднего
admission нового факта может дать новый State: изменился разрешённый snapshot
релевантных фактов. Старый State сохраняется с прежними результатами, identity
и lineage; он не обновляется задним числом.

State validity interval — фактические минимальный и максимальный timestamps
Observation в исходной Composition. Поздний confirmation witness не расширяет
этот interval. Полный evidence interval lineage может быть шире. Проверка
Transition horizon учитывает все facts обоих input States, включая witnesses.

## Transition contracts и условия

Transition требует две существующие admitted State, точные разрешимые
source/target State Contracts и совместимый DynamicsContext с теми же версиями.
Source/target order сохраняется. Самопереход текущим профилем отвергается.
Отсутствующий target не создаётся, вывод State не создаёт Transition автоматически.
Temporal ordering сравнивает validity intervals, а не время получения
подтверждений. Ретроспективное состояние с более поздними witnesses не доказывает,
что решение было доступно в момент самого события.

| Transition condition | Вычисляемая семантика |
|---|---|
| `TemporalOrder` | `source.validity.end < target.validity.start` при strict, иначе `<=` |
| `SourceCriteriaPass` | Переносит фактический aggregate source State, включая `Unresolved` |
| `TargetCriteriaPass` | Переносит фактический aggregate target State, включая `Unresolved` |
| `ObservationAfterTarget` | Open-world witness после конца target interval под заявленным horizon |

В каждом Transition Contract обязательны `TemporalOrder`, `SourceCriteriaPass`
и `TargetCriteriaPass`. Дополнительный witness condition регистрируется явно.
Так unresolved State не повышается до прошедшего transition по умолчанию.

Все conditions получают явный результат, затем общую агрегацию. `Fail`
отклоняет попытку с сохранением всех результатов; `Pass` и `Unresolved`
создают только `Candidate`. Transition не устанавливает причинность, prediction,
signal, action или truth и не переписывает input States.

## Canonicalization, provenance и invalidity

Новые объекты имеют отдельные versioned prefixes. Canonical basis сохраняет
contract/definition, source references, criterion/condition definitions и
результаты, witnesses, aggregate, horizon, actual State interval, context,
calculus/domain versions, provenance и Candidate status, где применимо.
Ordered criteria/conditions сохраняют порядок; unordered sets нормализуются
согласованно с Registry. Source/target references Transition не сортируются.

Recursive trace проходит через State/Composition/Relation/Measurement/Observation
до зарегистрированных Sources и включает реально использованные witnesses.
Downstream construction выдаёт shared-only доступ к upstream objects.
Legacy native Rust `Hash`/FNV-подобный `u64` identity protocol сохранён;
межплатформенная переносимость и формальная уникальность не обещаются.
Локальная collision guard расширена на State и Transition.
Совместимость прежнего O/M/R/C fixture проверяется в
[compatibility evidence](evidence/impl-004-compatibility.json).

Отклонённые запросы остаются в `RejectedAttempt` с точными inputs, reason и
status `Invalid`. Это audit representation попытки, а не полноценный invalid
State/Transition semantic object. MDC-05.9/06.12 в части такого representation
и retention остаются `PARTIAL`. Полный CB evaluator и lifecycle conformance
требуют отдельного этапа.

## Покрытие и проверки

| Требование | Состояние bounded profile |
|---|---|
| MDC-05 explicit derivation/definition/criteria/context | Реализовано для CompositionAbstraction и пяти kinds criteria |
| MDC-05 criteria aggregation, witnesses, canonicalization, lineage | Реализовано с явными Pass/Fail/Unresolved |
| MDC-06 existing States, conditions, temporal compatibility и direction | Реализовано для четырёх condition kinds |
| Upstream immutability, no target synthesis | Реализовано через immutable store и lookup-only references |
| Invalid semantic object representation/retention | PARTIAL: отклонённые requests/reasons сохранены для аудита |
| External source authenticity и историческая доступность данных | Не устанавливаются текущим profile |
| Полный CB evaluator и MDC-E2E-CONFORMANCE-001 | NOT EXECUTED / BLOCKED |

Точные commands/stdout/stderr и exit codes текущей копии записаны в
[отчёт проверок](evidence/impl-004.json). Исторические результаты не подменяют
новый прогон.

```bash
cargo test --locked --offline
cargo test --locked --offline --features std,alloc
cargo test --locked --offline --no-default-features --features alloc
cargo check --locked --offline --no-default-features --features alloc
cargo fmt --all -- --check
cargo clippy --locked --offline --all-targets -- -D warnings
cargo clippy --locked --offline --no-default-features --features alloc --all-targets -- -D warnings
cargo run --locked --offline --example state_transition
```

| Группа проверок | Результат |
|---|---|
| Default / std,alloc / no_std+alloc tests | PASS, 95/95 в каждой конфигурации |
| Library check no_std+alloc | PASS |
| Неподдерживаемый профиль без features | Ожидаемый exit 101 с явной диагностикой alloc |
| Format и Clippy обоих профилей | PASS |
| Исполнимый State → Transition example | PASS в default и no_std+alloc: states=2, transitions=1, result=Pass, status=Candidate, lineage_nodes=16 |
| Прежние примеры Source → Measurement и Relation → Composition | PASS |
| Совместимость прежних O/M/R/C bytes, identities и trace | MATCH на контрольном графе из 6 semantic objects |
| Неизменность nested package и lockfiles | MATCH, побайтовая сверка с MDC-IMPL-003 |
| Полный MDC-E2E-CONFORMANCE-001 | NOT EXECUTED / BLOCKED |

Среда — Rust 1.85.1, `x86_64-unknown-linux-gnu`; сборки размещены отдельно в `/tmp`.
Из 95 тестов 65 сохранены из предыдущего пакета. Добавлены 8 configuration tests,
2 unit-проверки witness resolution и 20 интеграционных тестов через публичный
Pipeline API. Они проверяют всю текущую цепочку, реальные результаты criteria и
conditions, horizon, late admission, immutable snapshots, no synthesis,
Fail precedence с сохранением Unresolved, ordering и эквивалентные графы при
перестановке загрузки. Существующие два E2E-теста остаются smoke-проверками.

Для compatibility probe один и тот же код отдельно скомпилирован против
неизменённого MDC-IMPL-003 и текущего MDC-IMPL-004. Сравнены canonical bytes и
identity двух Observation, двух Measurement, одной Relation и одной Composition,
а также полный recursive trace. Код и оба выхода сохранены в отчёте. Это
проверка указанного fixture на этой среде, а не межплатформенная гарантия hash.

Патч `mdc-core-impl-004.patch` применяется поверх MDC-IMPL-003.
`mdc-core-impl-004-from-original.patch` включает все изменения от первоначального
пользовательского архива. Патчи альтернативны. Для обоих проверено применение
к соответствующей базе и побайтовое совпадение результата с текущим проектом.

Следующий этап — полноценный Conformance Boundary, включая Evaluation contract,
criteria resolution, versions/context/provenance и диагностические артефакты.
Представление invalid semantic objects и полный MDC-E2E-CONFORMANCE-001 пока
остаются незавершёнными.

Hosted `no_std + alloc` проверки не подтверждают allocator/linker configuration
произвольной bare-metal-платформы. Без features ожидается явная ошибка об
обязательном `alloc`. Новые dependencies не вводятся. Wave/Trend/Signal,
VAIXLNS-Builder и STSA-MC остаются вне области работ.
