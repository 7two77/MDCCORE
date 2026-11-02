# MDC-IMPL-002 — Source → Observation → Measurement

Дата: 2026-11-02. Статус: **реализован и проверен в пределах MDC-SCALAR-001**.

Нормативное основание — предоставленный `One MDC.md`, STP-MDC v0.1,
**DRAFT / NORMATIVE**, MDC-01, MDC-02 и общие требования Conformance Boundary.
Этот пакет реализует ограниченный исполнимый профиль `MDC-SCALAR-001` и не
устанавливает полное соответствие MDC-01…06.

## Исходная точка

Основа — рабочая копия после MDC-IMPL-001. Контрольная неизменённая копия
подготовлена из архива MDC-IMPL-001 отдельно от изменяемого проекта. Пакет не
создаёт Git-репозиторий, не восстанавливает отсутствующий Git index и не меняет
пользовательский Codespaces. Вложенный каталог `mdc_core/` сохраняется как
историческая копия; его `Measurement` не переносится автоматически.

Сведения о файлах и границах изменений записываются в
[source manifest](evidence/impl-002-source-manifest.json). Исторические результаты
MDC-IMPL-001 остаются в [предыдущем отчёте](MDC-IMPL-001.md).

## Исполнимый профиль

Все контракты регистрируются явно через `Registry`. Ключ — `VersionedId` с
непустой строкой ID и положительной версией `u32`. При разрешении требуется
точное совпадение пары ID/version; другой зарегистрированный номер версии не
используется как fallback. Повторная регистрация существующего ключа отклоняется
и не заменяет контракт. После передачи registry в `Pipeline` его нельзя изменять
через публичный API pipeline.

| Контракт / объект | Определённое поведение |
|---|---|
| Schema | `SignedI64Le`: ровно 8 bytes, signed i64, little-endian |
| Source | Точная schema/version, непустой domain и unit, versioned time basis |
| Observation request | Зарегистрированный source, явный timestamp, payload, непустой acquisition context |
| `Scalar` operator | Один существующий Observation или Measurement; возвращает его scalar value |
| `Difference` operator | Два упорядоченных входа; вычисляет `left - right` через checked i128 |
| Compatibility | Одинаковые domain, time basis/version и unit; неявных преобразований единиц нет |

Тип `i128` для вычислений позволяет представлять разность исходных i64 без
переполнения. Для цепочки prior Measurements каждое дальнейшее вычитание также
проверяется; overflow возвращает ошибку, а не wrapping value.

Реестр подтверждает наличие и согласованность объявленных source descriptors.
Он не аутентифицирует внешнюю биржу, файл или feed и не доказывает истинность
payload. Доверие к поступившим исходным данным остаётся обязанностью вызывающей
стороны под её source contract.

## Admission и неизменность

Старый `Observation<S>` с публичными полями и `Observation::new()` сохранён для
совместимости. Это raw-объект; вычисление его legacy identity не делает его
принятым semantic object.

`Pipeline::admit()` разрешает source/schema, проверяет timestamp, acquisition
context и payload, затем создаёт `AdmittedObservation`. Его поля закрыты; наружу
возвращается typed `ObjectRef`, а чтение сохранённых объектов доступно только
через shared references. Внешний код не может самостоятельно создать или
изменить admitted object и передать raw Observation в measurement operation.

Принятые Observation и Measurement имеют status `Candidate`. Успешное
конструирование не объявляет их `Conformant` относительно ещё не реализованного
CB evaluator и не приравнивает результат к truth.

## Identity и canonical bytes

Legacy Observation сохраняет bytes и identity protocol MDC-IMPL-001. Новые
admitted Observation и Measurement используют отдельные type/version prefixes
`MDC-SCALAR-001/.../v1` и length-prefixed binary encoding. Числа кодируются
little-endian; references содержат type tags, ID и version, где применимо.

В canonical basis admitted Observation входят source/schema/version, timestamp,
domain, time basis/version, unit, payload и **acquisition_context**. Последний
передаётся источником как детерминированная координата acquisition. Вызывающая
сторона должна воспроизводить её при повторе входа; runtime clock, случайный UUID
или счётчик для её генерации профиль не вводит.

В basis Measurement входят operator/version, ordered inputs, value, unit,
context и derivation provenance. Перестановка inputs у `Difference` меняет
semantic content и обычно значение; она не нормализуется как unordered set.

Хеширование по-прежнему выполняется существующим `IdentityAdapter` через native
Rust `Hash` и FNV-подобный `u64` hasher. Версионированная структура bytes не
превращает этот hash protocol в переносимую гарантию между Rust toolchains и
платформами и не обеспечивает формальную уникальность.

Перед вставкой pipeline сравнивает basis с уже сохранённым content под тем же
числовым ID. Идентичное содержимое дедуплицируется; разное содержимое с тем же ID
отклоняется как `IdentityCollision`, без перезаписи upstream. Это локальная
защита хранилища, а не доказательство отсутствия коллизий во всех возможных данных.

## References, provenance и ошибки

`ObjectRef` различает Source, Observation и Measurement. Операция measure
разрешает только реально сохранённые admitted objects. Source reference не
является непосредственно измеряемым входом. Неизвестные references и версии не
создают fallback objects.

