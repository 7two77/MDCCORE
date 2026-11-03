# MDC-IMPL-006 — Invalid semantic artifacts и подготовка E2E

Дата: 2026-11-02. Статус: **реализован и проверен в пределах MDC-SCALAR-001**.
Нормативное основание — предоставленный STP-MDC v0.1 DRAFT / NORMATIVE,
MDC-01…06 invalidity/lineage requirements и CB. Полный conformance gate ещё
**NOT_EXECUTED / BLOCKED**; результаты component tests не меняют этот статус.

## Типизированное представление невалидного результата

Отказ admission/construction теперь сохраняет `InvalidSemanticObject` отдельно
от admitted store. `InvalidBody` содержит один из шести partial semantic bodies:

| Вид | Сохранённые semantic fields |
|---|---|
| Observation | Source/version, доступные Source/Schema definitions, timestamp, payload, acquisition context |
| Measurement | Operator/version/definition, ordered inputs, доступные unit/context, вычисленный value либо `None` |
| Relation | Contract/definition/context, endpoints, вычисленный attribute либо `None` |
| Composition | Contract/rule, members, relations, доступные результаты всех evaluated constraints |
| State | Derivation contract/definition, Composition reference, horizon, доступный interval и criterion results/witnesses |
| Transition | Contract/definition, ordered source/target State refs, horizon, condition results/witnesses |

Это semantic body с доступными definitions и результатами, а не переименование
пары `{request,error}`. Невычисленный результат остаётся `None`: fallback zero,
synthetic member или придуманный criterion result не создаются. Если вычисление
успело завершиться, но insertion отклонён из-за identity collision, candidate
сохраняет фактически полученный value/results и witnesses для invalid body.

Публичные getters дают `body()`, `kind()`, `error()`, `contract()`, `status()`,
`captured_inputs()`, `unresolved_references()` и `unresolved_definitions()`.
Status всегда `Invalid`. `Pipeline::resolve_invalid()` и `invalid_count()`
позволяют прочитать сохранённые объекты; unchecked public constructor и mutable
доступ отсутствуют.

Прежний `RejectedAttempt` остаётся точной audit receipt исходного запроса и
ошибки. Его `invalid_reference()` связывает receipt с semantic object. Повторные
эквивалентные failures могут дедуплицировать объект, сохраняя каждую receipt.

## Identity и canonicalization

Шесть namespaces `ObjectRef::InvalidObservation` … `InvalidTransition` отделены
от admitted refs. Невалидный numeric ID не превращается в пригодный вход.
`InvalidId` содержит legacy `u64` digest **и полный canonical key**. Разные keys
различаются даже при совпадении digest; исходный failure не теряется и не
заменяется новой retention error. Использовать один `digest()` как полный invalid
reference нельзя. Теги ранее существовавших references `0…7` сохранены; новые
invalid variants получили отдельные теги. Внешние exhaustive matches требуют
обновления.

Basis содержит typed body, exact error, creation-time input snapshots,
missing object refs, missing versioned definitions и status. Доступные results
и definitions кодируются явно, без Debug-serialization диагностического текста.
Для известного unordered Composition сортируются members и relation set;
дубликаты сохраняются, поскольку их multiplicity может быть причиной отказа.
Known symmetric Relation нормализует endpoints; directed/Proportion ordering и
неизвестная rule semantics не переопределяются. Audit receipt сохраняет исходный
порядок запроса. Error также является частью identity: эквивалентность требует
совпадения фактической failure diagnostic, а не только множества inputs.

Full keys и snapshots увеличивают память; вложенные invalid references могут
многократно повторять предыдущие canonical keys. Компактное persisted storage
и ограничения размера не реализованы. Это осознанная стоимость lossless collision
disambiguation, не обещание глобальной уникальности `u64`. Прежние ограничения
native Rust Hash/межплатформенной переносимости остаются прежними.

## Captured lineage и admissibility

При failure сохраняются snapshots реально разрешённых inputs и их closure.
Missing semantic object refs и missing source/schema/operator/rule definitions
отмечаются раздельно. У отсутствующего target не появляется synthetic object.

Для шести новых invalid variants `Pipeline::lineage_report()` возвращает captured
creation-time resolution. Позднее admission ранее отсутствовавшего input не
переписывает body, basis, error или старые missing lists. Новая попытка в другом
resolved context может получить другой invalid object; старый остаётся доступен.
При ссылке на предыдущий invalid artifact наследуется его captured closure.

Для admitted objects и Evaluation этот API показывает текущую resolution closure.
В частности, `lineage_report()` старого invalid Evaluation не является историческим
as-of replay: текущий target может разрешиться позднее, но Evaluation diagnostics,
результат и captured target basis не изменяются задним числом.

