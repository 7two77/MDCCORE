# MDC-E2E-CONFORMANCE-001 — Gate Readiness v0.1

Статус: **NOT_EXECUTED / acceptance BLOCKED**. Это подготовка обязательного gate,
а не новый отчёт о его исполнении. Нормативная основа — `One MDC.md`, STP-MDC v0.1
DRAFT / NORMATIVE, assertion matrix E2E-A01…A12 и negative suite N01…N11.
Machine-readable manifest: [mdc_e2e_gate_v1.json](../tests/fixtures/mdc_e2e_gate_v1.json).

Component tests MDC-IMPL-001…006 дают основания привязать проверки к реальным API.
Они не присваивают PASS всему gate. Старые два теста в
`tests/mdc_e2e_conformance_001.rs` остаются smoke-проверками.

## Вход и граница fixture

Используется ограниченный профиль `MDC-SCALAR-001`. Fixture version — 1.
До исполнения необходимо заморозить конкретные source records, deterministic
acquisition coordinates, horizon, context и **точные пары ID/version** всех
source/schema/operator/relation/composition/state/transition/conformance contracts.
Calculus/domain versions, Rust toolchain, target triple, features и fingerprint
implementation также входят в execution report. Название профиля не заменяет
полный список применённых контрактов.

Fixture вызывает `Registry` и `Pipeline` API, снимает canonical snapshots,
сравнивает и проверяет результаты. Он не содержит своих версий measurement,
State/Transition derivation, criteria aggregation или identity function.
Неизвестный API/contract нельзя заменить локальной логикой ради PASS.

Минимальный позитивный граф:

```text
registered Source → O-A/O-B → M-A/M-B → R → C
                                         → S-A/S-B → T → Evaluation
```

Оба State должны реально существовать. Каждый input и derivation contract
разрешается production API. Два запуска строятся в независимых `Pipeline`;
дополнительная перестановка creation order допустима только при эквивалентном
semantic input. Для ordered members/criteria/operands порядок сохраняется.

## Обязательные assertions

Все строки ниже имеют harness status **NOT_EXECUTED**. Component coverage
означает только наличие отдельных проверок, без автоматического зачёта gate.

| ID | Требование / ожидаемый результат | Production surface и существующая component coverage |
|---|---|---|
| E2E-A01 | Explicit contracts; PASS | Exact-version `Registry`; регистрации и проверки версий в `tests/source_measurement.rs`, `tests/conformance_boundary.rs` |
| E2E-A02 | Deterministic identity; PASS | `CanonicalSemanticBasis`, `IdentityAdapter`; whole-slice ordering tests в `tests/state_transition.rs`; требуется единое сравнение O/M/R/C/S/T/E |
| E2E-A03 | Full lineage; PASS | `Pipeline::trace`; captured invalid closure/report из 006; отдельные chain/trace tests не заменяют closure всего fixture |
| E2E-A04 | Upstream identity/content preservation; PASS | `snapshot()` до/после каждой реальной downstream operation; component tests в `tests/relation_composition.rs`, `tests/state_transition.rs` |
| E2E-A05 | PASS aggregation; semantic PASS | `aggregate_results`, `Pipeline::evaluate`; положительная Evaluation существующего target |
| E2E-A06 | UNRESOLVED aggregation; semantic UNRESOLVED | Реальные unavailable witnesses в State/Transition/Evaluation; `tests/state_transition.rs`, `tests/conformance_boundary.rs` |
| E2E-A07 | FAIL precedence; semantic FAIL | Actual failed condition вместе с Unresolved, общий aggregator; полный вектор результатов сохраняется |
| E2E-A08 | Invalidity preservation; INVALID + retained object | Typed partial semantic bodies из 006, доступные definitions/results/snapshots, отдельные invalid references и запрет использования как admitted input |
| E2E-A09 | Missing future object; no synthesis | Strict target lookup, counts, missing references и retained invalid result; `missing_references_versions_and_wrong_input_types_never_synthesize_targets` |
| E2E-A10 | No semantic inflation; PASS | Инспекция semantic types/labels всего графа; отдельный namespace assertion ещё не исполнен |
| E2E-A11 | No semantic backflow; PASS | Полные upstream snapshots при реальном downstream и попытке влияния Representation; component immutability coverage недостаточно |
| E2E-A12 | Projection non-interference; PASS | Зарегистрированный тестовый projection profile, реальная mapping operation и snapshots; **surface отсутствует** |

A06/A07 — это ожидаемые **semantic** результаты. Harness получает PASS, когда
фактический результат совпал с ожиданием. `Unresolved` не подменяется `Fail`,
а отсутствие исполнения не подменяется ни одним из них.

## Negative suite

