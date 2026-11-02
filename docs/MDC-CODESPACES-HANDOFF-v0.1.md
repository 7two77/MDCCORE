# MDC Core — Codespaces Handoff v0.1

Цель: проверить baseline MDC-IMPL-007 в новой среде и подготовить перенос в
`/home/codespace/.cargo/bin/mdc_core`. В этой сессии Codespaces не изменялся.
Корень архива называется **`mdc-core/`**; имя Cargo package — `mdc_core`.

## 1. Проверить доставку и распаковать candidate

Поместите `mdc-core-baseline-v0.1.tar.gz` и
`mdc-core-baseline-v0.1-SHA256SUMS.txt` в `/tmp/mdc-transfer` на Codespaces.
Checksum подтверждает целостность полученного архива, а не подпись автора.
Если startup снова выдаёт `rvm_bash_nounset`, откройте
`bash --noprofile --norc`; `set -u` для этой процедуры не требуется.
Все блоки выполняются в одной Bash-сессии: `MDC_STAGE` и `MDC_CANDIDATE`
сохраняются между разделами. В новом терминале задайте их заново.

```bash
MDC_PACKAGE_DIR="/tmp/mdc-transfer"
cd "$MDC_PACKAGE_DIR" || exit 1
sha256sum --check mdc-core-baseline-v0.1-SHA256SUMS.txt || exit 1

MDC_STAGE=$(mktemp -d /tmp/mdc-baseline-stage.XXXXXX) || exit 1
tar -xzf mdc-core-baseline-v0.1.tar.gz -C "$MDC_STAGE" || exit 1
MDC_CANDIDATE="$MDC_STAGE/mdc-core"
cd "$MDC_CANDIDATE" || exit 1
python3 scripts/check_gate_readiness.py || exit 1
```

Этот checker подтверждает сохранённый execution bundle и совпадение source
fingerprint. Он ещё не доказывает выполнение на Codespaces. Ожидаемый fingerprint:
`9a907068ef266ae73d30d583a2ecb4eddb47b422078b640c60c5cf9d02569bc6`.

## 2. Исполнить проверки в целевой среде

Нужны Rust/Cargo, rustfmt, Clippy и Python 3.9+. Зависимости crate отсутствуют.
Baseline исполнялся на Rust/Cargo 1.85.1, `x86_64-unknown-linux-gnu`.
Запишите фактические версии; использование другой среды требует отдельного
evidence и не наследует межплатформенную гарантию identity.
Если 1.85.1 уже установлен через rustup, его можно выбрать:
`export RUSTUP_TOOLCHAIN=1.85.1`.

Команды выполняются последовательно из candidate; при неожиданном ненулевом
exit остановитесь и сохраните stdout/stderr. Build artifacts и новые отчёты
размещаются вне candidate, историческое evidence архива остаётся неизменным.

```bash
: "${MDC_STAGE:?Повторите раздел 1 или задайте путь staging}"
: "${MDC_CANDIDATE:?Задайте путь candidate из раздела 1}"
cd "$MDC_CANDIDATE" || exit 1
rustc -vV
cargo --version
python3 --version
export CARGO_TARGET_DIR="$MDC_STAGE/target"

cargo test --locked --offline
cargo test --locked --offline --features std,alloc
cargo test --locked --offline --no-default-features --features alloc
cargo check --locked --offline --no-default-features --features alloc
cargo fmt --all -- --check
cargo clippy --locked --offline --all-targets -- -D warnings
cargo clippy --locked --offline --no-default-features --features alloc --all-targets -- -D warnings

python3 scripts/run_e2e.py --output "$MDC_STAGE/e2e-codespaces-default.json"
python3 scripts/check_gate_readiness.py --report "$MDC_STAGE/e2e-codespaces-default.json"
python3 scripts/run_e2e.py --no-default-features --features alloc --output "$MDC_STAGE/e2e-codespaces-alloc.json"
python3 scripts/check_gate_readiness.py --report "$MDC_STAGE/e2e-codespaces-alloc.json"
python3 -m unittest discover -s tests -p test_e2e_report.py
```

