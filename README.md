# MDC Core

История объединения исходной main и проверенного стека baseline → 008 → 009 → 010,
сохранённые материалы и правила сопровождения описаны в
[MDC-INTEGRATION-001](docs/MDC-INTEGRATION-v0.1.md).
Актуальная [матрица v0.15](docs/MDC-CONTRACT-TO-IMPLEMENTATION-MATRIX-v0.15.md)
учитывает опубликованные hosted CI receipts. Подготовка первого Git commit
сохранена в [MDC-OPS-001](docs/MDC-OPS-001.md).

Восстановленный Rust-пакет `mdc_core v0.1.0`. Активный пакет расположен в корне:
`Cargo.toml`, `src/`, `tests/`. Вложенный каталог `mdc_core/` сохранён как исходный
материал для сравнения; он не подключён как workspace member.

Последний функциональный пакет — **MDC-IMPL-015: durable replay recovery**.
[API, проверки и ограничения](docs/MDC-IMPL-015.md),
[формат журнала и HEAD](docs/MDC-REPLAY-RUN-WIRE-v1.md).
CSV, модель и план cutoffs сохраняются вместе с префиксом отдельных решений.
Курсор продвигается после journal sync и atomic HEAD publication/directory sync;
recovery восстанавливает и пересчитывает каждый изолированный snapshot.

MDC-IMPL-014: Binance CSV и bounded replay.
[Адаптер и команды](adapters/mdc_replay/README.md),
[моделируемый временной контракт](docs/MDC-BINANCE-REPLAY-CONTRACT-v0.1.md),
[проверки и ограничения](docs/MDC-IMPL-014.md).
Доступность свечей моделируется; каждый cutoff получает отдельный граф только
из доступных баров. Рыночная стратегия, доходность и ордера не реализованы.

MDC-IMPL-013: automatic time/count flush.
[Scheduler API, проверки и ограничения](docs/MDC-IMPL-013.md).
Основной WAL/recovery adapter добавлен в MDC-IMPL-012.
[Реализация, проверки и ограничения](docs/MDC-IMPL-012.md),
[host crate](adapters/mdc_store/README.md),
[wire contract](docs/MDC-WAL-WIRE-CONTRACT-v1.md).

Независимый `mdc_store` сохраняет полный префикс истории через synced WAL и
atomic HEAD. Buffered receipt допускает потерю suffix; строгий barrier сохраняет
все предыдущие inputs/outcomes до выдачи durable ACK. Добавлены host request IDs,
защита от второго writer, error fencing, checkpoint/rotation и exact replay.
ScheduledStore добавляет один worker, bounded queue, oldest-pending timer и
count threshold с явной BatchPolicy. Raw Store сохраняет explicit flush API.
Это Linux/local-files/single-writer профиль без жёсткого временного SLA,
исторической доступности данных или гарантии для любого оборудования.

Основа — [snapshot MDC-IMPL-011](docs/MDC-IMPL-011.md): SnapshotSession с frozen
Registry, записью всех mutators с создания и exact outcome/state validation.
Произвольный ранее созданный незаписанный Pipeline не импортируется.
[Решения владельца](docs/MDC-DECISIONS-2026-11-03-v0.1.md): historical replay →
service и exact restart context. Исполнение сделок и external cursor/broker
state требуют отдельных contracts. Моделируемые
available_at/decision_at реализованы только в bounded replay adapter 014.

Portable identity добавлена в [MDC-IMPL-010](docs/MDC-IMPL-010.md).
`Pipeline::new()` сохраняет legacy identity; portable graph явно выбирается через
`Pipeline::with_identity_protocol(registry, IdentityProtocol::PortableV1)`.
Профиль применяется ко всей цепочке, включая invalid artifacts и lifecycle journal.
Прежние решения 001…009/OPS-001 сохранены в `docs/`.