Каждая процедура нарушает один заявленный invariant; связанные preconditions
должны оставаться корректными. Все N01…N11 обязательны и пока **NOT_EXECUTED**.

| ID | Случай | Что необходимо проверить |
|---|---|---|
| N01 | Missing contract | Точная версия не разрешилась; нет fallback или invented criteria. Unknown contract может оставить semantic result отсутствующим и invalid diagnostics |
| N02 | Unresolved composition | State derivation не создаёт Composition; сохранены invalid State fields и unresolved input |
| N03 | Missing criterion | Неполная definition отвергается registry; нельзя создать default-pass criterion. Configuration rejection не означает выполненную semantic evaluation |
| N04 | Unresolved criterion | Реальный обязательный witness отсутствует под объявленным horizon; результат Unresolved сохраняется |
| N05 | Failed criterion | Реальный failed criterion даёт Fail, включая комбинацию с Unresolved; все вычисленные результаты доступны |
| N06 | Missing target state | Никакого synthetic State; invalid Transition и missing reference сохранены |
| N07 | Invalid temporal ordering | Reverse/overlap нарушает конкретный strict/non-strict contract; получен ожидаемый отказ/Fail |
| N08 | Incomplete provenance | Missing object/definition явно отражается в partial closure; complete lineage не выдумывается. Точная публичная invalid-input процедура ещё фиксируется |
| N09 | Nondeterministic identity basis | Эквивалентные входы при другом runtime order дают те же canonical outputs. Fixture не подставляет собственный hash |
| N10 | Attempted semantic backflow | Внешнее representation не переписывает calculus; требуется ещё отсутствующая projection boundary |
| N11 | Attempted future synthesis | Missing source/target lineage не порождает O/M/R/C/S/T. Сравнить counts, unresolved refs и invalid object retention |

`FAIL / UNRESOLVED / INVALID` для negative case выбираются объявленным contract,
а не по имени теста. INVALID — lifecycle status, не четвёртый ConformanceResult.
Для конфигурационного отказа или unknown contract semantic result может быть
отсутствующим; execution report обязан явно указать причину.

## Снимки, namespace и acceptance

Перед каждой downstream transformation фиксируются все затронутые upstream
semantic objects. Сравниваются полные canonical bytes, включая применимые
context, criteria/results, provenance и status; одного `.id` недостаточно.
Два запуска сравнивают **весь O/M/R/C/S/T/E graph**, не только финальную Evaluation.
Native u64 identity не объявляется формально уникальной или межплатформенной.

Forbidden vocabulary проверяется только в semantic artifact namespace:
WAVE, TREND, IMPULSE, FIBONACCI, ELLIOTT, SIGNAL, BUY, SELL, CAUSE, PREDICTION.
Упоминание слова в diagnostics не является semantic inflation. Разрешённое внешнее
Representation также не превращается автоматически в calculus label.

Gate может получить PASS только при одновременном выполнении условий:

- каждый A01…A12 и N01…N11 реально исполнен, получил harness PASS и совпал с ожиданием;
- canonical graphs двух запусков эквивалентны;
- все before/after upstream snapshots совпали;
- forbidden semantic artifacts отсутствуют;
- projection isolation проверена через объявленный registered profile;
- ни один mandatory case не пропущен, не assumed-pass, не BLOCKED и не NOT_EXECUTED.

Execution report сохраняет fixture/implementation/contracts/input versions,
команды и окружение, canonical artifacts, case IDs, expected/actual values,
semantic result, lifecycle status, diagnostics и evidence references.
Его минимальные обязательные fields перечислены в manifest. `NOT_EXECUTED`
принадлежит harness; semantic result — только PASS/FAIL/UNRESOLVED, либо null,
когда semantic evaluation не состоялась.

## Текущие blockers и ограничения

1. Нет единого runner, который исполняет и записывает все обязательные cases.
2. Нет registered projection profile/mapping для A12 и N10. Простой доступ к
   readonly объекту или копирование строки label не заменяют эту операцию.
3. Нужно заморозить конкретные executable bindings negative cases, особенно N08,
   input dataset и версии всех контрактов; наличие component tests этого не делает.

006 сохраняет typed invalid результаты **failed construction**. Это не означает
операцию ретроспективной invalidation уже admitted objects или автоматическую
invalidity propagation по descendants. Captured invalid lineage не обновляется
задним числом после позднего admission отсутствовавшего input. Полная reference
closure и admissibility остаются разными свойствами.

Horizon ограничивает source timestamp; available_at/decision_at, внешний source
trust и general historical as-of knowledge не появляются вследствие E2E PASS.
Полный PASS в будущем будет относиться только к объявленному profile, fixture,
contracts и implementation surface.
