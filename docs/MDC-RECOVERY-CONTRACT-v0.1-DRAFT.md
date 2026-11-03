# MDC-RECOVERY-001 — Snapshot + WAL recovery contract v0.1 DRAFT

2026-11-03. Статус: OWNER DIRECTION RECORDED / ENGINEERING DRAFT / NOT IMPLEMENTED.
Основание: historical replay → long-running service, snapshot MDC-IMPL-011
в main 27fbc0a и предложение владельца использовать local files + WAL с hybrid sync.
Этот документ — новый host contract; он не переопределяет исходные STP-MDC.

## Зафиксированное направление и границы решений

| ID | Направление владельца | Что ещё требуется зафиксировать |
|---|---|---|
| R-D01 | Local snapshot + append-only WAL, без embedded DB в первом профиле | Linux/local filesystem/single writer — предлагаемая первая область поддержки; lock mechanism и filesystem prerequisites |
| R-D02 | Snapshot и критические решения sync; streaming events допускают buffered batch | Typed ACK, critical barrier, batch limits и recovery suffix policy |

Одна секунда либо N событий приведены владельцем как примеры batch policy,
а не утверждённые численные SLA. Максимальная скорость или сохранность последних
streaming events этим выбором не гарантируются. Проект использует минимум зависимостей;
это не доказательство, что собственный WAL безопаснее проверенной embedded DB.

`SnapshotSession` 011 уже сохраняет recorded core history, frozen Registry,
requests/outcomes, aliases, lineage, lifecycle и audit. Его файловый пример не
добавляет durable ACK. WAL и durable host adapter пока отсутствуют.

## Термины и подтверждения

Каждому host request назначаются session ID, sequence и stable request ID.
Есть две разные границы: last accepted sequence и durable sequence D.
D — непрерывный префикс истории; durable holes запрещены.

| Receipt, предлагаемые имена | Значение |
|---|---|
| AcceptedVolatile(seq, outcome) | Запрос принят live session; storage durability не подтверждена |
| DurableThrough(D) | Все requests/outcomes/host receipts до D включительно прошли commit protocol |
| CriticalCommit(seq, outcome, D) | Критическая операция и весь её предшествующий префикс durable; D >= seq |
| StorageError / Indeterminate | Нельзя выдать durable ACK; при неопределённом commit новые записи блокируются до recovery |

Это проект API, не добавленные Rust types. Semantic Pass/Fail/Unresolved и storage
receipt ортогональны. Ошибка derivation может создать invalid artifact и audit;
такой request тоже входит в WAL. Outcome не заменяется storage success/failure.

Buffered admission может публиковать provisional live state, но consumer должен
видеть его durability. Использование результата как окончательного торгового
решения требует explicit critical barrier. Сам по себе Pass Evaluation не делает
его durable и не означает отправку заявки. Read-only pipeline view нельзя
документировать как durable view, если в нём есть volatile suffix.

## Главный инвариант hybrid режима

Критический barrier сохраняет ВЕСЬ предшествующий WAL prefix, включая streaming
inputs, invalid artifacts и audit, от которых решение зависит. Запись только
Evaluation или decision marker при несохранённых inputs запрещена. Сортировка
или повторная оценка истории против финального графа вместо ordered replay запрещена.

Предлагаемая safety policy: explicit commit_decision barrier перед внешним
использованием Evaluation; lifecycle invalidation и изменения host control state
также требуют strict barrier. Классификация критических действий и exact API
замораживаются перед реализацией. Buffered означает возможную потерю suffix после D.
Прежнее требование exact restore теперь относится к committed prefix; обещание
сохранить любой когда-либо наблюдавшийся volatile result не вводится.

## Компоненты и формат

Core остаётся std/no_std+alloc без filesystem, clock и network side effects.
Host adapter отвечает за файлы, writer exclusion, flush scheduling, commit state,
request deduplication и storage errors. Первый профиль предлагается ограничить Linux,
local filesystem и одним writer. Remote storage, NFS и multi-writer не включаются
по совпадению интерфейса File. Lock API должен учитывать MSRV 1.85.1.

Storage container должен иметь version, session/profile/engine identity, generation,
sequence boundaries, bounded lengths, record type и corruption detection.
WAL записывает original typed request, exact outcome и host deduplication receipt,
включая отказы и повторы. Контроль префикса и связь с snapshot обязательны.
Algorithm checksum, exact byte layout, commit marker и integrity chain пока не
зафиксированы. Semantic u64 digest не заменяет storage integrity; MAC не заявлен.

Предпочтительно переиспользовать fixed-width codec 011 и явно версионировать WAL
framing. Сейчас operation codec private: для отдельного adapter нужен узкий
проверяемый API, а не доступ к private Pipeline fields. bincode является внешней
зависимостью и сам по себе не доказывает переносимость, стабильность или максимум
скорости. Автоматическая сериализация Rust layout и новый dependency этим draft
не принимаются; решение по codec/dependencies фиксируется отдельным изменением.

Snapshot 011 содержит всю recorded history; физическое удаление WAL prefix не
означает compaction истории внутри snapshot. Pruning менял бы отдельный контракт.

## Предлагаемый порядок WAL commit

1. Получить исключительную writer lock до изменения store. Explicit create и open
   различаются; повреждённый store нельзя молча заменить пустой сессией.
2. Проверить bounded request и создать candidate/outcome. Append complete framed
   request/outcome/receipt в WAL по sequence; обработать partial write и error.
