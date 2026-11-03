# MDC-SNAPSHOT-001/1 — Recorded session snapshot contract v0.1

Дата: 2026-11-03. Область: opt-in snapshot с точным восстановлением записанной
сессии MDC Core. Решения потребителя: исторический replay с дальнейшим развитием
в длительно работающий сервис; после перезапуска требуется точное восстановление
контекста ядра. Реализация storage backend и временная доступность информации
оформляются отдельно.

## Граница записи

`SnapshotSession::new(registry, protocol)` создаёт пустой `Pipeline` с переданным
полным Registry и выбранным `IdentityProtocol`. Registry после создания сессии
не изменяется. Все изменяющие операции проходят через методы сессии:

| Метод | Request | Результат |
|---|---|---|
| `admit` | `ObservationRequest` | `Result<ObjectRef, PipelineError>` |
| `measure` | `MeasurementRequest` | то же |
| `relate` | `RelationRequest` | то же |
| `compose` | `CompositionRequest` | то же |
| `derive_state` | `StateRequest` | то же |
| `transition` | `TransitionRequest` | то же |
| `evaluate` | `EvaluationRequest` | то же |
| `evaluate_keyed` | `EvaluationRequest` | то же |
| `invalidate` | `InvalidationRequest` | `Result<LifecycleEventId, LifecycleError>` |

Каждый вызов исполняет соответствующий production API ядра и сохраняет исходный
typed request и typed outcome. Сохраняются успешные, отклонённые и идемпотентные
повторные вызовы в исходном порядке. Ошибка вызова не исключает его из истории.

`pipeline()` возвращает только `&Pipeline` для чтения, lineage, lifecycle views
и projection. Mutable escape отсутствует. Projection не меняет retained state
ядра и не входит в журнал. Внешний Representation, состояние торгового счёта,
позиции, заявки и контекст стратегии этим контрактом не сохраняются.

Сессия не присоединяется к ранее заполненному незаписываемому Pipeline. Общего
импортера произвольного in-memory Pipeline нет. Для существующего потребителя
запись выбирается до первого admission. Обычный `Pipeline` остаётся доступен.

## Публичный API снимка

```rust
SnapshotSession::new(registry: Registry, protocol: IdentityProtocol) -> Self
SnapshotSession::pipeline(&self) -> &Pipeline
SnapshotSession::operation_count(&self) -> usize
SnapshotSession::inventory(&self) -> Vec<ObjectRef>
SnapshotSession::state_manifest(&self, limits: SnapshotLimits)
    -> Result<Vec<u8>, SnapshotError>
SnapshotSession::export(&self, limits: SnapshotLimits)
    -> Result<Vec<u8>, SnapshotError>
SnapshotSession::restore(bytes: &[u8], limits: SnapshotLimits)
    -> Result<Self, SnapshotError>
```

`inventory()` возвращает primary handles всех retained semantic artifacts,
включая invalid artifacts и Evaluation, по одному на artifact. Registry Sources
и отдельное пространство LifecycleEvent в этот список не входят. Экспорт и
чтение не изменяют сессию. Неудачный restore отбрасывает новый candidate;
он не получает ссылку на существующий Pipeline и не меняет его.

Снимок зависит от истории. Одинаковый конечный semantic graph, полученный
разными последовательностями вызовов, может иметь разные archive bytes.
Storage archive не заменяет canonical semantic basis или identity объекта.

## Binary storage format v1

Все многобайтовые целые в storage codec записываются little-endian, независимо
от платформы. Целые имеют фиксированную ширину u8/u16/u32/u64 или signed i128.
Bool допускает только 0 и 1. Строка — u64 byte length и UTF-8 bytes; byte field —
u64 byte length и bytes. Collection count — u64 с проверяемым преобразованием
в `usize`. Enum tags заданы явно; неизвестные значения отклоняются.

| Последовательность | Поля |
|---|---|
| Header | 17 bytes `MDC-SNAPSHOT-001\0`, u16 format `1`, u16 engine `1` |
| Identity profile | u8 `0` LegacyRustHashFnv64 или `1` PortableV1 |
| Только legacy | u8 `usize::BITS`, u8 endian (`0` little, `1` big) |
| Registry | 14 групп definitions в фиксированном порядке |
| Operations | u64 count; для каждой operation: tag, request fields, outcome byte field |
| State | отдельное length-prefixed полное state manifest |
| Конец | EOF; trailing bytes запрещены |

