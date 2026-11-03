# MDC Portable Identity Contract v0.1

MDC-IMPL-010, 2026-11-03. Проектное расширение MDC-SCALAR-001 поверх
commit `d33bb979c0b8b6a69126fd79544d3f43e5df9d3b`. Этот документ не меняет
предметные контракты STP-MDC и не определяет криптографическое удостоверение.

## Два явных профиля

`Pipeline::new(registry)` сохраняет `legacy-rust-hash-fnv64`.
`Pipeline::with_identity_protocol(registry, IdentityProtocol::PortableV1)`
выбирает `MDC-ID-FNV1A64-LE-v1`. Протокол Pipeline неизменяем после создания.
В одном Pipeline все admitted O/M/R/C/S/T, Evaluation, invalid artifacts и
lifecycle events получают digest одного профиля **до** вставки в хранилище.
Conformance IdentityCoherent применяет профиль этого Pipeline.

Legacy означает прежний Rust slice Hash/FNV, включая native представление длины.
Его переносимость не утверждается; producing toolchain/target остаются частью
свидетельства. Неизвестное имя/версия протокола отклоняется без fallback.
`ProfiledIdentity` сравнивает protocol и digest. Один u64 без protocol — неполный
внешний identity handle. `ObjectRef` внутри Pipeline остаётся scoped reference;
перенос ссылок между Pipeline с разными профилями не является поддерживаемым import.

## Точный hash protocol v1

Для последовательности canonical bytes `B` длиной `n` hash preimage равен:

```text
4d 44 43 2d 49 44 00 01 00 01 01 || u64_le(n) || B
└────── magic ──────┘ └v1─┘ h  c
```

Magic — ASCII `MDC-ID` и NUL (7 bytes). Version — u16 LE = 1.
Hash code 1 — FNV-1a-64; codec code 1 — зафиксированный scalar basis codec ниже.
Длина — число bytes, а не символов или элементов, ровно 8 bytes LE.
FNV offset `0xcbf29ce484222325`, prime `0x100000001b3`. Для каждого byte preimage:
`h = (h XOR byte) * prime mod 2^64`. Нулевое состояние не переинициализируется.
Header, length и body обрабатываются одним непрерывным состоянием.
Пустое B допустимо на уровне digest API, но не допускает semantic object в Pipeline.
Digest как число u64; канонический binary digest — 8 bytes LE, текст — 16 lowercase
hex digits. Rust Hash, Debug, pointer width, host endian и randomized state не входят
в алгоритм. `portable_v1_preimage()` раскрывает точный preimage.

## Scalar basis codec 1

Codec сохраняет существующие `canonical_basis()` admitted объектов. Он не является
универсальным сериализатором произвольных Rust types. У legacy `Observation<S>`
с Debug-форматом нет нового обещания portable semantic canonicalization; для
portable graph используется admission под scalar source/schema contracts.

* Префиксы `MDC-SCALAR-001/{observation,measurement,relation,composition,state,
  transition,evaluation,invalid}/v1\0` различают semantic types и body version.
  Lifecycle использует `MDC-LIFECYCLE-001/invalidation/v1\0`; Evaluation может
  содержать version-tagged lifecycle context из 008.
* Строка = u64 LE длины UTF-8 bytes + точные bytes. Нормализация Unicode не
  выполняется; различные codepoint sequences не объявляются эквивалентными.
* Scalar payload — signed i64 LE; вычисленное scalar value — signed i128 LE.
  Timestamps, horizon, interval bounds, counts и numeric IDs — u64 LE;
  contract version — u32 LE. `usize` counts сериализуются как u64, не native width.
* `VersionedId` = строка ID + version u32 LE. Domain/Frame/Dynamics contexts
  используют прежний порядок полей из `contract/encoding.rs`.
* Коллекции имеют u64 LE count и прежний порядок полей/элементов. Нормализация
  unordered sets выполняется объявленным контрактом до encoding. Ordered inputs,
  дубликаты invalid inputs и причины отказов сохраняют свою семантику.
