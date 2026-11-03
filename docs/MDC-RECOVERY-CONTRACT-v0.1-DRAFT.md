# MDC-RECOVERY-001 — Durable recovery contract v0.1 DRAFT

2026-11-03. Статус: PROPOSED / OWNER DECISIONS PENDING / NOT IMPLEMENTED.
Основание: потребитель historical replay → long-running service и exact restart
restoration; MDC-IMPL-011 merged в main `27fbc0a` через PR #5.

## Что уже есть

`SnapshotSession` экспортирует self-contained recorded history и восстанавливает
её через production APIs с exact outcome/state checks. Это codec и in-memory
replay. Обычный write в примере 011 не задаёт durable commit, ACK или crash recovery.
Пакет не сохраняет broker orders, позиции или внешний strategy state.

Ниже предложены host storage semantics. Это новый проектный контракт; он не
переопределяет исходные STP-MDC требования и не заявляет уже реализованную гарантию.

## Два решения владельца

| Решение | Предлагаемая граница | Альтернатива / влияние |
|---|---|---|
| R-D01: platform/backend/writers | Linux, local filesystem, один writer | Remote DB, NFS, object store и несколько writers требуют других commit/locking semantics |
| R-D02: durable boundary | Каждая подтверждённая приложению операция сохранена до ответа | Explicit checkpoint допускает потерю/повтор операций после последнего подтверждённого checkpoint |

Оба ответа запрошены у владельца. До получения ответов они остаются PROPOSED.
Выбор checkpoint не эквивалентен обещанию сохранности каждого успешного mutator.
Для первого bounded профиля предлагается сохранять полный snapshot. WAL, pruning
и compaction — отдельная оптимизация после измерения стоимости этого решения.

## Предлагаемые общие инварианты

1. Core остаётся std/no_std+alloc и без filesystem/clock side effects. Host adapter
   отвечает за files, synchronization, writer exclusion и storage diagnostics.
2. Durable generation содержит immutable snapshot 011, storage format version,
   generation number, parent identity и полную проверку целостности контейнера.
   Алгоритм checksum и exact container layout фиксируются перед реализацией.
   u64 semantic identity не заменяет integrity check; authentication не заявляется.
3. Один session directory относится к одному frozen Registry/profile/engine.
   Неизвестная version, foreign history или нарушенная последовательность generation
   отклоняются. Подмена legacy profile на portable при recovery запрещена.
4. Startup сначала проверяет весь committed container и вызывает restore в свежем
   candidate. Active state публикуется только после полного успеха. Восстановление
   corrupted/unsupported committed state не создаёт молча пустой Pipeline.
5. Успешный durable ACK означает exact committed generation. После restart должны
   совпасть bytes, references, numeric aliases, captured lineage, historical
   Evaluation, lifecycle views/events и обе audit histories.
6. Semantic failure и storage failure различаются. Отклонённый semantic request
   может добавить invalid artifact/audit и потому тоже требует сохранения при
   режиме per-operation. Он не исключается из истории как «неуспешный вызов».
7. Нельзя подтвердить live result, затем пытаться сохранить его best-effort.
   Candidate isolation и правила publication должны исключить использование
   graph, который приложение считает committed, а storage ещё не сохранил.
8. После неоднозначной ошибки commit adapter блокирует новые записи до recovery.
   Не объявляет ни rollback, ни успешный commit без достаточного свидетельства.

## Предлагаемый local commit protocol при R-D01 = Linux/files

Разрешены только документированные local filesystem semantics с atomic rename
в пределах одного directory/filesystem и работающим sync. Общая гарантия для
любого mount, storage hardware, NFS или отключённого write cache не заявляется.

1. Получить удерживаемую процессом writer lock. Второй writer получает явный
   отказ до любых изменений. Stale lock после crash не удаляется по одному PID:
   конкретный механизм lock/release нужно выбрать и проверить на целевой OS.
2. Подготовить изолированный candidate и его bounded export. Ошибка export или
   semantic replay не меняет последнюю committed generation.
3. Создать новый temporary file в том же directory с exclusive creation; записать
   весь container, проверить write results, выполнить file sync.
