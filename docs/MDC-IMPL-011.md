# MDC-IMPL-011 — Exact recorded session snapshot roundtrip

Дата: 2026-11-03. Подготовлено поверх интегрированного MDC Core и решения
`684bae3580d581ebecba629ae247411ecc72779c`: первым потребителем выбран
исторический replay с переходом к длительно работающему сервису; после
перезапуска требуется точное восстановление контекста ядра.

Пакет реализует opt-in recorded session и self-contained binary export/restore.
Контракт: [MDC-SNAPSHOT-001/1](MDC-SNAPSHOT-CONTRACT-v0.1.md). Это основа
snapshot roundtrip; storage backend и историческая доступность информации
остаются следующими отдельными решениями.

## Что добавлено

`SnapshotSession`, `SnapshotLimits`, `SnapshotError` опубликованы из
`mdc_core::pipeline`. Сессия принимает полный frozen Registry и identity protocol,
начинает с пустого Pipeline и записывает все девять изменяющих операций ядра:
Observation, Measurement, Relation, Composition, State, Transition, Evaluate,
EvaluateKeyed, Invalidate. Выполняются production algorithms; собственного
fixture-алгоритма вычисления semantic objects нет.

Каждый вызов сохраняет original typed request и actual typed outcome, включая
ошибки и идемпотентные повторы. `pipeline()` предоставляет readonly view; обычные
Pipeline и его identity defaults сохраняются. Присоединение к уже заполненному
незаписываемому Pipeline не добавлено. Projection остаётся внешней операцией
чтения и не увеличивает записанную историю.

Основные файлы:

| Файл | Обязанность |
|---|---|
| `src/pipeline/snapshot.rs` | Public session API, журнал вызовов, header, export и ordered restore |
| `src/pipeline/snapshot/wire.rs` | Bounded fixed-width storage primitives и raw lookup reference codec |
| `src/contract/registry/snapshot.rs` | Все 14 Registry maps, dependency validation и canonical storage order |
| `src/pipeline/snapshot/operation.rs` | Девять request tags и полные typed outcomes/errors |
| `src/pipeline/snapshot/state.rs` | Полный retained-state manifest и inventory |
| `tests/snapshot_roundtrip.rs` | Public roundtrip, usability, limits и malformed archive checks |
| `examples/support/snapshot_fixture.rs` | Реальный граф, historical failures, lifecycle и unused definitions |
| `examples/snapshot_roundtrip.rs` | Producer и отдельный consumer process |
| `scripts/check_snapshot_roundtrip.py` | Сравнение archive/manifest bytes и process receipts матрицы |

Registry codec сохраняет exact definitions и versions, включая неиспользованные
source/schema/operator, structure/dynamics, conformance/projection/lifecycle
definitions. Decode использует register APIs и exact re-encoding check: malformed
definitions, duplicates, неправильный порядок и ненормализованные unordered
списки отвергаются.

Request references сохраняют supplied digest и полный key. Это существенно для
исторического отказа по неверному lookup handle: restore не исправляет его в
корректный key и не меняет первоначальный результат. Отдельные tags Evaluate и
EvaluateKeyed сохраняют numeric alias reservation history MDC-IMPL-009.

## Exact state вместо повторной оценки конечного графа

Restore сначала полностью декодирует контейнер, затем создаёт новый Pipeline
и исполняет исходные calls по порядку. Каждый фактический outcome сравнивается
побайтово с записанным. Retained-state limits проверяются после каждого вызова;
полный final state manifest должен совпасть целиком.

Manifest включает Registry, storage keys и canonical bodies всех O/M/R/C/S/T/E,
scalar cache и statuses, Evaluation full keys/aliases, typed invalid artifacts,
provenance, lifecycle birth/current invalid sets/events и оба audit journals.
Сохраняются multiplicity отказов, lifecycle revision и request idempotence.
Сравнение digest или только количества объектов не используется как замена
проверке этих данных.

Исходные missing snapshots и Evaluation не пересчитываются на основании позднее
добавленных inputs. Snapshot status и актуальный lifecycle view остаются
различимыми. Ошибка импорта отбрасывает свежий candidate и не изменяет исходную
живую сессию.

## Формат и эксплуатационные пределы

Header: 17-byte `MDC-SNAPSHOT-001\0`, format u16 LE `1`, engine u16 LE `1`,
profile `0` legacy либо `1` portable. Legacy дополнительно хранит usize width
и endian. Далее: 14 Registry groups, typed operation log с expected outcomes,
полный manifest и EOF. Byte lengths/counts — u64 LE; enum tags явные.

Default caps: archive 16 MiB, byte field 2 MiB, manifest 8 MiB, collection
16 384, total items 65 536, definitions 4 096, operations 4 096. Их можно явно
изменить через SnapshotLimits. Live recording этими caps не ограничено; слишком
большой журнал отклоняется при export с выбранными limits. Hard OOM и CPU/time
guarantees не заявлены. Длительная сессия хранит requests и outcomes в памяти;
compaction/checkpoint pruning не реализованы.

