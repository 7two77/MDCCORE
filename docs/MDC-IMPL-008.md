# MDC-IMPL-008 — Lifecycle invalidation и зависимости

Рабочая копия основана на опубликованном baseline commit
`a8c15c7ffbbe48993e97142f6842da044bda4162`. Codespaces, удалённые ветки и main
этим пакетом не менялись; commit/push MDC-IMPL-008 не создавались.

## Реализованный контракт

[MDC-LIFECYCLE-001/1](MDC-LIFECYCLE-CONTRACT-v0.1.md) фиксирует выбранную семантику
до заявлений о conformance: retained semantic snapshot и отдельный append-only
lifecycle journal, monotonic invalidation, required declared dependencies и
различие исторической Evaluation и текущей admissibility.

Добавлены versioned LifecycleContract, InvalidationRequest, LifecycleEvent,
полный LifecycleEventId, LifecycleView и typed lifecycle errors/rejections.
Pipeline предоставляет invalidate, lifecycle_revision, lifecycle_events,
resolve_lifecycle_event, lifecycle_view и lifecycle_view_at.

Инвалидация существующего O/M/R/C/S/T вычисляет descendants по реальному lineage,
включая State witnesses и Evaluation. Сохраняются bytes, identity, старые
результаты критериев и provenance каждого прежнего semantic artifact.
Новые construction paths отвергают revoked inputs; создание того же canonical ID
не восстанавливает объект. Новая Evaluation сохраняет Invalid/Fail и конкретные
причины lifecycle, не переписывая историческую Pass Evaluation.
Projection требует актуальную usable границу target и Evaluation.

Политика проверяет тип/context/scope, точную версию, reason и доступные witnesses;
она не доказывает истинность внешнего утверждения вызывающего участника.
Source-wide revoke, target promotion и reinstatement не добавлены.

## Snapshot status и current status

`resolve_*().status()` описывает сохранённый artifact. Для текущего использования
нужно `pipeline.lifecycle_view(&reference)?.status`.

```text
stored State: Candidate, прежние bytes/identity
current LifecycleView: Invalid + причинные events

stored Evaluation: Pass / Conformant, прежние bytes/identity
current LifecycleView: Invalid
new Evaluation: Fail / Invalid + TargetInvalidated + lifecycle context
```

Это API-решение ограниченного профиля. Оно сохраняет status в прежнем canonical
protocol и трактует последующий отзыв как отдельный canonical decision.
Внешние consumers, которые принимали решение только по старому status getter,
должны учитывать lifecycle view. Production operations Pipeline проверяют его сами.
Публичные enum получили новые variants: внешние exhaustive match требуют обновления.

`lifecycle_view_at` воспроизводит решения по retained objects на lifecycle epochs.
Новые objects запоминают admission epoch; порядок их создания внутри epoch не
моделируется. Это не general graph creation replay или historical availability.
Journal не имеет persisted import/crash recovery. Full keys/snapshots занимают
память; descendants определяются сканированием графа.

## Проверки и evidence

Source SHA-256:
`a8482f335237af67d0666bf67e5b942106c223220da1995b12ca27ad8eded7fe`.

| Проверка | Фактический результат |
|---|---|
| Rust 1.85.1 × default / std-alloc / alloc | 145/145 tests в каждой конфигурации |
| Rust 1.98.0 × default / std-alloc / alloc | 145/145 tests в каждой конфигурации |
| Format, Clippy, library check и примеры | PASS в шести локальных заданиях |
| Python evidence validator | 17/17 в каждом задании |
| Прежний MDC-E2E-CONFORMANCE-001 | 23/23 в каждом из двух процессов каждого задания |
| Lifecycle example | Два самостоятельных прогона каждого задания; canonical event bytes совпали |
| Legacy compatibility | Полный 16-object O/M/R/C/S/T/E graph и lineage совпали с baseline во всех шести конфигурациях |
| Hosted CI для MDC-IMPL-008 | NOT_EXECUTED |

145 = 124 прежних Rust tests + 20 integration lifecycle tests + 1 проверка
full-key disambiguation journal IDs. [Тесты](../tests/lifecycle.rs) вызывают реальные
core operations; [пример](../examples/lifecycle.rs) демонстрирует полную цепочку
и отзыв с сохранением snapshots. Его fixture не реализует собственный calculus.