**MDC-E2E-CONFORMANCE-001: PASS для объявленного fixture MDC-SCALAR-001.**
Все 12 assertions E2E-A01…A12 и 11 negative cases N01…N11 выполнены в двух
раздельных OS-процессах. [Execution evidence](docs/evidence/mdc-e2e-conformance-001-impl-015.json)
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
cargo run --locked --offline --example evaluation_retention
cargo run --locked --offline --example portable_identity
cargo run --locked --offline --example identity_migration
cargo run --locked --offline --example snapshot_roundtrip -- produce /tmp/mdc-snapshot.bin
cargo run --locked --offline --example snapshot_roundtrip -- consume /tmp/mdc-snapshot.bin
python3 scripts/run_e2e.py
python3 scripts/check_gate_readiness.py
python3 -m unittest discover -s tests -p 'test_e2e_report.py'
```

Матрица команд, результатов и compatibility checks записана в
[evidence MDC-IMPL-013](docs/evidence/impl-013-local-verification.json).
Подтверждены 213/213 Rust tests и 25/25 Python checks (17 E2E + 4 identity comparison + 4 snapshot) в каждом из шести локальных
запусков: Rust 1.85.1 / 1.98.0 × default, `std,alloc`, `no_std + alloc`.
Format, Clippy, прежний E2E, lifecycle, keyed-reference, portable и snapshot примеры проходят.
[Hosted CI 009](https://github.com/7two77/MDCCORE/actions/runs/41614270848) прошёл
для d33bb97. CI ветки 010 дополнительно исполняет portable tests/graph на i686
и s390x/QEMU и требует совпадения всех десяти matrix configurations.
011 добавляет 15 snapshot tests для cross targets, producer и два отдельных
consumer process, frozen archive/manifest vectors и их сравнение между targets.
Runner исполняется на std-хосте; библиотека сохраняет конфигурацию no_std+alloc.
Host adapter отдельно проходит 36 default и 38 fault-injection tests на обеих
версиях Rust. SIGKILL проверяет 18 publication/rotation boundaries. Power-loss
validation и production-scale SLO не заявлены.
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
evidence contracts или reinstatement. Bounded Linux recovery реализован отдельно
от core; его гарантии зависят от filesystem/device sync и заданных limits. Recorded snapshots
сохраняют core context, но не позиции/ордера внешнего торгового adapter. WAL/HEAD recovery добавлен в host crate. History compaction и
general core available_at/decision_at semantics отсутствуют; 014 изолирует
доступность в отдельном профиле adapter.
Restore имеет стоимость replay истории; import limits не ограничивают обычный SnapshotSession live recording;
mdc_store отдельно ограничивает число calls и размеры записей.
Native Rust
Hash остаётся непереносимым legacy protocol. Новый opt-in portable v1 имеет
явные bytes/hash rules и отдельный cross-target gate; гарантия ограничена
заявленными profiles, inputs и targets. Глобальная уникальность u64 не доказана.
Evaluation хранится по полному canonical key; коллизия digest не перезаписывает
другие результаты. Для новых consumers доступны `evaluate_keyed()` и
`ObjectRef::EvaluationKey`. Numeric aliases закреплены за первым reservation;
canonical keyed references следует использовать для collision-independent naming.
Внешние exhaustive match ObjectRef требуют обработки нового variant.

VAIXLNS-Builder и STSA-MC не входят в текущую область работ.

[Readiness matrix MDC-IMPL-006](docs/MDC-E2E-GATE-READINESS-v0.1.md) и её manifest
сохранены как историческая подготовка: NOT_EXECUTED / BLOCKED. Их отдельная
проверка `python3 scripts/check_gate_readiness.py --manifest-only` ожидаемо
возвращает exit 2. Текущий checker по умолчанию проверяет execution report,
возвращая exit 0 только при PASS и совпадении fingerprint текущего кода.

Host verification: `bash .github/ci/store.sh`. Новый host crate требует explicit
`cargo fetch --locked --manifest-path adapters/mdc_store/Cargo.toml` перед offline
проверками. Root core остаётся без новых зависимостей.

После PR #8: [проверка main](docs/MDC-OPS-004-POST-AUTO-FLUSH-v0.1.md) и
[draft требований historical availability/replay](docs/MDC-TEMP-001-SCOPE-v0.1-DRAFT.md).
Для первой итерации выбраны Binance Spot / BTCUSDT / 1m / CSV. Официальные малые
fixtures покрывают ms и us эпохи; все цены/объёмы точные e8, ошибки не превращаются
в нули. Technical State проверяет доступность свидетельств, а не торговый сигнал.
Storage PASS и технический replay не доказывают реальное время получения
исторических данных или готовность полноценной торговой системы.

После PR #9: [проверка main 9f21fe7](docs/MDC-OPS-005-POST-REPLAY-v0.1.md),
post-merge CI 16/16 PASS. Проект source/plan recovery принят в
[wire contract v1](docs/MDC-REPLAY-RUN-WIRE-v1.md) и реализован в отдельном RunStore
профиля 015. Исторический [проект контракта](docs/MDC-REPLAY-RECOVERY-CONTRACT-v0.1-DRAFT.md)
сохранён; source-run cursor не является состоянием брокера или ордеров.

После PR #10: [проверка main ee4d45f](docs/MDC-OPS-006-POST-RECOVERY-v0.1.md),
post-merge CI 16/16 PASS. Начальный release-пилот зафиксировал границу журнала
на конкретном наборе: из 256 запрошенных cutoffs сохранены 153, затем достигнут
лимит 64 MiB; префикс успешно восстановлен. Подготовлен
[проект воспроизводимых измерений](docs/MDC-REPLAY-CAPACITY-CONTRACT-v0.1-DRAFT.md).
Лимиты и runtime не изменены; общий production SLO пока не установлен.
