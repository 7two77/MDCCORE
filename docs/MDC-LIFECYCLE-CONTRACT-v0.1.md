# MDC-LIFECYCLE-001/1 — Contract decision v0.1

Дата: 2026-11-02. Пакет MDC-IMPL-008; база — опубликованный commit
`a8c15c7ffbbe48993e97142f6842da044bda4162`, ветка `mdc/baseline-v0.1`.
Это исполнимое проектное решение для ограниченного профиля, не новая редакция
STP-MDC от имени автора. Нормативные основания: MDC-01.4, MDC-05.9,
MDC-06.12 (invalid objects сохраняются в lineage), immutability и Conformance Boundary.

## Решение: retained semantic snapshot и lifecycle journal

Canonical bytes и identity уже принятого O/M/R/C/S/T/E неизменны. Их `status()`
описывает сохранённый artifact. Актуальное состояние использования читается через
`Pipeline::lifecycle_view()`. После invalidation это `Invalid`, хотя сохранённый
State/Transition остаётся Candidate, а историческая Evaluation может хранить Pass/Conformant.
Это явное разделение snapshot status и current lifecycle status. Canonical status
Transition не удалён из старого протокола и не переписан задним числом.

LifecycleEvent — отдельный immutable record вне `ObjectRef` calculus graph.
Он содержит canonical `Invalid` decision, точную definition контракта, request,
revision, snapshots затронутых объектов и witness objects с предыдущими status,
captured lineage refs и unresolved object/definition refs. Предыдущий граф
сохраняется целиком. Event не является самостоятельным архивом всех объектов
и definitions, на которые ссылается lineage: для анализа остаётся нужен retained graph/registry.

У event есть собственный namespace и `LifecycleEventId`: u64 digest **и полный
canonical key**. Сравнение/lookup не используют digest как единственный ключ.
Это локальное disambiguation, не доказательство глобальной uniqueness или
межплатформенной переносимости native identity protocol.

## Предусловия invalidation

Registry заморожен при создании Pipeline. LifecycleContract регистрируется с
точной VersionedId, одним из шести target types, DynamicsContext, непустым
множеством allowed reasons и политикой `RequiredDeclaredInputs`.
Повторная регистрация версии и duplicate reasons отвергаются; порядок reasons
не семантичен и нормализуется. Контракт не может измениться в работающем графе.

Request содержит непустой caller-supplied request ID, contract ref, существующий
admitted target, reason, непустое множество разных active admitted witnesses
и expected lifecycle revision. Проверяются тип, domain/frame/calculus context
в пределах контекста, имеющегося у данного типа объекта, и временной scope реальных inputs.
Для Observation/Measurement нет собственного calculus context: его не выдумывает runtime.

Поддержаны четыре reason codes: SourceContractViolation,
ContradictoryTemporalIdentity, DerivationContractViolation, EvidenceWithdrawn.
Они являются **явными решениями вызывающего участника по зарегистрированной политике**.
Наличие reason/evidence не удостоверяет внешнюю истинность, подлинность Source
или автоматически вычисленное доказательство нарушения. Аутентификация вызывающего
участника и external source revalidation не реализованы этим in-process API.

Source definitions, Evaluation и failed-construction artifacts не являются
корневыми target этого контракта. Evaluation участвует как downstream artifact.
Для отзыва Source целиком нужна отдельная политика; здесь отзываются конкретные записи.

## Propagation и admission

`RequiredDeclaredInputs` означает: каждый input сохранённой derivation, включая
criterion/condition witness, обязателен именно для этого artifact. Invalidation
затрагивает target и все существующие O/M/R/C/S/T/E, в чьей declared lineage он
присутствует. Порядок обхода определяется typed references. Независимые ветки
не меняют состояние. Уже invalid artifacts сохраняют независимые новые причины.

Проверяется фактический provenance, а не порядок создания или общий timestamp.
Операция не выбирает alternative witness для старого State и не пересчитывает
его bytes. Новый derivation с другим набором активных inputs создаёт новый artifact.

Все production construction paths запрещают использование отозванных inputs.
Повторная construction того же ID не восстанавливает отозванный artifact.
Observation facts, используемые для новых критериев, фильтруются по текущей usable
границе. Удалённое подтверждение может дать новому State `Unresolved`.

При отказе construction сохраняется прежний typed invalid body. Новый
PipelineError::InputInvalidated содержит ref, lifecycle revision и точные event
keys; эти причины входят в canonical bytes отказа и остаются frozen после новых событий.
Общий audit tracing через `trace`/`resolve_*` продолжает видеть отозванные объекты;
полная reference closure не означает admissibility.

## Evaluation и projection

Историческая Pass/Conformant Evaluation сохраняется. Её current lifecycle view
становится Invalid, если отозван её target или зависимость target.
Она больше не допускает новую projection. Ранее полученная external Representation
сохраняется как snapshot; reverse mutation по-прежнему запрещена.

Повторная Evaluation отозванного target создаёт retained Invalid/Fail assessment
с `TargetInvalidated`, прежними target bytes и lifecycle context (epoch, admitted
epoch, current status, causal event keys). Контекст входит в canonical identity
через явно помеченное extension v1. Evaluation в разных lifecycle epochs не
сливает причины отказа. Для targets без invalidation старые O/M/R/C/S/T/E bytes
и identity protocol сохраняются без extension.

Target не получает автоматическую Conformant promotion. Выбор отдельной target
promotion/reinstatement policy не входит в этот monotonic invalidation профиль.

## Epoch, replay, idempotency и ошибки

Epoch 0 предшествует первой invalidation. Каждая успешная новая invalidation
добавляет ровно один event и увеличивает epoch. Новые objects запоминают epoch
admission. `lifecycle_view_at(ref, epoch)` воспроизводит решения журнала для
retained ref; до admission epoch возвращает NotYetAdmitted, за концом журнала — FutureRevision.

Операции создания внутри одного epoch не имеют отдельного временного порядка.
Это не полный replay истории создания графа и не `available_at`/`decision_at`.
Для точного сравнения до/после операции используются заранее сохранённые
semantic snapshots и views на фиксированном наборе refs.

Тот же normalized request ID/body возвращает прежний event даже после следующих
epochs. Другой body под тем же ID даёт RequestIdConflict. Новый ID со старым
expected revision даёт RevisionConflict. Evidence — unordered set: permutation
эквивалентна, duplicates отвергаются.

Precondition failures не меняют graph, active statuses или successful journal;
они сохраняют request/error и observed epoch в отдельном lifecycle rejection audit.
Проверка scoped lineage выполняется до append; гарантии durable transaction,
multi-process concurrency и восстановления после allocation failure не заявлены.

## Ограничения

Профиль monotonic: нет un-invalidate, автоматического ремонта, удаления истории,
Source-wide revoke или пересчёта старых descendants. Работа выполняется над
in-memory retained graph. Нет persisted journal import и crash recovery.
Проверка descendants сканирует граф; full canonical keys/snapshots требуют памяти
и могут расти при цепочках Evaluation и последующих invalidation. Это не
подтверждённая реализация для больших production graphs.

Evaluation digest collision-retention остаётся отдельным MDC-IMPL-009.
Native identity portability — MDC-IMPL-010. Временная доступность данных,
рыночные profiles, VAIXLNS-Builder и STSA-MC не добавлены.