Ожидается 124/124 Rust tests в каждой конфигурации, 17/17 Python tests,
PASS обоих новых bundles с 23/23 cases в каждом из двух процессов. Сохраните
оба JSON как свидетельство именно Codespaces execution, вместе с версиями и logs.
Проверка `--manifest-only` относится к исторической подготовке MDC-IMPL-006 и
ожидаемо возвращает exit 2; это не ошибка текущего gate.

## 3. Сопоставить с существующим Git root

В предоставленной диагностике Git root был на `main` без commits, с staged
`mdc_core/tests/mdc_e2e_conformance_001.rs` и множеством untracked files.
Актуальное состояние нужно перечитать; это не основание считать дерево чистым.

```bash
MDC_PROJECT="/home/codespace/.cargo/bin/mdc_core"
cd "$MDC_PROJECT" || exit 1
git rev-parse --show-toplevel
git branch --show-current
git status --porcelain=v1 -uall -- ':!**/target/**'
git diff --cached --stat
diff -ru --exclude=.git --exclude=target --exclude=__pycache__ "$MDC_PROJECT" "$MDC_CANDIDATE"
```

У `diff` exit 1 означает найденные различия. Если Git root отличается от указанного
пути, сначала установите фактическую границу репозитория.
Вложенные исходники сохраняются; их 18 архивных файлов совпадают с первоначальной
копией. `target/` не входит в переносимый baseline.

## 4. Выбрать способ переноса после сверки

Проверенная staging-копия уже пригодна для отдельной работы через её корневой
Cargo manifest. Перенос в существующий Git root должен учитывать найденные
локальные различия. До записи сохраните отдельный backup рабочего дерева,
включая untracked files, Git index и Git metadata. Обычный `git stash` нельзя
считать backup unborn-репозитория. Команды `reset`, `clean` и восстановление
дерева из несуществующего HEAD здесь не нужны.

Доступные patches MDC-IMPL-007 — альтернативные пути к одной реализации:

| Patch | Требуемая база | Manifest проверки базы из candidate |
|---|---|---|
| `mdc-core-impl-007.patch` | Точная копия MDC-IMPL-006 | `docs/evidence/impl-007-source-manifest.json`, поле `original_file_sha256` |
| `mdc-core-impl-007-from-original.patch` | Первоначальные исходники из пользовательского архива | `docs/evidence/source-manifest.json`, поле `original_file_sha256` |

Перед patch должны совпасть hashes всех файлов выбранной базы; каждый новый путь
patch должен отсутствовать либо быть явно разобран как локальное изменение.
Наличие других файлов в `src`, `tests`, `examples`, `scripts` также требует сверки:
они входят в итоговый implementation fingerprint. Один успешный `patch --dry-run`
не доказывает точного совпадения базы и не защищает untracked файлы от конфликта.
Сначала применяйте выбранный patch к отдельной копии базы и сравнивайте результат
со staging candidate. Оба patch содержат реализацию и документы MDC-IMPL-007;
три новых документа baseline и его integrity report поставляются в baseline archive.

После фактического переноса повторите checker из активного root и сохраните
Git status/index comparison. Snapshot evidence относится к source fingerprint;
локальные изменения требуют нового исполнения. Создание первого commit и судьба
вложенной копии — отдельные решения, они не выполняются этой инструкцией.

## Критерий завершения передачи

1. Checksum архива совпал, candidate выделен однозначно.
2. Проверки Codespaces завершены с ожидаемыми exit codes; новые bundles сохранены.
3. Все локальные различия и staged/untracked файлы учтены до записи в Git root.
4. Активный пакет после выбранного переноса даёт ожидаемый fingerprint и checker PASS.

До выполнения этих действий статус передачи — **PREPARED / NOT EXECUTED**.
Scoped PASS ядра и успешная передача в пользовательскую среду — отдельные факты.
