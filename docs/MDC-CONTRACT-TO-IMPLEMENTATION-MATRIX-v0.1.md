# MDC Core — Contract-to-Implementation Matrix v0.1

Baseline: MDC-IMPL-007, MDC-SCALAR-001, дата 2026-11-02.
Нормативное основание — предоставленный `One MDC.md`, **STP-MDC v0.1 DRAFT / NORMATIVE**,
MDC-01…06, Conformance Boundary и MDC-E2E-CONFORMANCE-001.
Статус DRAFT сохраняется; пакет не утверждает спецификацию от имени её автора.
MA-000, VAIXLNS-Builder и STSA-MC не являются основанием этой матрицы.

## Значение статусов

- **IMPLEMENTED** — названный механизм реализован и исполнен в объявленном scalar-профиле.
- **PARTIAL** — часть механизма есть; в строке явно указано оставшееся ограничение.
- **MISSING** — названного механизма в текущей реализации нет.
- **UNVERIFIED** — достаточного свидетельства свойства нет; это не доказательство нарушения.

Эти статусы относятся к строкам и их границам. IMPLEMENTED для ограниченного
профиля не означает универсального соответствия всем возможным MDC-контрактам.
Отсутствующая функция не объявляется нормативным нарушением без отдельного
сопоставления с применимым обязательным требованием.

## Реализованный профиль

| Контракт / обязанность | Статус | Реализация | Проверка / evidence | Граница утверждения |
|---|---|---|---|---|
| MDC-01: admission Observation | IMPLEMENTED | [pipeline](../src/pipeline.rs), [Observation](../src/model/observation.rs), [Registry](../src/contract/registry.rs) | [source tests](../tests/source_measurement.rs), E2E A01/A03 | Exact source/schema/version/context, SignedI64Le payload; declared Source metadata |
| MDC-02: Measurement | IMPLEMENTED | [pipeline](../src/pipeline.rs), [Measurement](../src/model/measurement.rs) | [source tests](../tests/source_measurement.rs), E2E full graph | Scalar и ordered Difference, units/context, checked overflow и lineage |
| MDC-03: Relation | IMPLEMENTED | [structure operations](../src/pipeline/structure.rs), [contracts](../src/contract/structure.rs) | [structure tests](../tests/relation_composition.rs) | Precedes, TemporalDistance, Adjacent, Proportion; version/context/scope и references |
| MDC-04: Composition | IMPLEMENTED | [structure operations](../src/pipeline/structure.rs), [Composition](../src/model/composition.rs) | [structure tests](../tests/relation_composition.rs), E2E N02 | Referential closure, шесть вычисляемых constraints, явный порядок members |
| MDC-05: State Derivation | IMPLEMENTED | [dynamics operations](../src/pipeline/dynamics.rs), [contracts](../src/contract/dynamics.rs) | [dynamics tests](../tests/state_transition.rs), E2E A06/N04 | CompositionAbstraction и пять видов criteria; реальные witnesses; horizon по source timestamp |
| MDC-06: Transition | IMPLEMENTED | [dynamics operations](../src/pipeline/dynamics.rs), [Transition](../src/model/transition.rs) | [dynamics tests](../tests/state_transition.rs), E2E A09/N06/N07/N11 | Два существующих State, четыре вида conditions и временной порядок; target не синтезируется |
| Canonical snapshot | IMPLEMENTED | [snapshot](../src/canonical/snapshot.rs) | [foundation tests](../tests/technical_foundation.rs), [allocation regression](../tests/allocation_regression.rs) | Один owned buffer; identity вычисляется по сохранённым bytes; прежняя утечка устранена |
| Identity и коллизии | PARTIAL | [identity](../src/identity/mod.rs), collision guards в Pipeline | foundation, construction-order tests, [compatibility](evidence/impl-007-compatibility.json) | Прежний native Rust Hash/FNV u64; локальный запрет перезаписи; гарантия глобальной uniqueness не установлена |
| Provenance и reference resolution | IMPLEMENTED | [provenance](../src/provenance/mod.rs), [pipeline](../src/pipeline.rs), [captured invalid lineage](../src/pipeline/invalid.rs) | [invalid tests](../tests/invalid_semantic.rs), E2E A03/N08 | Recursive closure до зарегистрированного Source; captured snapshots не обновляются поздним admission |
| Conformance Boundary | IMPLEMENTED | [evaluation operations](../src/pipeline/conformance.rs), [contracts](../src/contract/conformance.rs) | [CB tests](../tests/conformance_boundary.rs), E2E A05…A07/N01…N05 | Шесть target types, пять встроенных criterion kinds, exact contract/context; target не изменяется |
| Трёхзначная агрегация | IMPLEMENTED | [evaluation](../src/contract/evaluation.rs) | CB/dynamics tests и E2E A05…A07 | FAIL > UNRESOLVED > PASS; INVALID — lifecycle, а не четвёртый semantic result |
| Typed invalid artifacts / lifecycle | PARTIAL | [invalid bodies](../src/model/invalid.rs), [retention](../src/pipeline/invalid.rs) | [invalid tests](../tests/invalid_semantic.rs), E2E A08 | Шесть typed failed-construction bodies и full keys; последующая invalidation admitted graph отсутствует |
| E2E conformance gate | IMPLEMENTED | [Rust fixture](../examples/support/mdc_e2e.rs), [launcher](../scripts/run_e2e.py), [validator](../scripts/e2e_report.py) | [Rust gate tests](../tests/e2e_gate.rs), [validator tests](../tests/test_e2e_report.py), [execution bundle](evidence/mdc-e2e-conformance-001.json) | Все A01…A12/N01…N11; два PID, три full-graph runs, prefix snapshots; scoped PASS |
| Technical projection isolation | IMPLEMENTED | [projection contracts](../src/contract/projection.rs), [operation](../src/pipeline/projection.rs), [Representation](../src/model/representation.rs) | [projection tests](../tests/projection_boundary.rs), E2E A11/A12/N10 | Только TEST-REP-01/1, Conformant/Pass Evaluation binding; detached representation; reverse import запрещён |
| std и no_std + alloc | IMPLEMENTED | [manifest](../Cargo.toml), [library](../src/lib.rs) | [verification](evidence/impl-007.json): 124/124 × 3 | Hosted tests и library check; без alloc поддержка не заявлена |