Format version описывает контейнер; engine version фиксирует replay semantics.
Этот codec отделён от semantic canonical encoding MDC-IMPL-010.

### Полный Registry

Группы: schemas, sources, operators, relations, constraints, compositions,
state_criteria, states, transition_conditions, transitions, conformance_criteria,
conformance, projections, lifecycle. В каждой группе сначала count, затем все
definitions в порядке BTreeMap по VersionedId. VersionedId содержит имя и u32
version; dependencies сохраняют точную version.

Сохраняются все 14 maps, включая неиспользованные definitions и несколько
зарегистрированных версий. Registry не извлекается только из reachable graph.
Restore регистрирует definitions через действующие register APIs в порядке
dependencies. Затем повторно кодирует нормализованный Registry и требует точного
совпадения со считанным участком. Дубликаты, перестановки и альтернативный порядок
нормализуемых unordered списков не принимаются как другой canonical archive.
Ошибки definitions не исправляются, версии не подменяются и не перезаписываются.

### Девять operation tags

| Tag | Request fields в порядке записи |
|---|---|
| 0 Admit | source, optional timestamp, payload, acquisition_context |
| 1 Measure | operator, ordered inputs |
| 2 Relate | contract, left, right |
| 3 Compose | contract, members, relations |
| 4 State | contract, composition, optional observation_horizon |
| 5 Transition | contract, source, target, optional observation_horizon |
| 6 Evaluate | contract, target, DynamicsContext |
| 7 EvaluateKeyed | contract, target, DynamicsContext |
| 8 Invalidate | request_id, contract, target, reason, evidence, expected_revision |

Optional u64 содержит bool presence и значение при наличии. References и списки
в requests сохраняют исходный порядок, дубликаты и supplied lookup handles.
Full-key reference сохраняет и supplied digest, и полный key, включая ошибочные
lookup values: decoder не пересчитывает digest, не «чинит» reference и не объявляет
его admitted object. Семантическую допустимость проверяет replay production API.

Evaluate и EvaluateKeyed имеют разные tags: это сохраняет историю numeric alias
reservation. Invalidation evidence сохраняется до нормализации операции ядром.

Каждый outcome — exact byte field со своим typed discriminant: Object Ok,
Object Err, Lifecycle Ok или Lifecycle Err. Object Ok содержит точный ObjectRef;
Lifecycle Ok — digest и полный event key. Ошибки сохраняют все поля production
ошибки, включая полные event handles при InputInvalidated. Outcome не заменяется
общим PASS или digest.

## Полный state manifest

Manifest начинается с `MDC-SNAPSHOT-001/state/v1\0`, имени identity protocol
и текущей lifecycle revision. Фиксированные numbered sections:

| Section | Exact retained content |
|---|---|
| 0 | Полный Registry |
| 1 | Observation storage key, object ID, canonical body, scalar cache i128, admission identity profile и snapshot status |
| 2…6 | Measurement, Relation, Composition, State, Transition: storage key, object ID, canonical body, snapshot status и derivation provenance |
| 7 | Evaluation full storage key, object ID/body, preferred primary reference, snapshot status и provenance |
| 8 | Numeric Evaluation alias и его полный key-владелец |
| 9 | Invalid artifact storage reference, artifact reference, digest, full key, canonical body и snapshot status |
| 10 | Lifecycle birth reference и revision |
| 11 | Текущий набор lifecycle-invalid references |
| 12 | Lifecycle events в порядке истории: digest, full key и canonical body |
| 13 | Все semantic rejection receipts: исходный request, полная ошибка и invalid reference |
| 14 | Все lifecycle rejection receipts: исходный request, полная ошибка и observed revision |

Canonical bodies включают зафиксированные definitions/context/results/lineage;
storage keys, cached values, aliases и provenance кодируются дополнительно,
чтобы совпадение bodies не скрывало другой retained mapping. Multiplicity audit
receipts и порядок событий сохраняются. Текущий lifecycle status и старый
snapshot status различаются точно так же, как до экспорта. Pending computed
candidate запрещён как нестабильное состояние сессии.

