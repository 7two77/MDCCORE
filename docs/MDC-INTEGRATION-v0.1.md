# MDC-INTEGRATION-001 — Integration candidate v0.1

2026-11-03. Репозиторий `7two77/MDCCORE`. Цель — подготовить проверяемый PR
в main с сохранением обеих исходных историй. Semantic implementation — MDC-IMPL-010.

## Проверенная история

До подготовки main указывает на `5bd25557aed4a6dcb08b7fb1bc2d14e91ca81dbf`.
Её отдельный root — `2dca7f93450d0766dd334d03d8b71487b15d0ea0`.
MDC baseline является вторым root. Проверенный стек уже линейный:

| Milestone | Commit | Hosted execution |
|---|---|---|
| Baseline v0.1 / OPS-001 | a8c15c7ffbbe48993e97142f6842da044bda4162 | [6 jobs PASS](https://github.com/7two77/MDCCORE/actions/runs/41518386565) |
| Lifecycle 008 | 70f37fc2b510f9436f1208ee378017dec444537e | [6 jobs PASS](https://github.com/7two77/MDCCORE/actions/runs/41556035798) |
| Evaluation retention 009 | d33bb979c0b8b6a69126fd79544d3f43e5df9d3b | [6 jobs PASS](https://github.com/7two77/MDCCORE/actions/runs/41614270848) |
| Portable identity 010 | b54b30febb7a63adfb12e23bd20ca030b71700da | [11 jobs PASS](https://github.com/7two77/MDCCORE/actions/runs/41619905838) |

Ветка `mdc/integration-v0.1` создана от исходной main. Integration merge имеет
**первого родителя main 5bd25557 и второго родителя 010 b54b30f**. Исходные commits
всего стека и оба roots становятся предками кандидата без изменения их SHA.
Публикация кандидата не перемещает remote main или существующие milestone branches.

PR #1 (009 → 008) и #2 (010 → 009) сохраняются как отдельные reviews этапов.
Integration PR переносит весь проверенный стек одним изменением относительно main;
отдельное слияние старых PR не является предпосылкой. После интеграции их статус
можно согласовать отдельно; подготовка не закрывает и не объединяет их автоматически.

## Разрешение конфликта и сохранность файлов

Единственный конфликт — add/add README.md. Root README сохраняет описание MDC Core
и получает ссылки на integration audit. Исходный main README сохранён побайтово в
[историческом файле](history/README-main-5bd25557.md), SHA-256:
`56235fc36fe5bb202b7d62253baef98e4dd298194b39f57d0d5b31da6adadd00`.

Исходный `PRISM-REFERENCE-RUNNER-001-v0.2-R2.zip` остаётся в корне побайтово,
SHA-256 `ddc4bdf2e81e63c776e3d80bb3700c4957b89ca19da6a887c8e52e0a44ac2624`.
Это сохранённый исходный материал; Cargo/CI не исполняет его как часть MDC.

Из 159 tracked files 010 только root README и `.github/workflows/ci.yml` получают
integration изменения. Остальные **157 files** защищены сравнением Git blob и mode.
Новые файлы — integration audit/guard и копии hosted receipts. Исторические
trailing spaces в исходной вложенной копии и main README сохранены побайтово;
whitespace check новых изменений сравнивает кандидата с 010 и исключает точный
архивный main README. `src`, `tests`,
`examples`, `scripts`, Cargo manifests/lockfiles и вложенная исходная копия
сохраняются. Implementation fingerprint остаётся:
`71e4f1e629242f8f6460741ba42987a96d9f2f3c677b2cd1d786703418b1b08b`.

[Integration plan](evidence/integration-plan-v0.1.json) фиксирует parents,
milestones, roots, exemptions, original file hashes и source fingerprint.

## Evidence и проверки

Hosted receipts baseline/008/009/010 сохранены в `docs/evidence/*-hosted-ci.json`.
Они побайтово совпадают с исходными execution receipts; временные download URLs
в них отсутствуют. [Portable comparison 010](evidence/impl-010-hosted-portable-comparison.json)
сохраняет actual target probes и одинаковый graph/lineage SHA-256 десяти исполнений.
Документы старых пакетов остаются историей подготовки, включая тогдашние
NOT_EXECUTED/UNVERIFIED; текущая [матрица v0.5](MDC-CONTRACT-TO-IMPLEMENTATION-MATRIX-v0.5.md)
ссылается на последующие published receipts.

Существующий CI сохраняет 6 native + 4 cross-target jobs и portable comparison.
Добавлен двенадцатый job `Preserve integration ancestry and the 010 source`.
Он получает полную Git history и проверяет:

* обязательных ancestors и последовательность baseline → 008 → 009 → 010;
* оба roots и integration commit с точными двумя parents;
* Git blobs/modes 157 защищённых files, исходный archive и исторический README;
* fingerprint всего implementation source из выбранного Git tree.

В режиме `--candidate` проверяется вершина integration branch/PR. Обычный режим
проверяет immutable integration tree в истории. Поэтому последующая работа в main
и feature branches может менять semantic code, сохраняя факт исходной интеграции.
History guard не закрепляет 010 как неизменяемую текущую реализацию навсегда.

[Local controls](evidence/integration-guard-controls.json) исполнили десять
сценариев: три положительных и семь ожидаемых отказов (одна история, squash,
обратный порядок parents, изменённые source/archive/исторический README).
Контрольный future-code commit проходит history mode и отклоняется candidate mode.
Синтетические commits создавались как локальные Git objects без изменения public refs.
[Local native CI](evidence/integration-local-verification.json) подтвердил
176 Rust tests, 17 E2E Python tests и 4 comparison Python tests на bridge commit.

```bash
python3 -B .github/ci/check_integration.py --candidate
python3 -B scripts/check_gate_readiness.py
```

Это команды для подготовленного кандидата. После дальнейших изменений main
history audit исполняется без `--candidate`.
Полный hosted integration run привязывается отдельным receipt к опубликованному
integration commit; прежний 010 PASS сам по себе не заменяет этот run.

## Порядок принятия

Сначала review integration PR и его CI. При принятии требуется **Merge commit**:
сохранение исходных commit IDs и обоих roots является условием этого изменения.
Squash/rebase integration PR не удовлетворяет этому условию. `--match-head-commit`
при CLI merge должен ссылаться на проверенный head из integration receipt.
Подготовка не создаёт release/tag и не меняет default branch settings.

После фактического merge Codespaces обновляет local main через fast-forward к
origin/main. До merge для проверки можно переключить отдельную integration branch.
Требования temporal availability, durable recovery и общий persisted graph importer
сохраняют прежние границы и не добавляются в этот integration package.
