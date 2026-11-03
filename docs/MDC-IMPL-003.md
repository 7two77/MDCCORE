# MDC-IMPL-003 — Relation и Composition

Дата: 2026-11-02. Статус: **реализован и проверен в пределах MDC-SCALAR-001**.

Нормативная основа — предоставленный `One MDC.md`, STP-MDC v0.1,
**DRAFT / NORMATIVE**, MDC-03 и MDC-04. Пакет расширяет ограниченный исполнимый
профиль `MDC-SCALAR-001`; полное соответствие MDC-01…06 и CB этим не объявляется.

## Исходная точка и API

Основа — архив MDC-IMPL-002 и отдельная неизменённая контрольная копия.
Активным остаётся корневой пакет. Вложенный `mdc_core/`, lockfiles и зависимости
сохраняются; Git-репозиторий не создаётся, пользовательский Codespaces не меняется.
Сведения о границах изменений записываются в
[source manifest](evidence/impl-003-source-manifest.json).

Legacy `Relation` и `Composition` с публичными полями остаются raw-структурами.
Создание их вручную не допускает объект в pipeline. Новые `AdmittedRelation` и
`AdmittedComposition` создаются только через `Pipeline::relate()` и `compose()`;
их поля закрыты, публичные getters возвращают значения или shared references.
Результаты разрешаются typed references, без неявного создания targets.

Публичные `ObjectRef`, `RejectedRequest`, `PipelineError` и `RegistryError`
получили новые variants. Внешние exhaustive matches необходимо обновить.
Сигнатуры ранее реализованных admission/measurement APIs сохранены.

## Relation contracts

`RelationContract` содержит точный versioned ID, kind и `RelationContext`.
Контекст включает domain/time basis, включительный temporal scope `[start,end]`,
versioned reference frame и causal context. Последние являются явными
объявленными descriptors; runtime не доказывает causal fact и не выполняет
неявное преобразование reference frames. Пустые IDs и scope с `start > end`
не допускаются конструкторами контекстных типов.

Каждая версия регистрируется явно и не заменяется. Kind определяет встроенное
вычисление этой версии relation contract; это вычислительное основание PRE-R6.
Неизвестная версия не подменяется зарегистрированной соседней версией.

| Kind | Проверяемая семантика |
|---|---|
| `Precedes` | Два admitted Observation: `t(left) < t(right)` |
| `TemporalDistance` | Два admitted Observation: signed `i128(t(right)) - i128(t(left))` в объявленной временной системе |
| `Adjacent` | Два Observation с явно зарегистрированным положительным `step`; directed либо symmetric согласно contract |
| `Proportion` | Два admitted Measurement с одинаковыми unit/domain/time basis; denominator не равен нулю |

Temporal kinds принимают только Observation: отдельная temporal coordinate для
Measurement не выводится из его lineage произвольным выбором одного timestamp.
PROPORTION принимает Measurement, включая prior Measurements из MDC-IMPL-002.
Scope проверяется по всему Observation lineage обоих inputs, а не только по
непосредственным references. Все входы должны соответствовать domain/time basis
контракта; Source, Relation и Composition не являются допустимыми operands
текущих scalar relation kinds.

PROPORTION хранится как точная signed пара `(numerator, denominator)` без
floating conversion, деления, сокращения или смены знака. Поэтому даже
`i128::MIN / -1` представляется безопасно как пара, а не вычисляется как
переполняющееся целочисленное частное. Упорядоченные operands и provenance
сохраняются; одинаковое числовое отношение не означает одинаковую Relation.

Operand ordering и directed topology разделены. PRECEDES, TEMPORAL_DISTANCE
и directed ADJACENT создают направленные arcs. PROPORTION сохраняет operands,
но не становится directed topology edge. Только symmetric ADJACENT допускает
нормализацию endpoints; одновременно нормализуется provenance, поэтому смена
порядка вызова не создаёт другую semantic Relation.

## Composition contracts и constraints

`CompositionContract` явно определяет member order/type, непустой набор
versioned constraint references и полный context. Registry разрешает все
references, отвергает дубликаты и сортирует constraint set канонически.

