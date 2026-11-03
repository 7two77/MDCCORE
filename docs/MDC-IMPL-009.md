# MDC-IMPL-009 — Evaluation collision retention

Дата: 2026-11-03. Рабочая копия основана на опубликованном commit
`70f37fc2b510f9436f1208ee378017dec444537e`, MDC-IMPL-008.
Commit/push пакета 009 не создавались; Codespaces и remote branches не менялись.

## Изменение

[Контракт MDC-EVALUATION-RETENTION-001/1](MDC-EVALUATION-RETENTION-CONTRACT-v0.1.md)
определяет authoritative full keys и compatibility aliases.
Evaluation теперь хранится по `EvaluationId { digest, canonical_key }`.
Коллизия с другой Evaluation или с core numeric ID сохраняет новый результат
в отдельном typed namespace вместо отказа/перезаписи.

Добавлены `ObjectRef::EvaluationKey`, `ConformanceEvaluation::key()`,
`EvaluationId::from_canonical_basis()`, `Pipeline::evaluate_keyed()`,
`evaluation_key()`, `canonical_evaluation_reference()`, `evaluation_references()`
и `evaluations_with_digest()`.

`evaluate()` сохраняет обычные numeric references, когда reservation доступен.
Старый alias навсегда относится к прежнему body. При коллизии возвращается full key.
`evaluate_keyed()` при первой insertion не создаёт alias и всегда возвращает full key.
Это путь для collision-independent naming; legacy alias ownership по определению
сохраняет историю первого reservation.

Повторная запись одинакового key не увеличивает artifact count. Bucket lookup
показывает все члены с полными keys. Неизвестный key не разрешается через один
только digest. Конфигурационные ошибки и computed Pass/Fail/Unresolved/None
сохраняют прежние result, status, criteria, diagnostics и provenance.

## Ссылки, lifecycle и совместимость

ObjectRef codec дополнен tag 14: digest LE и length-prefixed full canonical body.
Теги 0…13 и canonical body самой Evaluation не меняются. Storage preference
не является частью semantic basis Evaluation.

Trace, captured invalid lineage и projection разрешают полный key точно.
Lifecycle нормализует обе формы handle к immutable primary reference artifact:
invalidation/epoch/history нельзя обойти другой формой ссылки. Отзыв одной
Evaluation не инвалидирует другую лишь из-за одинакового digest.

Keyed-only Evaluation не резервирует numeric namespace O/M/R/C/S/T. Старый
numeric alias продолжает защищать свой ID прежним collision guard.
EvaluationKey не является допустимым Measurement input или новым conformance target.
Создание lookup key из произвольных bytes не допускает Evaluation к графу.

Внешняя Representation сохраняет конкретный выбранный handle. Её bytes могут
отличаться для legacy/keyed формы одного artifact; source/Evaluation snapshots
и сам calculus при этом одинаковы.

Публичный ObjectRef расширен: внешние exhaustive match нужно обновить.
Для новых consumers рекомендуется canonical keyed API, а не предположение,
что результат evaluate всегда имеет вид Evaluation(u64).

## Проверки

Source SHA-256:
`d5e5f48b47910d9095fa6c7cd5f40a4155d6ea44db70415fbce62d3553d00b1e`.

Локальная матрица: **Rust 1.85.1 / 1.98.0 × default / std-alloc / alloc**,
target x86_64-unknown-linux-gnu. В каждом из шести jobs:

- **168/168 Rust tests**: 145 прежних + 16 collision scenarios + 1 codec test + 6 public integration scenarios;
- **17/17 Python checks**, Format, Clippy, library check, примеры;
- старый E2E: 23/23 в каждом из двух отдельных процессов;
- два lifecycle runs и два keyed-reference example runs с одинаковыми outputs.

[Collision tests](../src/pipeline/evaluation_retention_tests.rs) меняют только
private digest seam под cfg(test). Production criteria, canonical content,
storage, lineage, invalidation и projection остаются реальными. В обычной сборке
нет public injection API, feature или env-переключателя digest.
[Public integration tests](../tests/evaluation_retention.rs) и
[пример](../examples/evaluation_retention.rs) используют normal identity function;
они не выдаются за найденную natural FNV collision.

Покрыты alias pinning, reverse insertion order keyed graphs, colliding core ID,
все виды результатов, ошибочный key/digest, полные snapshots, captured invalid
inputs, projection mismatch, lifecycle history и запрет обхода invalidation.

Контрольный 16-object O/M/R/C/S/T/E graph и lineage совпали с 008 во всех jobs.
Сохранился canonical lifecycle event с SHA-256
`36daa543e61875b193c7287a86ebb08cddfa35b866d22e40abc1ccc1f2ddfaa0`.
Новый keyed snapshot совпал между двумя процессами каждого job и всеми шестью jobs.
Это свидетельства declared fixtures на одном target, не общий portable identity proof.

