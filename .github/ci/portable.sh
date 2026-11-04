#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.."
MDC_PORTABLE_TARGET=${1:?target required}
export MDC_CI_PROFILE="portable-$MDC_PORTABLE_TARGET"
export MDC_CI_REPORT_DIR=${MDC_CI_REPORT_DIR:-"ci-evidence"}
mkdir -p "$MDC_CI_REPORT_DIR"
MDC_CI_STEP=metadata
finish() {
  MDC_PORTABLE_EXIT=$?
  trap - EXIT
  python3 -B .github/ci/record.py "$MDC_PORTABLE_EXIT" "$MDC_CI_STEP"
  exit "$MDC_PORTABLE_EXIT"
}
trap finish EXIT
python3 -B .github/ci/record.py running metadata
MDC_CI_STEP=portable-tests
cargo test --locked --offline --no-default-features --features alloc --target "$MDC_PORTABLE_TARGET" --test portable_identity 2>&1 | tee "$MDC_CI_REPORT_DIR/tests.log"
MDC_CI_STEP=portable-execution
for MDC_PORTABLE_RUN in 1 2; do
  cargo run --quiet --locked --offline --no-default-features --features alloc --target "$MDC_PORTABLE_TARGET" --example portable_identity > "$MDC_CI_REPORT_DIR/portable-$MDC_PORTABLE_RUN.log"
done
cmp "$MDC_CI_REPORT_DIR/portable-1.log" "$MDC_CI_REPORT_DIR/portable-2.log"
cargo run --quiet --locked --offline --no-default-features --features alloc --target "$MDC_PORTABLE_TARGET" --example identity_target > "$MDC_CI_REPORT_DIR/target.json"
python3 -B scripts/check_portable_identity.py --log "$MDC_CI_REPORT_DIR/portable-1.log" --output "$MDC_CI_REPORT_DIR/portable-verification.json"
MDC_CI_STEP=snapshot-tests
cargo test --locked --offline --no-default-features --features alloc --target "$MDC_PORTABLE_TARGET" --test snapshot_roundtrip 2>&1 | tee "$MDC_CI_REPORT_DIR/snapshot-tests.log"
cargo run --quiet --locked --offline --no-default-features --features alloc --target "$MDC_PORTABLE_TARGET" --example snapshot_roundtrip -- produce "$MDC_CI_REPORT_DIR/snapshot.bin" > "$MDC_CI_REPORT_DIR/snapshot-producer.log"
for MDC_SNAPSHOT_RUN in 1 2; do
  cargo run --quiet --locked --offline --no-default-features --features alloc --target "$MDC_PORTABLE_TARGET" --example snapshot_roundtrip -- consume "$MDC_CI_REPORT_DIR/snapshot.bin" > "$MDC_CI_REPORT_DIR/snapshot-consumer-$MDC_SNAPSHOT_RUN.log"
  cmp "$MDC_CI_REPORT_DIR/snapshot-producer.log" "$MDC_CI_REPORT_DIR/snapshot-consumer-$MDC_SNAPSHOT_RUN.log"
done
python3 -B scripts/check_snapshot_roundtrip.py --folder "$MDC_CI_REPORT_DIR" --output "$MDC_CI_REPORT_DIR/snapshot-verification.json"
MDC_CI_STEP=wal-bridge-tests
cargo test --locked --offline --no-default-features --features alloc --target "$MDC_PORTABLE_TARGET" --test wal_bridge 2>&1 | tee "$MDC_CI_REPORT_DIR/wal-bridge-tests.log"
MDC_CI_STEP=binance-replay-tests
cargo fetch --locked --manifest-path adapters/mdc_replay/Cargo.toml --target "$MDC_PORTABLE_TARGET"
cargo test --locked --offline --manifest-path adapters/mdc_replay/Cargo.toml --target "$MDC_PORTABLE_TARGET" --test replay 2>&1 | tee "$MDC_CI_REPORT_DIR/binance-tests.log"
MDC_CI_STEP=binance-replay-execution
for MDC_REPLAY_RUN in 1 2; do
  cargo run --quiet --locked --offline --manifest-path adapters/mdc_replay/Cargo.toml --target "$MDC_PORTABLE_TARGET" --bin mdc-binance-replay -- decision \
    adapters/mdc_replay/tests/fixtures/BTCUSDT-1m-2024-01-01-first3.csv BTCUSDT ms 100 close 1704067320049000 \
    "$MDC_CI_REPORT_DIR/binance-replay-$MDC_REPLAY_RUN.json" "$MDC_CI_REPORT_DIR/binance-replay-$MDC_REPLAY_RUN.bin"
done
python3 -B scripts/check_binance_replay.py --folder "$MDC_CI_REPORT_DIR" --output "$MDC_CI_REPORT_DIR/binance-verification.json"
MDC_CI_STEP=complete
