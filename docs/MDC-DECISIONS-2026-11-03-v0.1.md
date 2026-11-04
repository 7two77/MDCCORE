# MDC Core — Consumer and recovery decisions v0.1

2026-11-03. Основание: прямой ответ владельца проекта в текущей сессии.

1. Первый потребитель — исторический replay с дальнейшим развитием в длительно
   работающий сервис/торговый adapter.
2. После restart требуется точное восстановление graph, Evaluation, invalidation
   и retained audit. Выбран следующий пакет MDC-IMPL-011 — snapshot roundtrip.

Инженерное решение для первого ограниченного профиля: opt-in SnapshotSession
начинается с frozen Registry и пустого Pipeline, сохраняет полный sequence всех
mutating requests/outcomes и exact state manifest. Restore строит новый Pipeline
производственными API в исходном порядке и проверяет bytes каждого результата и
полного состояния. Mutable escape и restore против финального графа не допускаются.
Произвольный ранее созданный незаписанный Pipeline не экспортируется этим профилем.
Это решение реализации, а не новая редакция STP-MDC от имени владельца спецификации.

Будущие обязательства: availability contract до заявления о causal historical
replay, durable adapter/journal до заявления о сохранности после аварийной записи.
Snapshot сохраняет core state; позиции/ордера/external trading state будущего host
adapter требуют собственного контракта. VAIXLNS-Builder и STSA-MC не подключаются.

Открыты эксплуатационные детали первого consumer: data source/schema, desired
State/Transition results, time basis, graph scale, storage platform и delivery channel.
Они не препятствуют ограниченному exact core roundtrip в 011.

2026-11-04: владелец предложил первый market input: Binance Spot либо Deribit,
BTCUSDT/ETHUSDT, 1m OHLCV, CSV/Parquet, event=close_time и модель задержки.
Для MDC-IMPL-014 выбран ограниченный Binance Spot/BTCUSDT/1m/CSV профиль.
Задержка явно MODELED; после 2025-01-01 официальные Spot timestamps — us,
раньше — ms. Решения и границы зафиксированы в
[контракте адаптера](MDC-BINANCE-REPLAY-CONTRACT-v0.1.md).
