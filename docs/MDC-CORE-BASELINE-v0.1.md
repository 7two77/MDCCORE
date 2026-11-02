# MDC Core — Baseline v0.1

Дата фиксации: 2026-11-02. Основа: **MDC-IMPL-007**, пакет `mdc_core v0.1.0`,
ограниченный профиль **MDC-SCALAR-001**, fixture **MDC-E2E-CONFORMANCE-001/1**.
Это фиксация подготовленной реализации и evidence; Git tag или commit не создавался.

## Граница baseline

Активный пакет — корневые `Cargo.toml`, `src/`, `tests/`, `examples/`, `scripts/`.
Вложенный `mdc_core/` — сохранённая исходная копия из 18 файлов, не workspace member.
Документы MDC-IMPL-001…007 и прежние evidence сохраняются как история.
VAIXLNS-Builder и STSA-MC исключены из области baseline.

Исполняемая цепочка:

```text
registered Source → Observation → Measurement → Relation → Composition
                  → State → Transition → Evaluation → external Representation
```

Source и контракты регистрируются с точными версиями. Операции используют immutable
admitted objects; старые raw structs сами по себе не являются admitted inputs.
Evaluation имеет собственный результат и lifecycle; она не изменяет target.
Representation находится вне calculus graph, её reverse import в этом профиле запрещён.

## Идентификация и evidence

SHA-256 исполняемых исходников:
`9a907068ef266ae73d30d583a2ecb4eddb47b422078b640c60c5cf9d02569bc6`.
Manifest/lockfile, `src`, `examples`, `tests` и `scripts` входят в fingerprint;
документация, отчёты и build artifacts исключены. Поэтому новые документы baseline
не меняют fingerprint подтверждённой реализации.

Подтверждённая среда: **rustc 1.85.1, cargo 1.85.1,
x86_64-unknown-linux-gnu**, hosted std runner / Python validator.

| Проверка MDC-IMPL-007 | Фактический результат | Свидетельство |
|---|---|---|
| Rust tests: default, `std,alloc`, `no_std + alloc` | 124/124 в каждой конфигурации | [verification](evidence/impl-007.json) |
| Format, Clippy default и alloc, пять прежних примеров | PASS | [verification](evidence/impl-007.json) |
| Проверки Python-валидатора: acceptance и повреждение evidence | 17/17 | [verification](evidence/impl-007.json) |
| E2E default | 23/23 в каждом из двух отдельных процессов | [default bundle](evidence/mdc-e2e-conformance-001.json) |
| E2E alloc с hosted runner | 23/23 в каждом из двух отдельных процессов | [alloc bundle](evidence/mdc-e2e-conformance-001-alloc.json) |
| Совместимость с MDC-IMPL-006 | Совпали bytes/identity 16 объектов контрольного графа | [compatibility](evidence/impl-007-compatibility.json) |

В каждом процессе построены три независимых Pipeline: два одинаковых прогона и
допустимая перестановка creation order. Сравниваются весь O/M/R/C/S/T/E graph,
recursive lineage и полные upstream snapshots при каждой операции.
Ожидаемые semantic Fail/Unresolved в negative cases дают harness PASS при совпадении
с контрактом. Это не присваивает semantic Pass отвергнутым объектам.

[Повторная проверка целостности baseline](evidence/baseline-007-verification.json)
фиксирует hash артефактов, соответствие текущих файлов архиву MDC-IMPL-007 и
проверку обоих execution bundles. Сами Rust tests на этапе фиксации документов
заново не запускались; результаты выше относятся к исполнению MDC-IMPL-007.

## Материалы передачи

- [Contract-to-Implementation Matrix v0.1](MDC-CONTRACT-TO-IMPLEMENTATION-MATRIX-v0.1.md)
  разделяет IMPLEMENTED / PARTIAL / MISSING / UNVERIFIED.
- [Инструкция Codespaces](MDC-CODESPACES-HANDOFF-v0.1.md) описывает checksum,
  staging, повторное исполнение в новой среде и сохранение существующего Git-состояния.
- [Отчёт MDC-IMPL-007](MDC-IMPL-007.md) содержит привязки всех A01…A12/N01…N11,
  публичный API projection и подробные ограничения.

Новый архив `mdc-core-baseline-v0.1.tar.gz` включает неизменные файлы MDC-IMPL-007
и документы baseline. Его отдельный `mdc-core-baseline-v0.1-SHA256SUMS.txt`
проверяет целостность при переносе. Исторический архив MDC-IMPL-007 и оба его
патча не перепаковываются; incremental и cumulative patches остаются альтернативами
для MDC-IMPL-006 и первоначальных исходников соответственно.

## Условия дальнейшего сопровождения

Изменение исполняемых исходников делает сохранённый source fingerprint устаревшим:
после такого изменения обязательны релевантные component checks и новый E2E bundle.
Новая платформа/toolchain также требует отдельного исполнения и evidence.
Смена профиля или contracts требует явного определения новых ожидаемых результатов.

Следующие независимые направления описаны в матрице, но в baseline не реализуются:
историческая доступность `available_at`/`decision_at`, lifecycle invalidation с
propagation, дополнительная retention коллизий Evaluation и переносимый identity
protocol. У каждого направления должны быть собственные контрактные решения и
проверки. Реальные external source adapters и предметные projection profiles
также требуют отдельных контрактов.

Codespaces пользователя этим пакетом не изменён. Проверенный перенос в активный
Git root и решение о первом commit остаются действиями в целевой среде.
