# MDC-IMPL-010 — Portable identity

2026-11-03. База: `d33bb979c0b8b6a69126fd79544d3f43e5df9d3b` (MDC-IMPL-009).
Новый profile — opt-in; root package и вложенная исходная копия сохраняются.

## Реализация

* [Контракт](MDC-PORTABLE-IDENTITY-CONTRACT-v0.1.md) фиксирует preimage, FNV1a64,
  version, codec, length, tags, migration и границы обещаний.
* `IdentityProtocol`, `ProfiledIdentity`, `portable_v1_preimage()` и reference codec.
* `Pipeline::with_identity_protocol()` и immutable `identity_protocol()`.
  Профиль применяется до insertion всех O/M/R/C/S/T/E, invalid artifacts и events;
  IdentityCoherent использует этот же профиль. Прежний `new()` остаётся legacy.
* Explicit profiled snapshot capture и проверяемый legacy snapshot migration.
  Общий importer старого persistent graph не реализован; пример строит новый граф
  из тех же definitions/requests и выводит correspondence по semantic roles.
* Protocol-aware Evaluation lookup сохраняет collision retention 009. Приватный
  collision seam остаётся только под cfg(test), production hash не подменяется.

## Проверки и evidence

Local matrix Rust 1.85.1/1.98.0 × default/std-alloc/alloc:
**176 Rust tests**, **17 Python validator tests** в каждой конфигурации,
Format, Clippy, checks, examples и прежний E2E 23/23 в двух процессах — PASS.
Восемь новых Rust tests: семь public protocol/migration/graph cases и один
private portable collision + lifecycle scenario.

Шесть независимых basis/preimage/digest vectors лежат в
`tests/fixtures/portable-identity-v1.tsv`. Python validator независимо вычисляет
FNV и проверяет каждый digest всего portable graph. Его семь negative controls
отклоняют неполное исполнение, другой protocol, повреждённые/лишние records и lineage.
Frozen portable graph/lineage SHA-256:
`b8466289ccf2bb4c528df7a9c6c9931762b301e9bd86b158f0223e845a0cd160`.

Текущие [local execution records](evidence/impl-010-local-verification.json)
фиксируют dirty preparation checkout и native target, не hosted или cross runs.
В CI добавлены i686 и s390x/QEMU для обеих версий Rust и отдельный comparison
всех десяти исполнений. Hosted verification на момент подготовки NOT_EXECUTED;
окончательный receipt должен содержать actual commit/run/jobs/artifact IDs.

[Source manifest и compatibility](evidence/impl-010-source-manifest.json)
связывают implementation fingerprint с изменениями относительно 009.
Прежние документы/evidence сохранены как история. Старый E2E graph/lineage и
lifecycle event проверяются на совпадение с 009 во всех local configurations.
Source fingerprint включает новые tests/examples/scripts, поэтому новые E2E
bundles сохранены под именами impl-010; старые bundles не перезаписываются.

## Использование

```rust
use mdc_core::identity::IdentityProtocol;
use mdc_core::pipeline::Pipeline;
use mdc_core::contract::registry::Registry;
let pipeline = Pipeline::with_identity_protocol(Registry::new(), IdentityProtocol::PortableV1);
assert_eq!(pipeline.identity_protocol(), IdentityProtocol::PortableV1);
```

Этот пример выбирает protocol; реальный admission по-прежнему требует регистрации
contracts. Исполнимые примеры: `portable_identity`, `identity_migration`, `identity_target`.
Обычная проверка: `bash .github/ci/run.sh default` (аналогично std-alloc и alloc).
Cross checks требуют target std, compiler/linker и runtime из CI workflow.

## Совместимость и ограничения

Публичные APIs добавлены; signatures existing constructors и default algorithm
сохранены. Enum wire discriminants сделаны явными без изменения кодов. Без alloc
сохраняется прежняя явная ошибка конфигурации. Dependencies/lockfiles не меняются.
Нельзя вставить legacy input references в portable graph как способ миграции.
Portable digest поверх сохранённого legacy body переносит только эти bytes.
Digest u64 не является доказательством semantic equivalence или collision freedom.
Все подтверждения относятся к declared scalar graph и заявленным target/toolchains.

Публикация выполняется в отдельной ветке `mdc/impl-010-portable-identity`, parent 009.
Ветки 009/008/main и Codespaces checkout не переписываются. После hosted PASS
Codespaces синхронизируется обычным fetch/switch; archive installer не требуется.