Manifest сравнивается целиком побайтово. Проверка одного u64 identity или только
числа объектов не заменяет его.

## Restore и порядок причин

1. Проверить размер, header, версии и profile; для legacy — ширину и endian.
2. Декодировать и проверить весь Registry, operations/outcomes и ожидаемый
   manifest; потребовать EOF до начала replay.
3. Создать новый пустой Pipeline из восстановленных definitions и protocol.
4. Выполнить requests в исходном порядке. После каждого вызова сравнить exact
   typed outcome с записанным. При расхождении прекратить restore.
5. После каждой операции проверить bounded retained-state manifest. В конце
   сравнить полный final manifest с архивом и вернуть новую сессию.

Это ordered replay против исходных frozen definitions. Повторной оценки всего
графа на основе его конечного содержимого нет. Старый missing dependency не
становится разрешённым в прежнем invalid artifact после позднего admission;
старые Evaluation, события, snapshots и причины отказа не переписываются.
Decoder не создаёт admitted objects прямой записью непроверенных private fields.

## Ограничения ресурсов и ошибки

| SnapshotLimits | Default |
|---|---:|
| max_snapshot_bytes | 16 MiB |
| max_field_bytes | 2 MiB |
| max_collection_items | 16 384 |
| max_total_items | 65 536 |
| max_definitions | 4 096 |
| max_operations | 4 096 |
| max_manifest_bytes | 8 MiB |

Limits передаются явно в export, restore и state_manifest. Counts ограничивают
collection и суммарные items данного codec pass; definitions и operations
дополнительно имеют свои совокупные budgets. Byte fields и manifest имеют
отдельные length budgets. Arithmetic overflow и невозможное преобразование
length отклоняются. Большая сессия может потребовать больших explicit limits.

Live recording этими import/export limits не ограничено: retained graph,
requests и outcomes растут в памяти. Limit error не откатывает ранее исполненный
live вызов. Codec budgets не являются hard allocator/OOM guarantee или CPU/time
sandbox: реальная операция может выделить память до следующей проверки manifest.

Ошибки: LimitExceeded, Truncated, InvalidMagic, UnsupportedFormat,
UnsupportedEngine, UnsupportedProtocol, InvalidTag, InvalidUtf8,
InvalidDefinition, NonCanonicalRegistry, LegacyEnvironmentMismatch,
OutcomeMismatch (zero-based operation index), StateMismatch, TrailingBytes,
InvalidSessionState. Несовпадение outcome или manifest не допускает частичного
успешного восстановления.

## Совместимость и границы обещаний

Legacy archive требует совпадения width/endian; этих двух полей недостаточно
для обещания совместимости произвольного будущего Rust/toolchain. Exact outcome
и state comparison выявляют различия поведения replay engine. Неизвестная engine
version отклоняется. Для будущих изменений потребуется сохранённый совместимый
engine либо отдельная versioned migration; она в v1 не реализована.

PortableV1 убирает native layout из identity protocol. Межплатформенная проверка
снимка относится к заявленным engine, profiles, graphs и исполненным targets;
она не устанавливает универсальную совместимость будущих версий вычислений.
Согласованность archive не является аутентификацией: подпись/MAC не добавлены.

Контракт покрывает exact recorded session roundtrip в памяти. Durable storage,
WAL, atomic publication, fsync, recovery после обрыва записи, concurrent writers,
reinstatement и compaction/pruning истории не реализованы. Restore time и размер
archive растут вместе с историей; production-scale performance не подтверждена.
Horizon и lifecycle revision сохраняются, но available_at/decision_at и запрет
использования поздно полученной информации в историческом решении не добавлены.

Файловый пример пишет `.bin` и `.bin.state` обычными write calls. Sidecar `.state`
служит независимым тестовым oracle. Сам `SnapshotSession::restore()` использует
только `.bin`, в котором уже находятся Registry и expected manifest. Ни эти две
записи, ни их пара не заявлены atomic или crash-safe.