* Bool/presence markers — один byte; семантические enum codes сохраняются.
  Enum, кодируемые через cast, теперь имеют явные неизменённые discriminants.
  TargetType O/M/R/C/S/T = 0…5; Result Pass/Fail/Unresolved = 0/1/2;
  Status Candidate/Conformant/Invalid = 0/1/2; CriterionOrder Ordered/Unordered = 0/1.
  DefinitionKind Source…Conformance = 0…10 в объявленном порядке.
* Identity digest самого объекта не добавляется в его body. Ссылки на inputs,
  версии, criteria/results, captured evidence и lineage кодируются как прежде.
  Поэтому portable downstream bytes содержат portable upstream IDs.

Typed ObjectRef codec (`canonical_reference_v1`) фиксирует tags:

| Tag | Payload |
|---|---|
| 0 | Source VersionedId |
| 1…6 | O/M/R/C/S/T: numeric ID u64 LE |
| 7 | Evaluation numeric alias u64 LE |
| 8…13 | Invalid O/M/R/C/S/T: digest u64 LE + length-prefixed full canonical key |
| 14 | EvaluationKey: digest u64 LE + length-prefixed full canonical key |

Полный layout каждого scalar body зафиксирован соответствующим canonical_basis
и invalid encoder в source manifest 010, шестью byte/hash vectors и frozen
portable graph control. Codec v1 не меняет уже существующие layouts. Изменение
порядка/значения кодов или полей требует новой codec/protocol version и новых
контрольных векторов; нельзя молча переопределять v1.

## Snapshot и миграция

`snapshot_with_protocol()` получает basis один раз и вычисляет digest по тому же
buffer. Это capture bytes, не admission и не автоматический выбор protocol из T.
Прежний `snapshot()` остаётся legacy API. Для portable object следует явно
передать `pipeline.identity_protocol()`.

`migrate_legacy_snapshot()` проверяет legacy digest захваченных bytes в текущем
legacy runtime, затем возвращает tagged portable snapshot тех же bytes. При
несовпадении digest возвращается ошибка. Исторический snapshot не изменяется.
Свидетельство другой legacy среды нужно проверять в её producing runtime;
данный helper не угадывает width/endian или алгоритм неизвестной версии Rust.
Он не заменяет embedded legacy references и не превращает старый граф в portable.

Перенос semantic graph — explicit replay тех же source/schema/operator/structure/
dynamics/conformance definitions и запросов в **новом** portable Pipeline.
Каждый downstream request получает references нового графа; таблица соответствий
old/profile/reference → new/profile/reference сохраняется отдельно по заявленным
semantic roles. Совпадение O bytes ожидаемо; downstream bytes могут измениться
из-за новых upstream IDs. Это новые artifacts, а не исправленная история.
`examples/identity_migration.rs` исполняет такой replay для контрольной цепочки;
tests сравнивают значения/результаты всех шести уровней и сохранность old snapshots.
Общего persistent graph importer/replayer этот пакет не добавляет.

Evaluation lookup по body требует правильного protocol:
`EvaluationId::from_canonical_basis_with_protocol()`. Legacy lookup не выбирает
другой artifact по совпавшим bytes/bucket; прежние aliases остаются pinned.
Collision rules 009 действуют для обоих профилей. FNV64 не доказывает глобальную
уникальность: core guard отклоняет разные bodies с одним ID; Evaluation/invalid/
lifecycle namespaces сохраняют полный canonical key.

## Область проверки

Native matrix: Rust 1.85.1/1.98.0 × default/std-alloc/alloc на x86_64 Linux.
Дополнительно CI исполняет Rust tests и два portable process runs на i686 Linux
(32-bit LE) и s390x Linux под QEMU (64-bit BE), обе версии Rust, профиль alloc.
Compiled target probe фиксирует реальную width/endian, а comparison job требует
все десять конфигураций, чистые checkout metadata, один commit/run/source fingerprint,
одинаковые canonical bytes/IDs/lineage и frozen graph SHA-256.

На этапе подготовки 010 local native matrix PASS; hosted/cross-target evidence
публикуется отдельным receipt, привязанным к commit/run. 23-case legacy E2E и
portable graph scenario — разные проверки. Это не сертификация других schemas,
floating-point данных, OS/targets, arbitrary Rust toolchains или всех input graphs.
Reinstatement, durable recovery и historical availability остаются вне 010.