Measurement сохраняет explicit operator contract и ordered inputs в
`DerivationProvenance`. `Pipeline::trace()` возвращает детерминированно
отсортированное recursive closure, включая start object:

```text
Measurement → prior Measurements / Observations → registered Sources
```

Порядок trace closure не заменяет semantic ordering исходных measurement inputs.

Отклонённый admission или measurement сохраняется в `RejectedAttempt`: исходный
запрос, точная причина и status `Invalid` остаются доступны для аудита.
Неуспешная операция не помещает пригодный к дальнейшему вычислению объект в
semantic store. Это **audit record отклонённой попытки**, а не полноценный
невалидный Measurement semantic object. Поэтому MDC-02.8 в части invalid object
representation/retention остаётся `PARTIAL`.

## Общая агрегация

`aggregate_results()` реализует `FAIL > UNRESOLVED > PASS`, сохраняя различие
между явным `Unresolved` и `Fail`. Пустой набор criteria возвращает
`AggregationError::EmptyCriteria`; это явное решение текущего исполнимого профиля,
не нормативное утверждение о семантике произвольного пустого contract.

Функция не придумывает отсутствующие criteria и не является CB evaluator.
Resolution mandatory criteria, их связь с конкретным Conformance Contract,
полный evaluation lifecycle и Evaluation identity/context/version требуют
следующего этапа.

## Матрица покрытия

| Норма / механизм | Статус текущей реализации |
|---|---|
| MDC-01 source/schema/context admission | Реализовано для указанного scalar profile; произвольные source schemas не поддержаны |
| MDC-01 identity/provenance/immutability | Реализовано для `AdmittedObservation`; legacy raw Observation не получает этих гарантий |
| MDC-02 explicit operator/version, inputs, value, units | Реализовано для `Scalar` и `Difference` |
| MDC-02 recursive lineage и ordered canonical inputs | Реализовано для admitted Observation и prior Measurements |
| MDC-02.8 invalidity | PARTIAL: rejected requests сохранены; полноценный invalid Measurement отсутствует |
| Shared tri-state aggregation | Реализовано; пустой набор отвергается |
| Внешняя source authenticity, произвольные schemas/operators и unit conversion | Не реализованы |
| MDC-03…06 construction и полный CB evaluator | Не реализованы |
| MDC-E2E-CONFORMANCE-001 | NOT EXECUTED / BLOCKED |

## Проверки и воспроизводимость

Результаты текущего пакета записаны в
[машиночитаемый отчёт](evidence/impl-002.json): точные команды, stdout/stderr,
exit codes и число прошедших тестов. Среда — Rust 1.85.1,
`x86_64-unknown-linux-gnu`; сборки размещены отдельно в `/tmp`.

```bash
cargo test --locked --offline
cargo test --locked --offline --features std,alloc
cargo test --locked --offline --no-default-features --features alloc
cargo check --locked --offline --no-default-features --features alloc
cargo fmt --all -- --check
cargo clippy --locked --offline --all-targets -- -D warnings
cargo clippy --locked --offline --no-default-features --features alloc --all-targets -- -D warnings
cargo run --locked --offline --example source_measurement
```

Без features ожидается преднамеренный отказ об обязательном `alloc`. Hosted
`no_std + alloc` проверки не подтверждают allocator и linker configuration
произвольной bare-metal-платформы.

| Группа проверок | Результат |
|---|---|
| Default / явные std,alloc / no_std+alloc tests | PASS, 37/37 в каждой конфигурации |
| Library check no_std+alloc | PASS |
| Неподдерживаемый профиль без features | Ожидаемый exit 101 с явной диагностикой alloc |
| Format и Clippy обоих профилей | PASS |
| Исполнимый Source → Observation → Measurement example | PASS в default и no_std+alloc: value=12, unit=count, lineage_nodes=4 |
| Неизменность вложенного пакета и lockfiles | MATCH, побайтовая сверка с MDC-IMPL-001 |
| Полный MDC-E2E-CONFORMANCE-001 | NOT EXECUTED / BLOCKED |

Тестов текущего пакета — 25 новых: 11 unit-проверок и 14 integration-проверок
через публичные API. Сохранены 12 тестов предыдущего пакета. Проверены реальная
цепочка вычислений с prior Measurements, recursive lineage, сохранение upstream
snapshots после downstream operation, эквивалентность графов при другом порядке
загрузки, точные причины отказа и сохранность object maps, версии контрактов,
совместимость units/domain/time basis и checked overflow. Старые два E2E-теста
продолжают считаться smoke-проверками.

Патч `mdc-core-impl-002.patch` предназначен для состояния MDC-IMPL-001.
Дополнительно подготовлен `mdc-core-impl-002-from-original.patch` для исходного
пользовательского архива до MDC-IMPL-001. Патчи являются альтернативами, их не
нужно применять последовательно. Для обоих проверено применение к соответствующей
базе и побайтовое совпадение полученной копии с подготовленным проектом.

Следующий функциональный этап — Relation и Composition поверх admitted object
graph с explicit contracts и referential closure. Он должен вызывать эти API,
сохраняя границу между implementation и test harness. Domain profiles Wave,
Trend, Signal, VAIXLNS-Builder и STSA-MC в этот пакет не входят.