Допустимые members — Observation, Measurement либо их смешанный scalar-набор
согласно `MemberType`. Nested Composition members и Relation-as-member не
поддержаны. Members и Relations должны существовать в admitted store, быть
уникальными и сохранять собственный lineage. Каждая используемая Relation
должна ссылаться непосредственно на объявленных members: исключений из
referential closure текущий профиль не предусматривает.

Контекст каждой Relation должен точно совпадать с context Composition, включая
scope, reference frame и causal descriptor. Для members проверяются domain/time
basis и temporal bounds всего Observation lineage. Это явное ограничение
профиля, а не универсальное правило совместимости произвольных MDC contexts.

| Constraint | Фактически вычисляемое условие |
|---|---|
| `Cardinality` | Число уникальных members входит в зарегистрированный диапазон `min..max`, где `1 <= min <= max` |
| `SameUnits` | Unit всех scalar members совпадает, без conversion |
| `Connected` | Все members связаны relations при рассмотрении связности без направленности |
| `Acyclic` | Направленная часть графа не содержит cycle; nondirected relations не превращаются в arcs |
| `OrderedChain` | Для каждой соседней пары ordered members существует Relation требуемого contract с тем же направлением |
| `RequiresRelation` | Присутствует хотя бы одна Relation точной требуемой версии contract |

OrderedChain разрешён только для ordered members и directed relation contract.
Symmetric ADJACENT и PROPORTION не могут быть объявлены его направленными arcs.
Само условие проверяет наличие нужных рёбер, а не универсальный физический порядок
времени. Хронологическая chain требует выбора PRECEDES; TEMPORAL_DISTANCE
допускает отрицательную разность временных координат.
Для одноэлементной последовательности условие соседних пар выполняется без
ребра; требование существования Relation задаётся отдельным RequiresRelation.

После структурных проверок вычисляются все объявленные local constraints.
Каждый результат сохраняет versioned reference, kind и `Pass`/`Fail`.
Aggregate выводится из фактических результатов общим механизмом агрегации.
При failed constraint попытка отклоняется; причина содержит полный вычисленный
набор результатов. Новые admitted objects в store не вставляются.

Успешно построенная Composition остаётся `Candidate` с local aggregate `Pass`.
Это не `Conformant`: CB evaluator и его lifecycle ещё не реализованы.
Текущий набор constraints полностью вычислим; режим внешних `Unresolved`
constraints здесь не вводится и не подменяется default-pass.

## Canonicalization, lineage и immutability

Новые объекты используют отдельные versioned prefixes и length-prefixed fields.
Relation basis включает contract/kind parameters, operands, полный context,
computed value, provenance и Candidate status. Composition basis сохраняет
contract, member policy, members, Relations, constraint definitions/results,
context, provenance и status.

Ordered members сохраняют порядок. Unordered members нормализуются по typed
references; Relations являются уникальным отсортированным набором. Constraint
set и provenance следуют той же нормализации. Это предотвращает ситуацию, когда
основные fields совпадают, а insertion-dependent provenance меняет identity.

Теги существующих Source/Observation/Measurement references сохранены; новые
Relation и Composition получают отдельные теги. Совместимость ранее принятых
O/M bytes и identities проверяется отдельно в
[compatibility evidence](evidence/impl-003-compatibility.json).
Хеш остаётся legacy native Rust `Hash`/FNV-подобным `u64`: переносимость между
платформами/toolchains и формальная уникальность не обещаются. Локальная проверка
коллизий расширена на все четыре вида admitted objects и не допускает перезапись.

Recursive trace Composition включает members, Relations, Measurement/Observation
lineage и зарегистрированные Sources. Trace выдаёт sorted closure; это не
изменяет semantic ordering внутри directed Relation или ordered Composition.
Публичный API не выдаёт mutable доступ к upstream objects; downstream construction
не меняет их content.

Отклонённые Relation/Composition requests сохраняются с точными inputs и reason
как `RejectedAttempt` со status `Invalid`. Это audit representation попытки,
а не полноценный invalid Relation/Composition semantic object. Поэтому
MDC-03.10 и MDC-04.11 в части invalid object representation/retention остаются
`PARTIAL`; audit записи не выдаются за полную реализацию этих требований.