4. Атомарно заменить committed path через rename. Выполнить directory sync.
5. Только после успешных sync опубликовать candidate как active и выдать ACK с
   generation/identity. Последовательность ACK и in-memory publication фиксируется
   API contract до кода; ошибка storage не маскируется под semantic Fail.

Если crash случился до ACK, caller не может заключить, что операция не committed:
recovery может обнаружить предыдущую или новую полную generation согласно crash
point. Нужен стабильный request ID и способ узнать outcome, прежде чем повторять
неидемпотентный вызов. Invalidation request_id ядра не является общим host request ID.

Temporary files не считаются committed. Recovery не выбирает «самый новый по mtime»
и не promotes orphan автоматически. Missing store отличим от corrupted store:
создание новой сессии требует explicit create; open существующей не заменяет её
пустой. Retention предыдущих generations и cleanup задаются до удаления файлов.

## Дополнительный контракт при per-operation ACK

Host request ID, ordered operation и outcome должны участвовать в durable receipt.
Повтор того же ID с тем же request возвращает прежний outcome; тот же ID с другим
request отклоняется. Проверяется crash после commit до доставки ответа: клиент
повторяет запрос и не создаёт второй artifact, audit receipt или lifecycle event.
Host deduplication не переписывает существующие core identities или историю 011.

Для корректной первой реализации допустима дорогая candidate copy/replay и полный
snapshot на каждый commit в пределах явных budgets. Throughput такого варианта не
объявляется достаточным для trading service без измерений. Durable receipt, core
snapshot и generation должны публиковаться атомарно как одно committed состояние.

При explicit checkpoint caller получает отдельный checkpoint ACK. Обычные core
mutators не обещают durability. External input cursor должен быть согласован с
checkpoint либо повтор данных должен быть детерминированным и явно поддержанным.
Это не exactly-once координация с биржей, брокером или внешней очередью.

## Acceptance scenarios для будущей реализации

| ID | Сценарий и ожидаемое наблюдение |
|---|---|
| R01 | Новый store → commit → другой процесс open: полное exact restoration и continuation |
| R02 | Failure при write/file sync до rename: прежняя committed generation читается |
| R03 | Terminate producer до/после write, file sync, rename и directory sync: recovery допускает только документированные полные generations |
| R04 | Успешный ACK → немедленный restart: именно подтверждённая generation сохраняется |
| R05 | Rename выполнен, ACK не доставлен: resolve/retry request ID не создаёт повторного semantic эффекта в per-operation режиме |
| R06 | Ошибка после rename/directory sync: indeterminate outcome, запрет новых writes до recovery |
| R07 | Truncated/corrupted/unknown-version committed file: отказ с диагностикой, без silent fallback/reset |
| R08 | Orphan temporary file и неполный container игнорируются как committed; cleanup безопасен |
| R09 | Второй writer отклоняется; после реального termination lock освобождается по контракту OS |
| R10 | Неуспешные semantic calls, invalid objects, idempotent invalidation и pinned aliases переживают recovery |
| R11 | Budget exhaustion/export failure не подтверждают новую generation и не меняют прежнее durable состояние |
| R12 | Explicit-checkpoint профиль, если выбран: cursor/replay и допустимый suffix loss проверяются отдельно |

Fault injection с simulated I/O errors и kill процесса доказывает лишь описанные
software/process-crash сценарии. Реальная power-loss durability требует отдельной
проверки целевой filesystem/storage среды. Таблица выше — план, не execution evidence.

## Следующие рабочие результаты

После R-D01/R-D02: заморозить storage layout, ACK/error state machine, request-ID
semantics, lock mechanism, limits и retention. Затем реализовать отдельный host
adapter, fault-injection tests, fresh-process recovery и CI receipts. До этого
изменения runtime APIs и новый durable PASS не объявляются.

Temporal availability (`available_at`, `decision_at`, late evidence eligibility)
остаётся отдельным обязательным контрактом до заявления о причинно корректном
historical trading replay. Snapshot/recovery его не заменяют.
