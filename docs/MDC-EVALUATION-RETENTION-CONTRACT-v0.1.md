# MDC-EVALUATION-RETENTION-001/1 — Contract decision v0.1

База: commit `70f37fc2b510f9436f1208ee378017dec444537e`, MDC-IMPL-008.
Дата: 2026-11-03. Область: хранение Evaluation и разрешение её references
при совпадении u64 digest. Правила conformance criteria, aggregate results,
canonical Evaluation body и native identity function сохраняются.
Это проектное расширение ограниченного MDC-SCALAR-001, не portable identity protocol.

## Канонический ключ

`EvaluationId` содержит digest и все canonical bytes конкретной Evaluation.
Его Eq/Ord/lookup учитывают оба компонента. Разные bodies с одним digest не
равны и не перезаписывают друг друга. У Evaluation есть `key()`; canonical
reference имеет вид `ObjectRef::EvaluationKey(EvaluationId)`.

Полный key является authoritative ключом внутреннего BTreeMap для **всех**
Evaluation, включая успешные, failed, unresolved и diagnostics-only результаты.
`evaluation_count()` считает artifacts, а не число aliases.
`evaluations_with_digest()` возвращает все найденные canonical references;
выбор произвольного «первого подходящего» body по одному digest не допускается.
Незнакомый полный key не подменяется известным членом того же bucket.

`EvaluationId::from_canonical_basis()` создаёт только lookup reference. Он не
допускает новый artifact и не подтверждает, что произвольные bytes являются
Evaluation. Resolve требует точного существующего key в Pipeline.

## Совместимость numeric alias

`evaluate(request)` сохраняет compatibility preference:

1. Если Evaluation с тем же full key уже есть, возвращается её прежний primary ref.
2. При первой insertion свободный numeric ID может быть зарезервирован как
   `ObjectRef::Evaluation(u64)`.
3. Если ID занят другим semantic object или Evaluation, новый результат
   сохраняется под EvaluationKey и возвращается именно этот reference.

Numeric reservation никогда не меняет владельца, включая после invalidation.
Старые references, provenance и snapshots продолжают разрешаться к прежнему body.
Если alias для digest не был создан, resolve `Evaluation(digest)` возвращает
None даже при существовании единственного keyed artifact в bucket.

Legacy alias — исторический reservation, а не доказательство уникальности hash.
При colliding bodies выбор владельца alias зависит от порядка legacy insertion.
Для collision-independent references предусмотрен `evaluate_keyed(request)`:
он всегда возвращает full key и при первой insertion **не создаёт numeric alias**.
Графы, построенные только через keyed API, имеют одинаковые canonical references
при перестановке вставки colliding Evaluation. Смешанный режим не переименовывает
прежний primary ref и не меняет historical lifecycle records.

Для уже существующего legacy artifact keyed API возвращает дополнительный lookup
handle к тому же artifact. Поздний вызов legacy API для keyed-first artifact не
создаёт alias задним числом. `canonical_evaluation_reference()` преобразует
любой разрешённый handle в canonical full-key reference.

## Семантические и ссылочные границы

Storage collision не является semantic Fail или Unresolved. Вычисленные result,
status, criteria, diagnostics, context и provenance сохраняются без подмены.
Коллизия Evaluation с numeric ID другого core object не меняет этот объект.
Keyed Evaluation не занимает global numeric namespace O/M/R/C/S/T.
Зарезервированные legacy aliases продолжают защищаться прежним numeric collision guard.

Новый ObjectRef codec tag — **14**, затем digest u64 LE и length-prefixed full key.
Теги 0…13 и все старые encodings остаются прежними. `EvaluationKey` — новый
public enum variant; внешние exhaustive matches должны быть обновлены.
Канонический body Evaluation не содержит storage preference: keyed и legacy
API для одинакового запроса вычисляют одинаковые bytes и digest.

`trace`, captured invalid lineage, Evaluation provenance и projection используют
точный supplied handle. Keyed Evaluation не становится допустимым input для
Measurement/Relation/Composition и не становится новым типом conformance target.

External Representation сохраняет выбранный Evaluation handle, поэтому её
внешние bytes могут различаться для alias и full key одного artifact. Source и
Evaluation snapshots в ней совпадают; calculus graph не изменяется.

## Lifecycle

Каждый artifact имеет один immutable primary handle. Lifecycle birth/current
status/replay нормализуют разрешённые aliases к нему. Invalidation одной
Evaluation не распространяется на другую лишь из-за одинакового digest.
Projection не обходит lifecycle через альтернативную форму reference.
Старые bodies, включая Pass/Conformant, сохраняются для аудита.

Evaluation не является корневым target invalidation: это ограничение 008
сохранено. Она участвует в propagation по lineage её target. Новая Evaluation
после invalidation содержит прежний lifecycle context и получает свой full key.

## Проверка коллизий

Natural FNV64 collision не ищется probabilistically. Unit tests имеют private
`cfg(test)` digest override на уровне Pipeline Evaluation identity и private
setter artifact digest. Сами criteria, actual result, canonical body, storage,
trace, lifecycle и projection выполняются production API.
В обычной сборке нет public настройки, feature или environment variable для
подмены digest. Hosted examples не используют injection и не объявляют, что
обнаружили natural collision.

Проверяются различие полных keys при одном digest, pinned aliases, все semantic
результаты, столкновение с core object, точный lookup, неправильные target,
InvalidInputType, propagation, historical views и projection. Отдельные public
integration scenarios используют normal identity function для всех шести target types.

## Ограничения

Полные keys копируются в references и canonical encodings: расход памяти и
длина вложенных references выше. Bucket lookup сканирует map; production-scale
характеристики, durable import и crash recovery не подтверждены.
Legacy digest-only references не дают collision-independent naming и не
превращаются в полный historical availability protocol.

Native Rust Hash/FNV остаётся прежним. Полный key предотвращает локальную
перезапись разных bodies, но не доказывает global uniqueness u64 или переносимость
между архитектурами. Это остаётся областью MDC-IMPL-010.