`LineageReport::is_complete()` означает полноту object/definition closure, не
admissibility, truth или source authenticity. Complete invalid Relation может
иметь trace до Sources и всё равно не приниматься numeric/Composition/State/
Transition APIs. Strict `trace()` сохраняет ошибку для неполного graph и не
превращает missing refs в реальные объекты. Проверки usability остаются отдельными.

При известном conformance contract существующий invalid target распознаётся как
`TargetInvalid` с semantic `Fail`, а не missing-target `Unresolved`. Его semantic
snapshot/reference сохраняются в invalid Evaluation. Unknown contract по-прежнему
не получает выдуманный semantic result.

Полные collision-discriminated keys применяются к шести новым invalid namespaces.
Хранилище Evaluation сохраняет политику MDC-IMPL-005: его собственная коллизия
обычного `u64` завершает `evaluate()` ошибкой, без перезаписи существующего
artifact. Отдельный collision-retention namespace для Evaluation не добавлен.

## Граница реализованного invalidity

В bounded profile реализованы representation и retention результатов **failed
construction** для всех шести видов. Существующие admitted maps/snapshots не
перезаписываются, даже при конструировании invalid descendant.
Это не операция последующей invalidation уже admitted object и не автоматическая
invalidity propagation по ранее существовавшим descendants. Такие операции пока
не реализованы; CB Fail по-прежнему не изменяет target.

Declared source trust, context/version descriptors и source-timestamp horizon
ограничения прежних пакетов сохраняются. Arrival time, `available_at`, `decision_at`
и общий historical as-of store не добавлены.

## Проверки и evidence

Rust 1.85.1, `x86_64-unknown-linux-gnu`. **118/118 тестов** проходят в каждой из
конфигураций default, `std,alloc` и `no_std + alloc`; Format, Clippy обоих профилей
и пример `invalid_artifacts` проходят. Без features получается ожидаемая явная
диагностика обязательного `alloc`. Hosted tests не доказывают allocator/linker
configuration произвольной bare-metal-платформы.

Сохранён 101 прежний тест; добавлены 17: codec — 2, invalid model — 4,
forced digest collision — 1, computed-candidate collision retention — 1,
integration — 9. Проверяются все шесть bodies, actual Pass/Fail/Unresolved,
missing refs/definitions, dedup, late admission, непригодность invalid inputs,
Evaluation TargetInvalid и сохранность admitted graph.

- [Проверки, команды и stdout/stderr](evidence/impl-006.json).
- [Compatibility probe](evidence/impl-006-compatibility.json): 16 прежних
  O/M/R/C/S/T/E объектов совпали по bytes/IDs/trace на контрольной цепочке.
- [Source manifest](evidence/impl-006-source-manifest.json): границы изменений.

Вложенные 18 файлов, Cargo manifest/lockfiles и исторические отчёты 001…005
сохранены; dependencies не добавлены. Пользовательский Codespaces не изменялся.
Патч `mdc-core-impl-006.patch` предназначен для MDC-IMPL-005,
`mdc-core-impl-006-from-original.patch` — для первоначального архива. Они
альтернативны; применение и побайтовая реконструкция проверяются при упаковке.

```bash
cargo test --locked --offline
cargo test --locked --offline --no-default-features --features alloc
cargo clippy --locked --offline --all-targets -- -D warnings
cargo run --locked --offline --example invalid_artifacts
python3 scripts/check_gate_readiness.py
```

## Подготовленный E2E gate

[Readiness matrix](MDC-E2E-GATE-READINESS-v0.1.md) и
[machine-readable manifest](../tests/fixtures/mdc_e2e_gate_v1.json) перечисляют
все **12 E2E-A01…A12 и 11 N01…N11**. У каждой процедуры harness status
`NOT_EXECUTED`; общий acceptance — `BLOCKED`. Component coverage служит API map,
не автоматическим зачётом gate.

`check_gate_readiness.py` проверяет preparation metadata и ожидаемо завершает
работу с **exit 2**: metadata корректны, gate не исполнен. Это не semantic Fail
и не gate PASS. Единый runner, executable negative bindings и registered
technical projection/mapping пока отсутствуют. Старые два E2E-теста остаются smoke.

Следующий пакет — MDC-IMPL-007: фактический runner, technical projection adapter
и execution evidence по обязательным assertions. Fixture должен вызывать ядро,
а не восполнять calculus собственной реализацией. Рыночные projection profiles,
VAIXLNS-Builder и STSA-MC в этот этап не входят.