## Матрица покрытия

| Требование | Состояние в bounded profile |
|---|---|
| MDC-03 refs/kind/context/version/computation | Реализовано для четырёх объявленных kinds |
| MDC-03 direction, ordered ratio, symmetric normalization | Реализовано с разделением ordering и topology |
| MDC-03 scope, units и recursive lineage | Реализовано для admitted scalar operands |
| MDC-04 explicit rule, constraints, closure | Реализовано без closure exceptions |
| MDC-04 rule-dependent ordering и upstream immutability | Реализовано для O/M members и admitted Relations |
| MDC-03/04 invalid semantic object retention | PARTIAL: requests/reasons сохранены для аудита |
| Nested compositions, external unresolved criteria | Не реализованы в текущем профиле |
| MDC-05/06, CB evaluator, полный E2E | NOT EXECUTED / BLOCKED |

## Проверки и воспроизводимость

Точные команды и результаты текущей копии записаны в
[машиночитаемый отчёт](evidence/impl-003.json). Результаты прежних пакетов остаются
историческими и не подменяют текущий прогон.

```bash
cargo test --locked --offline
cargo test --locked --offline --features std,alloc
cargo test --locked --offline --no-default-features --features alloc
cargo check --locked --offline --no-default-features --features alloc
cargo fmt --all -- --check
cargo clippy --locked --offline --all-targets -- -D warnings
cargo clippy --locked --offline --no-default-features --features alloc --all-targets -- -D warnings
cargo run --locked --offline --example relation_composition
```

Поддерживаемые features, отсутствие новых dependencies и ограничения hosted
`no_std + alloc` остаются прежними. Без features ожидается явная ошибка об
обязательном `alloc`, а не случайная ошибка импортов.

| Группа проверок | Результат |
|---|---|
| Default / std,alloc / no_std+alloc tests | PASS, 65/65 в каждой конфигурации |
| Library check no_std+alloc | PASS |
| Неподдерживаемый профиль без features | Ожидаемый exit 101 с явной диагностикой alloc |
| Format и Clippy обоих профилей | PASS |
| Исполнимый Relation → Composition example | PASS в default и no_std+alloc: members=2, relations=1, status=Candidate, lineage_nodes=7 |
| Прежний Source → Observation → Measurement example | PASS |
| Совместимость O/M canonical bytes, identities и trace | MATCH на контрольной цепочке из 2 Observation и 3 Measurement |
| Неизменность nested package и lockfiles | MATCH, побайтовая сверка с MDC-IMPL-002 |
| Полный MDC-E2E-CONFORMANCE-001 | NOT EXECUTED / BLOCKED |

Среда: Rust 1.85.1, `x86_64-unknown-linux-gnu`. Все сборки размещены вне проекта.
Из 65 тестов 37 сохранены из предыдущего пакета. Добавлены 6 registry-проверок,
4 Relation unit-теста и 18 интеграционных проверок через публичный Pipeline API.
Они проверяют actual constraints, полное lineage текущей цепочки, неизменность
upstream snapshots, эквивалентность unordered графа при перестановках, rejection
audit и отсутствие вставок при отказе. Отдельно проверены изолированные members,
направленные циклы и параллельные arcs, которые не должны создавать ложный cycle.

Для compatibility probe один и тот же Rust-код отдельно скомпилирован против
неизменённого MDC-IMPL-002 и текущего MDC-IMPL-003. В отчёте сохранены код probe,
выходы обоих запусков и сравнение. Это проверка заданного fixture на указанной
среде, а не межплатформенная гарантия native hash protocol.

Патч `mdc-core-impl-003.patch` предназначен для состояния MDC-IMPL-002.
`mdc-core-impl-003-from-original.patch` включает все изменения от первоначального
пользовательского архива. Это альтернативные патчи. Для каждого проверено
применение к соответствующей базе и побайтовое совпадение результата с проектом.

Следующий функциональный этап — State и Transition поверх проверенной
Composition. Full CB evaluation и MDC-E2E-CONFORMANCE-001 остаются отдельными
этапами. Wave/Trend/Signal, VAIXLNS-Builder и STSA-MC в текущую область не входят.