## Открытые механизмы и свойства

| Область | Статус | Подтверждённая граница / необходимое следующее свидетельство |
|---|---|---|
| `available_at` / `decision_at`, исторический as-of replay | MISSING | Source timestamp/horizon не определяет фактическую доступность знания; нужны отдельная модель времени и temporal fixtures |
| Последующая invalidation admitted objects и propagation | MISSING | Retained invalid construction не инвалидирует ранее принятый граф; нужны lifecycle contract и операции распространения |
| Отдельный collision-retention namespace Evaluation | MISSING | Собственная коллизия Evaluation даёт ошибку без перезаписи; full-key retention шести invalid namespaces не распространяется на Evaluation |
| Переносимый identity protocol | MISSING | Stable wire encoding/hash protocol вместо native Rust Hash пока не принят; миграция требует версионирования и совместимости |
| Внешняя подлинность Source / истинность payload | UNVERIFIED | Declared metadata и PASS критериев не удостоверяют реальный источник или истинность данных |
| Формальная semantic uniqueness identity | UNVERIFIED | Примеры различения и collision guards не доказывают инъективность u64 |
| Детерминизм между архитектурами / toolchains | UNVERIFIED | Проверены отдельные процессы одной Rust 1.85.1 x86_64 среды; необходима отдельная матрица платформ |
| Конкретная bare-metal платформа | UNVERIFIED | Нужны target-specific allocator/linker/runtime checks |
| Произвольные schemas/operators, units conversion, market projection profiles | MISSING | Не реализованы текущим scalar-профилем; их требования определяются отдельно |

Source fingerprint и точные результаты собраны в [Baseline v0.1](MDC-CORE-BASELINE-v0.1.md).
Исторические статусы PARTIAL/BLOCKED в MDC-IMPL-001…006 сохраняют значение для
своих этапов. Текущий E2E результат берётся из MDC-IMPL-007 execution bundles;
прежний readiness manifest не подменяет отчёт об исполнении.
