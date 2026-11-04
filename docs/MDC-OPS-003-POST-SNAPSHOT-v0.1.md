# MDC-OPS-003 — Post-snapshot checkpoint and recovery preparation

2026-11-03. PR #5 объединён владельцем проекта. Main:
`27fbc0a7c83378fdcc303732cc76cdd8f81fe40c`; родители — main после PR #4
`ea22cda835ee1b9d677f1976938cff2c0a69696e` и проверенная ветка 011
`21a61e7df653697d510cf8d8fac1bf809d066b5e`.

Tree `552dc38dcd5edeffe9427a63ef6df89b7827c411` совпадает с исходным 011
`e5c31fb` и проверенным merge candidate `60a2857`. История не переписана.
Source fingerprint:
`570f62581a6c3e64dda6b293201a585f25590c7743d50d130eee77df15cdb3b4`.

[Pre-merge integration CI](https://github.com/7two77/MDCCORE/actions/runs/41713976610)
прошёл 12/12 jobs: 209 Rust + 25 Python в каждой native configuration;
7 portable + 15 snapshot tests в каждой cross configuration; exact frozen
snapshot bytes на всех десяти configurations, history guard и artifacts.
[Post-merge CI 41716728041](https://github.com/7two77/MDCCORE/actions/runs/41716728041)
также прошёл 12/12 jobs на main 27fbc0a с теми же counts/controls.
[Execution receipt](evidence/post-merge-main-27fbc0a.json) содержит jobs,
проверенные log counts, оба comparison results, history guard и 12 artifacts.

Следующий этап — [MDC-RECOVERY-001 contract draft](MDC-RECOVERY-CONTRACT-v0.1-DRAFT.md).
Владелец предложил local snapshot + WAL и hybrid durability: streaming events
буферизуются, snapshot/critical decisions синхронизируются. Это заменяет прежний
выбор между только per-operation sync и только explicit checkpoint.

Draft разделяет AcceptedVolatile и DurableThrough, требует durable полного
префикса перед публикацией решения, описывает rotation и recovery tail policy.
Для первого engineering profile зафиксированы Linux/local filesystem/single writer,
[wire v1](MDC-WAL-WIRE-CONTRACT-v1.md), SHA-256 frame chain, flock и отдельный
HEAD certificate. N/timeout задаются конфигурацией; секунда/N остаются примерами,
не утверждёнными SLA. [Framing vectors](evidence/wal-framing-v1.json) не являются
execution evidence реального recovery.

Runtime WAL/recovery отсутствует. Wire/commit contract подготовлен к реализации; следующий результат — host adapter
и crash-point acceptance R01…R12. Temporal profile
и внешний trading state остаются отдельными требованиями.
