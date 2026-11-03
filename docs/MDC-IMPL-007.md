# MDC-IMPL-007 — E2E runner и technical projection isolation

Дата: 2026-11-02. Статус: **MDC-E2E-CONFORMANCE-001 PASS для объявленного fixture
MDC-SCALAR-001 и записанной реализации**. Нормативное основание — предоставленный
STP-MDC v0.1 DRAFT / NORMATIVE, Conformance Boundary и E2E-контракт.
Это scoped execution result: он относится к указанным source fingerprint,
toolchain, features и target, а не ко всем возможным MDC-контрактам и платформам.

## Исполняемый gate

Fixture [mdc_e2e](../examples/support/mdc_e2e.rs) вызывает реальные операции
`Pipeline`: admission, Measurement, Relation, Composition, State, Transition,
Evaluation и projection. Собственных алгоритмов измерения, derivation или
conformance fixture не добавляет. CLI [examples/mdc_e2e.rs](../examples/mdc_e2e.rs)
выдаёт structured JSON; пропущенная, неисполненная или failed обязательная
процедура не получает PASS.

[Execution bundle](evidence/mdc-e2e-conformance-001.json) содержит два отдельных
OS-процесса с фактическими различными Rust PID. В каждом исполнены все **23/23**
процедуры: **E2E-A01…A12 и N01…N11**. В каждом процессе построены три независимых
Pipeline: два одинаковых прогона и прогон с переставленным допустимым порядком
создания объектов. Сравниваются полные O/M/R/C/S/T/E canonical bytes, identity
и recursive lineage, а также результаты между процессами.

Положительный граф содержит 16 объектов: 4 Observation, 4 Measurement,
2 Relation, 2 Composition, 2 State, 1 Transition и 1 Evaluation. Lineage
достигает зарегистрированного Source. Перед и после каждой из 16 операций
снимаются snapshots всех ранее возвращённых semantic objects; сравнивается
полный накопленный prefix. Getter ID проверяется против ObjectRef ID и
identity canonical basis. Дополнительные скрытые объекты, invalid artifacts
и rejection receipts в положительном графе не допускаются.

| Обязательные процедуры | Фактически проверяемое поведение |
|---|---|
| A01 | Разрешение references, definitions и точных версий контрактов |
| A02 | Детерминизм полного графа в независимых Pipeline и при допустимой перестановке |
| A03 | Полная recursive lineage closure до Source |
| A04 | Отсутствие изменения upstream bytes и identity после downstream операций |
| A05…A07 | Реальные Pass, Unresolved и Fail; агрегация `FAIL > UNRESOLVED > PASS` |
| A08 | Сохранение шести typed invalid bodies, диагностик и captured inputs |
| A09 | Отсутствующий target State не синтезируется |
| A10 | Запрещённые предметные labels/types не попадают в semantic namespace |
| A11…A12 | Зарегистрированная projection, detached representation и запрет reverse mutation |
| N01…N03 | Unknown version, missing Composition и undefined conformance criterion |
| N04…N05 | Недостаточное подтверждение и явное нарушение критерия |
| N06…N08 | Missing target, несовместимый временной порядок и неполный lineage |
| N09…N11 | Перестановка, backflow attempt и отсутствие Source/target без fallback |

N08 использует известный зарегистрированный operator и отсутствующий Observation:
это отдельная проверка lineage, а не совмещённая ошибка unknown operator.
N11 использует отсутствующий Source/target; запрет произвольного будущего
timestamp из этого не выводится.

Harness PASS означает совпадение фактического поведения с ожидаемым контрактом.
Для A06/N04 ожидается semantic `Unresolved`, для A07/N05/N07 — `Fail`, для A05 —
`Pass`. Их успешные negative-проверки не превращают semantic Fail в Pass.
У процедур без вычисляемого semantic result это поле остаётся отсутствующим
значением; результат не выводится из текста ошибки.

## Technical projection boundary

Добавлены `ProjectionProfile`, `ProjectionRequest`, `ProjectionError` и detached
`Representation`, а также registration/lookup profile в `Registry` и методы
`Pipeline::project()` / `apply_representation()`.

Profile содержит точные versioned references mapping/schema/conformance,
target type и context. Поддерживается фиксированная техническая схема
`TEST-REP-01/1`. Registration проверяет её, наличие conformance definition,
target type и context; перезапись profile не разрешена.

`project()` читает admitted target одного из шести типов и требует сохранённую
Evaluation с `Conformant`, semantic `Pass` и без diagnostics. Evaluation должна
относиться к точному target, его текущим semantic bytes, заданным conformance
contract и context. Unknown profile/target/Evaluation, неправильный тип и
несовпадение basis дают явные ошибки.

Representation сохраняет profile/schema/conformance, source/Evaluation refs,
context и снимки обоих semantic bodies. Поле `label` доступно для внешнего
редактирования; provenance и mapping fields доступны через getters. У неё нет
варианта ObjectRef и calculus identity: она не вставляется в core graph.
Повторная projection даёт одинаковые external canonical bytes. Изменение label
меняет только внешние bytes. В fixture label изменяется, после чего реальная
попытка reverse operation возвращает `BackflowForbidden`; весь core graph
сохраняется. Обратный transformation contract в этом профиле не зарегистрирован.

Это технический mapping без рыночной семантики. Label не придаёт State или
Transition смысл торгового сигнала и не устанавливает trading readiness.

## Независимая проверка отчёта