Legacy width/endian check предотвращает заведомо несовместимый native layout.
Exact outcome/manifest checks выявляют остальные replay расхождения; portable
identity не заменяет совместимость engine semantics. Future version потребует
сохранённого старого engine либо отдельной migration. Скрытого fallback нет.

## Проверки и evidence

Public scenarios покрывают пустую сессию и оба identity profiles; полный typed
граф и unused versioned definitions; все Evaluation outcomes и reference policies;
invalid artifacts всех шести видов; повторные audit receipts; late admission;
lifecycle epochs, invalidation и rejected/idempotent invalidation; дальнейшую
работу восстановленного Pipeline; projection boundary; history order; readonly
queries; raw malformed lookup handles; truncation, unknown versions/tags,
outcome tampering, trailing bytes, legacy layout mismatch и все limits.

Private tests проверяют wire boundaries, Registry canonicality, exact state
oracle и принудительные collision cases через уже существующие cfg(test) seams.
Production API для подмены hash не добавлен.

Текущий отдельный producer/consumer сценарий воспроизводит 38 calls, 31 retained
artifact, lifecycle revision 2, 9 semantic rejection receipts и 2 lifecycle
rejection receipts. Это характеристики данного fixture, не минимальная полнота
произвольного пользовательского графа. Два consumer process требуют exact
archive re-export и state_manifest equality. Sidecar `.state` — тестовый oracle;
restore API использует только self-contained `.bin`.

CI runner исполняет snapshot producer и два consumer process для каждого native
profile. Cross runner исполняет snapshot tests и те же три процесса на i686 и
s390x. Comparison script требует десять согласованных configuration receipts,
одинаковые archive и manifest SHA-256, а также корректные source/run metadata.

Локальная матрица Rust 1.85.1 / 1.98.0 × default, std-alloc, alloc завершилась:
**209/209 Rust tests и 25/25 Python checks** в каждой из шести конфигураций.
Format, Clippy, library check, E2E 23/23 в двух процессах и примеры проходят.
[Полный local receipt](evidence/impl-011-local-verification.json) содержит версии,
исходный fingerprint, hashes логов и шесть результатов. Проверки Python включают
17 E2E, четыре identity comparison и четыре snapshot negative controls.

[Frozen vector](../tests/fixtures/snapshot-roundtrip-v1.json) закрепляет portable
archive: 372782 bytes, SHA-256
`349f902ecd41fec5da8c158eba913fe8812bace0dcfb9283547c3a250f796928`;
manifest: 269158 bytes, SHA-256
`f9a2f97d8df14c0a50cb9a99a9694c89938ac1396be332d7ed069695f039d907`.
Все шесть локальных конфигураций совпали с этими controls. Hosted cross-target
verification оформляется отдельным receipt после publication. Прежний E2E
сохраняет область scalar conformance; новый roundtrip имеет самостоятельные проверки.

## Команды воспроизведения

```bash
cargo test --locked --offline --test snapshot_roundtrip
cargo test --locked --offline --no-default-features --features alloc --test snapshot_roundtrip

cargo run --quiet --locked --offline --example snapshot_roundtrip -- produce /tmp/mdc-snapshot.bin
cargo run --quiet --locked --offline --example snapshot_roundtrip -- consume /tmp/mdc-snapshot.bin
cargo run --quiet --locked --offline --example snapshot_roundtrip -- consume /tmp/mdc-snapshot.bin

bash .github/ci/run.sh default
bash .github/ci/run.sh std-alloc
bash .github/ci/run.sh alloc
```

Для сравнения скачанных CI artifacts: `python3 -B scripts/check_snapshot_roundtrip.py
--compare-root ARTIFACTS_DIR --output RECEIPT.json`. Нужны все десять artifacts
одного checkout/run; неполная локальная выборка не заменяет этот receipt.
Cross runner требует настроенные target libraries, linker/runtime и QEMU из CI.

## Совместимость и следующие решения

Пакет добавляет APIs и отдельный storage protocol; существующий default identity
и semantic canonical encoding не заменяются. Core остаётся без новых external
dependencies и поддерживает std и no_std + alloc. Вложенная исходная копия не
используется как активный пакет. Исторические evidence сохраняются отдельно от
нового execution receipt. Commit, publication и hosted verification фиксируются
отдельно после завершения проверок.

SnapshotSession восстанавливает recorded core context. Он не сохраняет торговые
позиции, состояние брокера/биржи, открытые заявки или внешний strategy state.
Файловый пример не добавляет WAL, fsync, atomic rename или crash recovery. Следует
отдельно определить место хранения, атомарную публикацию, поведение при неполной
записи, совместимость следующих engine versions и политику роста истории.

Для причинно корректного исторического replay дополнительно нужны available_at,
decision_at и правила допуска поздно полученных свидетельств. Timestamp horizon
и lifecycle revision эти требования не заменяют. Они остаются расширением
профиля, а не свойством, автоматически полученным благодаря snapshot roundtrip.
