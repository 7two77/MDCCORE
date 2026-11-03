# MDC-OPS-002 — Следующий этап: решения и сценарии приёмки v0.1

Дата: 2026-11-03. Статус: CORE CONSUMER DECISIONS CONFIRMED / 011 CONTRACT IN PROGRESS.
База: main `6177277f12bc7a67861dc365e96340409bed8d34`.
Последний функциональный пакет: MDC-IMPL-010. Этот документ инициирует подготовку
следующего этапа, не вводит новые semantic contracts и не назначает продуктовые
требования вместо владельца проекта.

## Подтверждённая исходная точка

PR #3 объединён merge commit с сохранением обеих историй.
[Post-merge CI](https://github.com/7two77/MDCCORE/actions/runs/41626673525) —
12/12 jobs PASS: шесть native configurations с 176 Rust tests и 21 Python checks
каждая, четыре cross-target configurations с семью protocol tests каждая,
portable comparison и history guard.
[Checkpoint](evidence/post-merge-main-6177277.json) фиксирует фактический commit,
parents/tree, jobs и результаты history/comparison.

В Codespaces пользователь подтвердил HEAD = origin/main = 6177277, clean worktree,
upstream origin/main, history/evidence checks PASS. Это отдельное пользовательское
execution evidence; повторный локальный запуск через агент не заявляется.

Поддержан ограниченный admitted scalar profile: O/M/R/C/S/T, Evaluation,
typed invalid artifacts, monotonic invalidation и opt-in portable identity.
Pipeline хранит граф в памяти; Registry заморожен при создании Pipeline.
Lifecycle journal не содержит всей истории создания graph/definitions.
Portable digest protocol не является форматом хранения или импорта графа.
Package version в Cargo.toml — 0.1.0; запись о GitHub release отсутствовала
при проверке. Публикация crate/tag/release не входит в OPS-002.

## Рекомендуемый порядок событий

1. Выбрать первого потребителя и один полезный сквозной сценарий: входные данные,
   ожидаемые State/Transition/Evaluation, способ запуска и ограничения.
2. Зафиксировать сценарий как executable acceptance contract, используя public
   core API. Consumer fixture не реализует собственные derivation algorithms.
3. По требованиям потребителя выбрать функциональную ветку:
   persistent graph, temporal availability или продолжение in-memory интеграции.
4. Для выбранного механизма сначала заморозить его versioned contract, затем
   добавить implementation, positive/negative tests и обновить traceability.
5. На границе внешнего import/длительного исполнения задать resource budgets
   и измерить контрольные graph sizes. Пороговые значения пока не подтверждены.
6. После приёмки первым потребителем подготовить release candidate:
   API compatibility, поддерживаемые profiles/MSRV/targets, changelog, licensing
   и delivery channel. Номер версии и публикация — отдельные решения.

Это условный граф зависимостей. Temporal profile может развиваться без durable
backend, если перезапуск не требуется. Snapshot обычно полезен раньше crash
recovery; оба механизма не являются предусловием обычной in-memory библиотеки.

| Рабочий пакет | Когда нужен | Что необходимо решить | Критерий завершения |
|---|---|---|---|
| MDC-CLIENT-001 — первый потребитель | Следующий общий шаг | Конкретные input/output и полезная операция, среда запуска | Отдельный consumer использует public API и воспроизводимо проходит свой acceptance scenario |
| MDC-IMPL-011 — exact snapshot roundtrip (предлагаемый номер) | Если нужен перенос/перезапуск графа | Полный состав snapshot, версии формата, profiles, валидация и budgets | Export/import сохраняет bytes, IDs, aliases, lineage, lifecycle views и audit без пересчёта прошлых результатов |
| MDC-RECOVERY-001 — durable adapter/journal | Если нужна сохранность при аварии | Commit boundary, порядок записей, single/multi-writer, platform/backend, recovery policy | Контролируемая прерванная запись восстанавливает документированное committed состояние; повреждения диагностируются |
| MDC-TEMP-001 — availability profile | Если требуется «что было известно на момент t» | Event time, available time, decision cutoff, clock basis, late corrections/invalidation | Будущая доступность не влияет на ранний decision; поздняя информация создаёт новый результат |
| MDC-ROBUST-001 — resource envelope | Перед внешним import/эксплуатационной нагрузкой | Объём, глубина lineage/full keys, throughput, error policy | Границы измерены; budget exhaustion диагностируется без изменения принятого графа |
| MDC-RC-001 — release candidate | После приёмки потребителем | API stability, версия, delivery channel, license при публичной поставке | Проверяемый release manifest и совместимость; публикация выполняется отдельным действием |

## Два решения владельца, определяющие ближайшую реализацию

DR-N01 — первый практический сценарий: CONFIRMED, 2026-11-03.
Владелец выбрал исторический replay для исследования закономерностей с дальнейшей
эволюцией в длительно работающий сервис. Торговый adapter является отдельным
потребителем core; его domain contracts и temporal availability фиксируются отдельно.

DR-N02 — восстановление после перезапуска: CONFIRMED, 2026-11-03.
Требуется точное восстановление принятого графа, Evaluation и lifecycle/audit history.
Следующий функциональный пакет — MDC-IMPL-011. Crash-safe commit/storage — следующий
отдельный contract после snapshot roundtrip; успешный in-memory replay его не доказывает.

Уточнения после этих двух решений:
- Для replay: требуется ли causal cutoff отдельно от timestamp наблюдения?
- Для сервиса: кто вправе admit/invalidate и сколько writers работает с графом?
- Каков ожидаемый объём/глубина графа и скорость поступления данных?
- Какие данные/результаты должен читать первый consumer?
- Поставка внутренняя, через Git tag/archive или публичный crate?
- Для durable хранения: какая платформа/backend и какой уровень потери данных допустим?

## Инженерные решения, для которых подготовим проектный контракт

DR-N03 — identity compatibility. Сохранить одобренный legacy default.
Новый consumer может явно выбрать PortableV1; snapshot обязан маркировать protocol.
Неизвестные versions отклоняются без fallback. Существующие v1 body layouts,
snapshots и numeric aliases не меняются задним числом.

DR-N04 — snapshot completeness, PROPOSED.
Snapshot включает полные frozen definitions Registry, identity protocol, все
O/M/R/C/S/T/E, full canonical keys и pinned aliases, typed invalid artifacts,
captured unresolved references/definitions, lifecycle events/birth epochs и audit
receipts/request-id state. Журнал invalidation и один canonical byte buffer не
заменяют такой snapshot. Import сохраняет текущий Invalid и запрет использования
отозванных объектов; старые Pass/Conformant Evaluations остаются историческими.

DR-N05 — import validation, PROPOSED.
Проверять schema/protocol versions, sizes/depth, types, keys/digests, alias ownership
и обязательную closure admitted graph до публикации нового Pipeline.
Явно captured unresolved references в invalid artifacts/Evaluations сохраняются;
они не являются повреждением только потому, что ссылка не разрешается.
Незавершённый import не меняет существующий admitted graph.
Восстановление не оценивает старые artifacts против финального/текущего графа:
позднее появившиеся inputs не должны переписать старый отказ. Для 011 выбран
отдельный recorded-session профиль: полный replay исходных операций по frozen
definitions в исходном порядке, с exact-byte проверкой каждого outcome и полного
state manifest. Он применим к сессиям, записываемым с создания, и не является
общим structural importer произвольного незаписанного Pipeline.

DR-N06 — storage boundary, PROPOSED.
Сохранить std и no_std+alloc kernel. Filesystem/network/clock и durable commit
разместить в host adapter; точное разделение codec API/adapter утверждается перед
кодом. Snapshot roundtrip подтверждает сериализацию, а не crash-safe persistence.
Single-writer — предлагаемая первая граница durable adapter, если потребитель
не требует конкурентной записи.

DR-N07 — operational errors, OPEN FOR CONTRACT DESIGN.
Различать storage/budget/decoding errors и semantic conformance results.
Resource exhaustion не должен молча превращать содержательно корректный input
в FailedCriterion. Требуются конкретные typed errors и правила сохранения audit.

DR-N08 — temporal semantics, OPEN IF REPLAY SELECTED.
Разделить timestamp события, подтверждённую доступность evidence и cutoff решения.
Определить time basis, равенство на границе, несколько acquisition receipts,
late corrections и то, как поздняя invalidation влияет на historical decision.
Нынешние horizon и lifecycle epoch не являются available_at/decision_at.
Влияние новых полей на canonical identity/context/version решается до реализации.
Это расширение MDC; чужой market contract автоматически нормативным не становится.

DR-N09 — release boundary, OPEN.
Нужны выбранный consumer, scope стабильности public API, versions/MSRV/targets,
документированные limitations и delivery channel. При публичной поставке нужно
решение владельца о license; LICENSE-файл в проверенном tree не обнаружен.
Существующая публичность GitHub repo не является решением о публикации crate.

## Предлагаемые приёмочные сценарии

S0 — общий сценарий потребителя.
Registered definitions → admitted graph → Evaluation → invalidation input →
запрет дальнейшего использования зависимостей. Проверить result/lineage и неизменность
предыдущих bytes. Полезный expected output задаёт потребитель, а не fixture.

S1 — точное восстановление, если выбран snapshot.
Построить graph, получить Pass Evaluation, выполнить invalidation и повторную
Evaluation. Export → новый процесс → import в пустой Pipeline.
Сравнить полные canonical bytes/IDs/keys/aliases, criteria/results, provenance,
lifecycle revisions/views, invalid artifacts и audit. Старый invalid snapshot
сохраняется даже после позднего admission отсутствовавшего input.
Forced-collision checks используют private cfg(test) seam, не production switch.

S2 — отказ импорта.
Unknown format/protocol, повреждённый key, неверный alias owner, отсутствующий
обязательный admitted input, чрезмерная длина/глубина и оборванный record дают
точную диагностику. Активный граф до/после отказа совпадает.
Преднамеренно unresolved captured inputs не подменяются синтетическими объектами.

S3 — causal availability, если выбран temporal profile.
Event timestamp=10, evidence available=30, decision cutoff=20:
evidence не допустим для решения на 20. Для явно заданного criterion с политикой
missing evidence → Unresolved ожидается Unresolved. Cutoff=40 может использовать
evidence; исходный результат на 20 остаётся неизменным.
Граничный случай available=cutoff и historical invalidation фиксируются отдельно.

S4 — recovery, если выбран durable adapter.
После подтверждённой операции принудительно прервать следующую запись/транзакцию.
Recovery восстанавливает заявленную committed границу и сохраняет причину отказа
или неполного tail по выбранной policy. Требуется отдельное platform/fault evidence;
из успешного in-memory replay такой вывод не следует.

## Что начато этим пакетом

Обновлён post-merge checkpoint и текущая матрица; описаны decisions/dependencies
и acceptance scenarios. Code/API, identity defaults, source fingerprint, Cargo
manifests/lockfiles и исторические receipts сохранены. Функциональный номер 011
подтверждён для snapshot согласно DR-N01/DR-N02.
Выбор владельца записывается отдельным decision record, затем начинается
contract-to-test-to-implementation работа в feature branch.
