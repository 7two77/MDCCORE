# MDC Core — Contract-to-Implementation Matrix v0.6

MDC-OPS-002, 2026-11-03. Checkpoint main 6177277. Scope — admitted scalar profile
и объявленные fixtures/targets. [v0.5](MDC-CONTRACT-TO-IMPLEMENTATION-MATRIX-v0.5.md)
сохранена как снимок подготовки интеграции.

| Область | Статус | Evidence / граница |
|---|---|---|
| MDC scalar O/M/R/C/S/T и Conformance | IMPLEMENTED | Versioned contracts и реальные операции ограниченного профиля |
| Typed invalid artifacts / Evaluation retention | IMPLEMENTED | Immutable snapshots, full keys, pinned aliases, positive/negative tests |
| Lifecycle | IMPLEMENTED | Monotonic invalidation, propagation, retained history; reinstatement не добавлен |
| Portable identity | IMPLEMENTED | Opt-in profile, frozen preimage/codec/hash, vectors и cross-target comparison |
| Применение integration к main | IMPLEMENTED | PR #3 merged; main 6177277, оба roots и исходные milestones сохранены |
| Post-merge CI | IMPLEMENTED | Run 41626673525, 12/12 jobs PASS: native 176 Rust + 21 Python × 6, four cross-target jobs, comparison/history |
| Codespaces main | IMPLEMENTED | User execution: HEAD=origin/main=6177277, history/gate PASS, clean worktree, upstream main |
| Первый эксплуатационный consumer | UNVERIFIED | Есть examples; конкретные input/output, режим и acceptance ещё не выбраны |
| Exact snapshot export/import | MISSING | Нет общего typed graph/Registry restore API; byte rebinding и fixture replay имеют другую область |
| Durable recovery | MISSING | In-memory lifecycle journal не является полным durable log; commit/recovery policy не реализована |
| Temporal availability | MISSING | Нет available_at/decision_at и causal eligibility profile |
| Production resource envelope | UNVERIFIED | Нет подтверждённых scale/budget/throughput границ; full keys и lineage могут расти |
| General graph migration | PARTIAL | Explicit replay fixture и captured-byte rebinding; нет общего persisted importer |
| Full lifecycle | PARTIAL | Нет reinstatement, Source-wide revoke и полного creation-time replay |
| Global u64 uniqueness | UNVERIFIED | FNV64 не инъективен; guards/full canonical keys остаются необходимы |
| Arbitrary schema/float normalization | MISSING | Scalar profile и legacy Debug Observation не расширены |
| Public release readiness | PARTIAL | Package 0.1.0 и CI есть; consumer/API/license/delivery decisions не завершены |

[Post-merge evidence](evidence/post-merge-main-6177277.json).
[Следующие решения и сценарии](MDC-OPS-002-NEXT-STAGE-v0.1.md).
Snapshot, durable recovery и temporal profile — отдельные contracts; статусы
не означают нарушение старых MDC contracts или универсальную production certification.
