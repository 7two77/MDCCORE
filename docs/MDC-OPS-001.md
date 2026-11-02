# MDC-OPS-001 — Git baseline и CI

## Состояние и область

Подготовка первого Git commit для MDC Core Baseline v0.1 и workflow проверок.
Lifecycle, canonical bytes и публичный API ядра не меняются. Source fingerprint:
`9a907068ef266ae73d30d583a2ecb4eddb47b422078b640c60c5cf9d02569bc6`.

Пользователь подтвердил перенос 109 файлов baseline в Git root
`/home/codespace/.cargo/bin/mdc_core`, checker PASS и неизменность прежнего index.
Backup переноса: `/home/codespace/mdc-core-backups/mdc-original-3doeuxhp`.

В предоставленном выводе Codespaces проверки прошли на Rust/Cargo 1.98.0,
Python 3.12.1, `x86_64-unknown-linux-gnu`: 124/124 Rust tests в трёх конфигурациях,
Format, Clippy, 17/17 Python tests и оба E2E bundles по 23/23 в двух процессах.
Числа Rust tests основаны на предоставленном console output. Два raw E2E reports
при подготовке index отдельно проверяются и копируются из backup в `docs/evidence/`;
их нельзя заменить отчётами Rust 1.85.1 под новым именем.

## Состав первого commit

В allowlist входят все 109 файлов baseline, CI workflow/driver, этот документ,
локальные результаты проверки CI и фактические Codespaces E2E reports с summary.
Все 18 файлов вложенного `mdc_core/` включаются как исходный reference snapshot,
без подключения к Cargo workspace. Уже staged E2E-файл входит в эту группу;
его bytes сохраняются. `target/` на любой глубине, `ci-evidence/`, Python caches,
backup и посторонние файлы не добавляются в index.

`prepare.py` из пакета MDC-OPS-001:

1. Проверяет точную базу, hashes payload и новых путей, отсутствие commits,
   staged entries и фактические E2E reports Rust 1.98.0.
2. В режиме `--prepare-index` сохраняет backup дерева вместе с `.git`, прежний
   index и список staged entries вне проекта; backup проверяется чтением архива.
3. Устанавливает только проверенные изменения, сохраняет оба raw Codespaces
   reports и создаёт machine-readable summary на их основе.
4. Проверяет core fingerprint и reports, добавляет точный allowlist, проверяет `git diff --cached --check`,
   совпадение staged bytes с планом и подготавливает tree SHA через `git write-tree`.
5. Записывает diff stat, предложенный commit message и audit record в backup.

Без флага `--prepare-index` выполняется только preflight. Скрипт не создаёт commit,
tag, remote branch или push. В случае отказа после начала записи backup и audit
record показывают выполненный шаг; автоматического удаления локальных файлов нет.

Предлагаемая тема первого commit: `Restore MDC Core baseline and add CI`.
Перед commit проверяются `git diff --cached --stat`, allowlist и сохранённый audit.
Прежний index доступен в backup, а новый index представляет весь baseline.
Whitespace check применяется к поддерживаемым корневым файлам; замороженный
`mdc_core/**` исключён, поскольку его исходные trailing spaces сохраняются побайтово.

## CI

[Workflow](../.github/workflows/ci.yml) запускается на push, pull_request и вручную.
Матрица: Rust **1.85.1 / 1.98.0** × **default / std-alloc / alloc**, Ubuntu 24.04,
Python 3.12. Crate dependencies не добавлены. Установка toolchain/actions требует
сетевого доступа GitHub runner; Cargo-команды используют `--locked --offline`.

[Driver](../.github/ci/run.sh) в каждой ячейке исполняет Format, Rust tests,
library check, Clippy, пять примеров, два отдельных E2E процесса, evidence checker
и 17 проверок Python validator. В default дополнительно проверяется явный отказ
библиотеки без alloc. Ошибки pipeline/tee сохраняют ненулевой exit code.

Python suite читает фиксированный путь исторического bundle. Driver временно
подставляет свежий report только для этой suite, а EXIT trap восстанавливает
исторический файл при успехе и отказе. Crate semantic code при этом не меняется.

Логи, полный свежий execution bundle и metadata загружаются через
`upload-artifact` с `if: always()`, срок хранения 14 дней. При ошибке установки
toolchain driver ещё не запущен, и доступными могут остаться только GitHub step logs.
Metadata включает commit SHA, Rust/Cargo/Python, core source fingerprint и hashes
CI files. `.github/` не входит в прежний core fingerprint, поэтому workflow/driver
идентифицируются отдельно. Проверки двух Rust версий выполняются независимо;
это само по себе не доказывает равенство identity между toolchains.

Локальный запуск из корня:

```bash
bash .github/ci/run.sh default
bash .github/ci/run.sh std-alloc
bash .github/ci/run.sh alloc
```

По умолчанию отчёты создаются в `ci-evidence/<profile>/` и игнорируются Git.
`MDC_CI_REPORT_DIR` и `CARGO_TARGET_DIR` позволяют вынести их за пределы проекта.

Локальные результаты Rust 1.85.1 сохранены в
[ops-001-local-ci.json](evidence/ops-001-local-ci.json).
Hosted GitHub Actions execution получает статус только после реального запуска
workflow в репозитории. Этот пакет не утверждает, что workflow уже запущен.

## Критерии завершения

- Подготовленный index соответствует точному allowlist, вложенная копия сохранена.
- Фактические Codespaces reports проверены и включены без подмены происхождения.
- Первый commit создаётся после review подготовленного дерева.
- После отправки commit workflow реально запускается и сохраняет artifacts.

Последние два действия остаются отдельными шагами целевой среды.
