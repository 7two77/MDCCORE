# MDC Core

Подготовка первого Git commit и CI описана в [MDC-OPS-001](docs/MDC-OPS-001.md).

Восстанавливаемый Rust-пакет `mdc_core v0.1.0`. Активный пакет расположен в корне:
`Cargo.toml`, `src/`, `tests/`. Вложенный каталог `mdc_core/` сохранён как исходный
материал для сравнения; он не подключён как workspace member.

Текущий рабочий пакет — **MDC-IMPL-008: Lifecycle invalidation и зависимости**.
[Контракт MDC-LIFECYCLE-001/1](docs/MDC-LIFECYCLE-CONTRACT-v0.1.md),
[реализация и проверки](docs/MDC-IMPL-008.md),
[матрица v0.2](docs/MDC-CONTRACT-TO-IMPLEMENTATION-MATRIX-v0.2.md).
Исторические решения и результаты MDC-IMPL-001…007/OPS-001 сохранены в `docs/`.

**MDC-E2E-CONFORMANCE-001: PASS для объявленного fixture MDC-SCALAR-001.**
Все 12 assertions E2E-A01…A12 и 11 negative cases N01…N11 выполнены в двух
раздельных OS-процессах. [Execution evidence](docs/evidence/mdc-e2e-conformance-001-impl-008.json)
содержит фактические результаты, canonical bytes, lineage и snapshots. PASS
применим к записанным source fingerprint, toolchain, features и target; текущую
применимость отчёта проверяет `check_gate_readiness.py`.

Пакет содержит исполнимый ограниченный профиль `MDC-SCALAR-001`: schema/source/
operator-контракты с точными версиями, admission наблюдений, чтение signed-i64
значения и упорядоченную разность. Зарегистрированные Relation contracts создают
PRECEDES, TEMPORAL_DISTANCE, ADJACENT и PROPORTION; Composition contracts проверяют
явные constraints и замкнутость ссылок. State Derivation Contracts вычисляют
трёхзначные criteria, а Transition Contracts связывают два существующих State
через explicit conditions и временные требования. Работа выполняется через
`Pipeline`, который сохраняет recursive lineage, реальные witness references
и аудит отклонённых запросов. Старые raw Observation/Relation/Composition/State/
Transition остаются API совместимости и сами по себе не допускают объекты к
новым операциям.

Целевые конфигурации библиотеки:

| Конфигурация | Назначение |
|---|---|
| Default / `std` | Стандартная библиотека; `std` включает `alloc` |
| `--no-default-features --features alloc` | `no_std` с динамической памятью |
| `--no-default-features` | Не поддерживается текущей моделью; ожидается явная ошибка компиляции |

Проверки выполняются из корня пакета:

```bash
cargo test --locked --offline
cargo test --locked --offline --no-default-features --features alloc
cargo check --locked --offline --no-default-features --features alloc
cargo run --locked --offline --example source_measurement
cargo run --locked --offline --example relation_composition
cargo run --locked --offline --example state_transition
cargo run --locked --offline --example invalid_artifacts
cargo run --locked --offline --example lifecycle
python3 scripts/run_e2e.py
python3 scripts/check_gate_readiness.py
python3 -m unittest discover -s tests -p 'test_e2e_report.py'
```

Полная матрица команд, результатов и compatibility checks записывается в
[evidence MDC-IMPL-008](docs/evidence/impl-008.json).
Подтверждены 145/145 Rust tests и 17/17 Python checks в каждом из шести локальных
запусков: Rust 1.85.1 / 1.98.0 × default, `std,alloc`, `no_std + alloc`.
Format, Clippy, старый E2E и два прогона lifecycle example проходят.
Hosted CI пакета 008 ещё NOT_EXECUTED; опубликованный CI baseline проверял a8c15c7.
Runner исполняется на std-хосте; библиотека сохраняет конфигурацию no_std+alloc.
Python validator независимо проверяет покрытие cases, ожидаемые semantic
results, graph/snapshot evidence, projection isolation и fingerprint реализации.

Шесть typed invalid partial bodies сохраняют доступные semantic fields,
definitions, actual results и creation-time lineage snapshots. Audit receipt
связывается с объектом через `invalid_reference()`. Full canonical keys позволяют
сохранить разные failures при одинаковом digest; память для nested keys выше.
Подробности retention приведены в [MDC-IMPL-006](docs/MDC-IMPL-006.md).

Полная reference closure не означает admissibility: invalid objects нельзя
использовать как admitted inputs. Существующий invalid target Evaluation
получает TargetInvalid/Fail, а не missing-target Unresolved. Последующая
invalidation admitted objects выполняется по versioned LifecycleContract с
propagation по required declared inputs. Исторические canonical snapshots
сохраняются; текущий status доступен через `Pipeline::lifecycle_view()`.
Повторная Evaluation отозванного target сохраняет TargetInvalidated, lifecycle
epoch и причинные event keys. Новые derivation/projection paths проверяют usable границу.

State и Transition с `Pass` либо `Unresolved` остаются `Candidate`; `Fail`
отклоняет derivation с сохранением всех вычисленных результатов. Наличие
Observation после интервала проверяется по реально принятым фактам и заданному
horizon. Horizon ограничивает объявленный timestamp источника, а не момент
доступности данных; полная защита backtest от lookahead этим не устанавливается.

Тесты `no_std + alloc`, исполняемые на обычном хосте, не доказывают готовность к
конкретной bare-metal-платформе или наличие её allocator. Сигнатура публичного
trait `canonical_basis()` изменена с `&[u8]` на `Vec<u8>` в предыдущем пакете;
инструкции миграции приведены в [MDC-IMPL-001](docs/MDC-IMPL-001.md).

Technical projection использует зарегистрированный versioned profile и mapping
`TEST-REP-01/1`. Требуется сохранённая Conformant/Pass Evaluation, связанная с
точным target, его semantic bytes, contract и context. Внешняя Representation
не входит в calculus graph; изменение её label не меняет ядро. Обратная операция
явно возвращает `BackflowForbidden`.

PASS fixture не устанавливает универсальное соответствие MDC для любых данных,
контрактов, платформ или будущих изменений. Профиль scalar не реализует
произвольные schemas, units conversion, рыночную интерпретацию, общие external
evidence contracts, reinstatement или durable lifecycle recovery. Native Rust
Hash и локальная политика коллизий не дают межплатформенной гарантии identity.
Отдельная retention коллизий Evaluation остаётся ограничением MDC-IMPL-005/006.

VAIXLNS-Builder и STSA-MC не входят в текущую область работ.

[Readiness matrix MDC-IMPL-006](docs/MDC-E2E-GATE-READINESS-v0.1.md) и её manifest
сохранены как историческая подготовка: NOT_EXECUTED / BLOCKED. Их отдельная
проверка `python3 scripts/check_gate_readiness.py --manifest-only` ожидаемо
возвращает exit 2. Текущий checker по умолчанию проверяет execution report,
возвращая exit 0 только при PASS и совпадении fingerprint текущего кода.