Существенные отрицательные сценарии: неизвестная contract version, неправильные
type/context/reason, missing target/evidence, duplicate witnesses, revoked witness,
устаревший expected revision, конфликт request ID и попытки восстановления того же
ID. Проверяется freeze причин в прежних invalid artifacts и разные причинные
контексты повторных Evaluation. Неизвестный conformance contract сохраняет null
semantic result, даже когда lifecycle context target известен.

Первая сборка Rust 1.98.0 остановилась с `No space left on device`. Повтор выполнен
после удаления task build cache с `CARGO_PROFILE_DEV_DEBUG=0` и
`CARGO_PROFILE_TEST_DEBUG=0`. Все три профиля прошли. Ошибка и build environment
сохранены в attempt report, она не подменена успешным исходным запуском.
Обе версии исполнялись на x86_64-unknown-linux-gnu; это не гарантия других платформ.

- [Полная сводка](evidence/impl-008.json): CI metadata, hashes логов, compatibility и source fingerprint.
- [История попыток](evidence/impl-008-local-verification.json): команды driver, результаты и инфраструктурный retry.
- [Новый default E2E bundle](evidence/mdc-e2e-conformance-001-impl-008.json).
- [Новый alloc E2E bundle](evidence/mdc-e2e-conformance-001-alloc-impl-008.json).
- [Source manifest](evidence/impl-008-source-manifest.json): точные границы изменений.
- [Matrix v0.2](MDC-CONTRACT-TO-IMPLEMENTATION-MATRIX-v0.2.md).

Полные логи и raw reports шести локальных jobs поставлены отдельно в
`mdc-core-impl-008-ci-evidence.tar.gz`. Они относятся к рабочей копии с base commit
a8c15c7 и `worktree_dirty=true`; это не GitHub Actions execution нового commit.
Baseline hosted CI run 41518386565 остаётся свидетельством только MDC-OPS-001.

## Новое current evidence и CI

Исторические reports MDC-IMPL-007 и Codespaces baseline сохранены побайтово.
`scripts/e2e_report.py::DEFAULT_REPORT` указывает на новый bundle IMPL-008;
launcher, checker, Python tests и CI используют один и тот же путь.
Прошлый report закономерно содержит прежний fingerprint и не выдаётся за текущий.

CI driver дополнен двумя lifecycle runs и сравнением их outputs; metadata теперь
также фиксирует dirty worktree. При проверке Python suite временно используется
свежий report с восстановлением поставленного current bundle по EXIT trap.
Workflow matrix и его external actions не менялись.

```bash
bash .github/ci/run.sh default
bash .github/ci/run.sh std-alloc
bash .github/ci/run.sh alloc
python3 scripts/check_gate_readiness.py
```

## Применение

Patch `mdc-core-impl-008.patch` предназначен **только для commit a8c15c7** с чистым
рабочим деревом. Включены исходники, tests, docs и новые current reports. Полный
архив — альтернативный способ получить ту же подготовленную копию.
SHA256SUMS проверяет выбранный файл доставки; старые baseline/OPS архивы не переупакованы.

После загрузки patch и SHA256SUMS в `/home/codespace/mdc-transfer`:

```bash
bash --noprofile --norc <<'MDC_IMPL_008'
set -e
cd /home/codespace/mdc-transfer
sha256sum --check --ignore-missing mdc-core-impl-008-SHA256SUMS.txt
cd /home/codespace/.cargo/bin/mdc_core
test "$(git rev-parse HEAD)" = a8c15c7ffbbe48993e97142f6842da044bda4162
test -z "$(git status --porcelain=v1 -uall)"
git switch -c mdc/impl-008-lifecycle
git apply --check /home/codespace/mdc-transfer/mdc-core-impl-008.patch
git apply /home/codespace/mdc-transfer/mdc-core-impl-008.patch
python3 -B scripts/check_gate_readiness.py
git diff --stat
MDC_IMPL_008
```

Перед новым commit выполняются три профиля CI driver, review API/контракта и
обновлённой матрицы. Старые branches/main и commit baseline сохраняются.
Этот блок не выполняет commit, push или объединение независимых Git histories.

## Оставшиеся границы

MDC-IMPL-009 Evaluation collision-retention и MDC-IMPL-010 portable identity
не реализованы. Historical available_at/decision_at, external source authenticity,
автоматический ремонт и большие persisted graphs не следуют из scoped PASS.
VAIXLNS-Builder и STSA-MC остаются вне области работ.