3. Для buffered вызова разрешён только AcceptedVolatile. Pending bytes и задержка
   ограничены host policy; превышение budget создаёт backpressure либо error.
4. Для batch/critical barrier завершить commit protocol непрерывного префикса,
   выполнить WAL sync и только после его успеха продвинуть D и выдать durable ACK.
   Exact порядок marker/write/sync и проверка crash points фиксируются в wire contract.
5. После неоднозначной I/O ошибки не выдавать fictitious success/rollback. Перейти
   в recovery-required state; запретить новые writes и внешнюю публикацию решения.

Для нового WAL segment недостаточно sync его содержимого: создание/rename и
соответствующие directory entries должны быть синхронизированы по контракту.
Ни таймер, ни достижение N записей не доказывают sync до фактического успеха.
При остановленном scheduler flush может задержаться; SLO проверяется измерениями.

## Snapshot publication и ротация

Checkpoint фиксирует состояние на durable D. Если live session имеет suffix > D,
нельзя экспортировать её целиком и назвать результат checkpoint D: нужно выбрать
согласованную границу (например, сначала durable flush всей live history), либо
получить изолированный candidate на D.

1. Создать complete snapshot + host receipt/cursor state на одной durable границе.
2. Записать temporary file в том же filesystem, проверить bytes и выполнить file sync.
3. Publish через atomic rename; выполнить directory sync. Согласовать manifest,
   generation pointer и новый WAL segment так, чтобы любой crash оставлял допустимый
   snapshot/WAL pair. Exact publication state machine обязателен до удаления файлов.
4. Только после durable publication удалить покрытые WAL segments; cleanup также
   должен сохранять recovery invariant. Mtime и orphan temp не выбирают committed state.

Atomic rename не равен durability. WAL rotation не может уничтожить единственное
свидетельство durable prefix. Нельзя отключать integrity checks как fallback.

## Recovery и потерянный ответ

Recovery выбирает проверенный snapshot и matching WAL history по version/session/
sequence, проверяет integrity и восстанавливает новый candidate до последнего
доказуемого committed prefix. Каждый core outcome и итоговое состояние должны
проверяться средствами 011. Несовпадение не чинится пересчётом against current graph.

Requests после D могут быть потеряны или отброшены как uncommitted; они не считаются
durable только потому, что bytes остались после process crash. Полный commit до
недоставленного ACK может быть восстановлен. Caller узнаёт outcome по stable host
request ID перед повтором. Тот же ID с иным request отклоняется; повтор committed
request не создаёт новый artifact/audit/event. Invalidation request_id ядра не
заменяет общий host deduplication protocol.

Torn tail можно отбросить только когда формат доказывает, что он uncommitted.
Corruption committed range, conflicting history или неразличимая неоднозначность
требуют hard error и диагностики, без silent reset/fallback. Конкретная схема
commit markers должна различать эти случаи; она ещё не реализована.

External input cursor должен быть согласован с durable boundary для replay.
Это не exactly-once delivery бирже/брокеру и не atomic transaction с торговой
позицией: внешний state и side effects требуют собственного контракта.

## Границы fsync

sync_all запрашивает у OS синхронизацию данных/metadata. Возврат успеха не является
универсальным доказательством сохранения при любой потере питания: влияют filesystem,
mount settings, драйвер/controller, volatile device cache и соблюдение flush накопителем.
Гарантия требует заявленных storage prerequisites и их проверки. Software fault
injection и kill процесса не равны испытанию реальной power loss.

## Acceptance scenarios для будущего кода

| ID | Проверяемое наблюдение |
|---|---|
| R01 | Snapshot + WAL → другой процесс: exact committed state и continuation |
| R02 | Buffered suffix после D: потеря допустима и явно наблюдаема; durable prefix сохранён |
| R03 | Critical barrier сохраняет все предыдущие inputs и semantic failures, а не только решение |
| R04 | Успешный durable ACK → restart: весь подтверждённый prefix существует |
| R05 | Commit состоялся, ответ потерян: retry ID возвращает outcome без повторного эффекта |
| R06 | Partial write/sync failure: нет durable ACK; indeterminate state блокирует новые writes |
| R07 | Kill на write/marker/sync/rename/rotation/cleanup boundaries: допустим только документированный prefix |
| R08 | Committed corruption/unknown version/foreign WAL: fail closed; torn uncommitted tail — только по доказуемому правилу |
| R09 | Второй writer отклонён; stale-lock recovery соответствует OS contract |
| R10 | Lifecycle, Evaluation aliases, captured missing inputs и audit multiplicity сохраняются |
| R11 | Batch/backpressure/resource limits: error не подменяет semantic result и не повреждает D |
| R12 | Snapshot publication с volatile suffix, WAL rotation и source cursor не теряют committed inputs |

Это acceptance plan, не execution evidence. Throughput, fsync latency, размер WAL
и restore time должны быть измерены; low dependency count не заменяет эти проверки.

## Порядок реализации

1. Заморозить WAL framing/version/integrity, typed receipts, D/commit state machine,
   request-ID conflict rules, lock и budgets; проверить их на crash-point модели.
2. Реализовать host storage adapter и необходимый узкий API к 011. Сначала strict
   prefix commit/recovery с fault injection, затем buffered batch с теми же invariants.
3. Добавить checkpoint/rotation, fresh-process tests и CI receipts R01…R12.
4. Измерить throughput/latency/restore budgets перед подключением торгового consumer.

Temporal availability available_at/decision_at остаётся отдельным обязательным
контрактом до заявления о причинно корректном historical trading replay.