[run_e2e.py](../scripts/run_e2e.py) запускает CLI два раза, сохраняет stdout/report,
stderr и exit codes, добавляет rustc/cargo, features, host target и SHA-256 manifest
реализации. Fingerprint покрывает Cargo manifest/lockfile, `src`, `examples`,
`tests` и `scripts`. Документация, отчёты и build artifacts исключены из него,
чтобы evidence не ссылалось рекурсивно на собственный hash.

[e2e_report.py](../scripts/e2e_report.py) независимо проверяет exact case coverage
и фиксированные ожидаемые semantic results, execution flags, diagnostics и
непустое evidence. Проверяются три положительных графа, identity/reference
consistency, lineage, coverage всех 16 операций и полные prefix snapshots.
В production API и Rust assertions проверяется binding target/Evaluation.
Python validator сверяет projection source basis с Transition в графе,
external before/after bytes, зафиксированную попытку reverse operation
и неизменность фактического core graph; external bytes заново не декодирует. Затем
проверяются разные process IDs, равенство графов между процессами и совпадение
fingerprint с текущим исходным кодом. Validator проверяет evidence и не
реализует заново MDC calculus.

Rust acceptance tests отклоняют missing/duplicate case, unexecuted/failed case,
пустое evidence и semantic mismatch. Python mutation tests проверяют отказ
независимого validator при повреждении evidence. Полная матрица Rust tests,
Format/Clippy, примеров и Python checks сохраняется в
[verification evidence](evidence/impl-007.json).

Фактически прошли **124/124 Rust tests** в каждой из конфигураций default,
`std,alloc` и `no_std + alloc`, **17/17 Python mutation tests**, Format и Clippy
в default и alloc. E2E launcher отдельно выполнен в default и alloc: в каждом
bundle два процесса исполнили 23/23 процедуры; canonical graph между
конфигурациями совпал. [Alloc bundle](evidence/mdc-e2e-conformance-001-alloc.json)
сохраняет отдельные команды и результаты. Режим без alloc ожидаемо отклонён
явной диагностикой. Compatibility probe подтвердил прежние bytes и identity
всех 16 объектов контрольной цепочки MDC-IMPL-006.

SHA-256 реализации в обоих execution bundles:
`9a907068ef266ae73d30d583a2ecb4eddb47b422078b640c60c5cf9d02569bc6`.
Toolchain: rustc 1.85.1 / cargo 1.85.1, host `x86_64-unknown-linux-gnu`.

```bash
cargo test --locked --offline
cargo test --locked --offline --features std,alloc
cargo test --locked --offline --no-default-features --features alloc
cargo check --locked --offline --no-default-features --features alloc
cargo fmt --all -- --check
cargo clippy --locked --offline --all-targets -- -D warnings
python3 -m unittest discover -s tests -p 'test_e2e_report.py'
python3 scripts/run_e2e.py
python3 scripts/check_gate_readiness.py
```

E2E runner и Python launcher требуют hosted std/Python environment. Библиотечная
technical projection доступна в no_std+alloc. Hosted tests не подтверждают
allocator/linker setup произвольного bare-metal target.

## Evidence и сохранённая история

- [Текущий execution bundle](evidence/mdc-e2e-conformance-001.json): raw reports,
  snapshots, full invalid keys/captured bases, external representation bytes,
  expected/actual results и metadata.
- [Execution bundle no_std+alloc](evidence/mdc-e2e-conformance-001-alloc.json):
  та же библиотека с hosted std runner и отключённым default feature.
- [Матрица проверок](evidence/impl-007.json): команды и фактические результаты.
- [Compatibility probe](evidence/impl-007-compatibility.json): сравнение 16 прежних
  O/M/R/C/S/T/E objects с MDC-IMPL-006 на контрольной цепочке.
- [Source manifest](evidence/impl-007-source-manifest.json): границы изменений.

Исторические [readiness matrix](MDC-E2E-GATE-READINESS-v0.1.md) и
[manifest MDC-IMPL-006](../tests/fixtures/mdc_e2e_gate_v1.json) сохранены со статусом
NOT_EXECUTED / BLOCKED. Они описывают подготовку до появления runner.
`check_gate_readiness.py` по умолчанию проверяет текущий execution report и
fingerprint; exit 0 означает валидный текущий PASS. Флаг `--manifest-only`
проверяет прежнюю подготовку и ожидаемо возвращает exit 2.

Вложенная исходная копия, lockfiles и отчёты предыдущих пакетов сохраняются.
Codespaces пользователя не изменяется. Incremental patch предназначен для
MDC-IMPL-006; cumulative patch — для первоначального архива. Это альтернативные
способы восстановления одной подготовленной копии.

## Сохраняющиеся ограничения

Последующая invalidation admitted objects и propagation по существующим
descendants не реализованы. Conformance Evaluation не меняет target; статусы
State/Transition Candidate сохраняют прежнюю семантику. Failed construction
сохраняет typed invalid body, что не заменяет lifecycle invalidation.

Source trust остаётся declared metadata. Horizon ограничивает source timestamp,
а не `available_at`/`decision_at`; историческая доступность, внешняя подлинность
Source и общий as-of replay не установлены этим gate. В частности, позднее
resolution старого invalid Evaluation имеет ограничения MDC-IMPL-006.

Legacy native Rust Hash и u64 identity имеют прежние ограничения переносимости
и коллизий. Полные canonical keys различают шесть typed invalid namespaces;
отдельного collision-retention namespace для Evaluation нет: его собственная
коллизия возвращает ошибку без перезаписи. Ни PASS fixture, ни два OS-процесса
не доказывают глобальную semantic uniqueness и межплатформенный детерминизм.

Произвольные schemas, units conversion, общие external evidence contracts,
рыночные projection profiles, VAIXLNS-Builder и STSA-MC в пакет не входят.