- [Полная сводка](evidence/impl-009.json): metadata, hashes логов и compatibility.
- [Команды и результаты](evidence/impl-009-local-verification.json).
- [Новый default E2E bundle](evidence/mdc-e2e-conformance-001-impl-009.json).
- [Новый alloc E2E bundle](evidence/mdc-e2e-conformance-001-alloc-impl-009.json).
- [Source manifest](evidence/impl-009-source-manifest.json).
- [Матрица v0.3](MDC-CONTRACT-TO-IMPLEMENTATION-MATRIX-v0.3.md).

Build env локальной матрицы: CARGO_INCREMENTAL=0, CARGO_PROFILE_DEV_DEBUG=0,
CARGO_PROFILE_TEST_DEBUG=0. Полные logs/raw reports поставляются отдельно в
`mdc-core-impl-009-ci-evidence.tar.gz`.
Hosted CI нового commit 009 — **NOT_EXECUTED**. Прошедший run 41556035798
относится к 008/70f37fc и не выдаётся за проверку 009.

## CI и история evidence

Driver дополнен двумя runs evaluation_retention и сравнением outputs.
Прежние lifecycle и core E2E checks сохраняются. Старые reports 001…008,
Codespaces baseline, вложенная копия из 18 файлов, Cargo manifests и lockfiles
не менялись. DEFAULT_REPORT указывает на новый IMPL-009 bundle; старый report
не подменяет current fingerprint.

## Передача в Codespaces

Достаточно загрузить **архив и SHA256SUMS**. Большой patch скачивать не требуется:
в архив входит `mdc-delivery/apply.py` рядом с `mdc-core/`.
Скрипт проверяет archive checksum, candidate bytes, чистый Git root, base commit
70f37fc и hashes всех базовых файлов. Он создаёт Git bundle/index backup и
сохраняет предыдущий ci-evidence до записи. Затем строит и проверяет patch,
создаёт новую ветку и сравнивает все применённые файлы с candidate.
Commit, index staging, push и изменение main не выполняются.

Без `--apply` выполняется только project-read-only preflight; temporary patch
пишется в каталог распаковки. В случае ошибки после начала записи backup и
apply.json фиксируют шаг. Автоматических reset/clean или удаления local files нет.

```bash
bash --noprofile --norc <<'MDC_IMPL_009'
set -e
MDC_DELIVERY=""
for dir in /home/codespace/mdc-transfer /tmp/mdc-transfer; do
    if [ -f "$dir/mdc-core-impl-009.tar.gz" ] &&
       [ -f "$dir/mdc-core-impl-009-SHA256SUMS.txt" ]; then
        MDC_DELIVERY="$dir"
        break
    fi
done
if [ -z "$MDC_DELIVERY" ]; then
    printf 'STOP: загрузите архив 009 и SHA256SUMS в каталог mdc-transfer\n'
    exit 1
fi
cd "$MDC_DELIVERY"
sha256sum --check --ignore-missing mdc-core-impl-009-SHA256SUMS.txt
MDC_STAGE=$(mktemp -d /home/codespace/mdc-009-stage.XXXXXX)
tar -xzf mdc-core-impl-009.tar.gz -C "$MDC_STAGE"
python3 -B "$MDC_STAGE/mdc-delivery/apply.py" \
  --project /home/codespace/.cargo/bin/mdc_core \
  --archive "$MDC_DELIVERY/mdc-core-impl-009.tar.gz" \
  --backup-dir /home/codespace/mdc-core-backups --apply

cd /home/codespace/.cargo/bin/mdc_core
export CARGO_TARGET_DIR=/home/codespace/mdc-build/impl-009
export CARGO_PROFILE_DEV_DEBUG=0
export CARGO_PROFILE_TEST_DEBUG=0
for profile in default std-alloc alloc; do
    bash .github/ci/run.sh "$profile"
done
printf '\nMDC_IMPL_009_CODESPACES=PASS\n'
MDC_IMPL_009
```

Отдельный patch — альтернатива архиву; его база также только 70f37fc с чистым
деревом. Публикация нового commit и hosted CI выполняются после review и проверок
в целевой среде. Независимая история удалённой main остаётся без изменений.

## Остающиеся ограничения

Полные ключи требуют памяти и увеличивают nested encodings. Storage in-memory;
durable import/crash recovery и production-scale характеристики не подтверждены.
Legacy aliases не дают collision-independent naming. Native Rust Hash/FNV,
portable identity, temporal availability, Source-wide revoke и reinstatement
сохраняют границы предыдущих пакетов. MDC-IMPL-010 — следующий identity-пакет.
VAIXLNS-Builder и STSA-MC не добавлены.
