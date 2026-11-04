#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.."
export MDC_CI_PROFILE=binance-replay
export MDC_CI_REPORT_DIR=${MDC_CI_REPORT_DIR:-ci-evidence/replay}
mkdir -p "$MDC_CI_REPORT_DIR"
MDC_CI_STEP=metadata
finish() {
  MDC_REPLAY_EXIT=$?
  trap - EXIT
  python3 -B .github/ci/record.py "$MDC_REPLAY_EXIT" "$MDC_CI_STEP"
  exit "$MDC_REPLAY_EXIT"
}
trap finish EXIT
python3 -B .github/ci/record.py running metadata
run_check() {
  MDC_CI_STEP=$1
  shift
  "$@" 2>&1 | tee "$MDC_CI_REPORT_DIR/$MDC_CI_STEP.log"
}
run_check dependencies cargo fetch --locked --manifest-path adapters/mdc_replay/Cargo.toml
run_check format cargo fmt --manifest-path adapters/mdc_replay/Cargo.toml -- --check
run_check tests cargo test --locked --offline --manifest-path adapters/mdc_replay/Cargo.toml
run_check clippy cargo clippy --locked --offline --manifest-path adapters/mdc_replay/Cargo.toml --all-targets -- -D warnings
MDC_CI_STEP=replay-execution
for MDC_REPLAY_RUN in 1 2; do
  cargo run --quiet --locked --offline --manifest-path adapters/mdc_replay/Cargo.toml --bin mdc-binance-replay -- decision \
    adapters/mdc_replay/tests/fixtures/BTCUSDT-1m-2024-01-01-first3.csv BTCUSDT ms 100 close 1704067320049000 \
    "$MDC_CI_REPORT_DIR/binance-replay-$MDC_REPLAY_RUN.json" "$MDC_CI_REPORT_DIR/binance-replay-$MDC_REPLAY_RUN.bin"
done
cargo run --quiet --locked --offline --example identity_target > "$MDC_CI_REPORT_DIR/target.json"
run_check replay-validation python3 -B scripts/check_binance_replay.py --folder "$MDC_CI_REPORT_DIR" --output "$MDC_CI_REPORT_DIR/binance-verification.json"
export MDC_BINANCE_REPORT_DIR="$MDC_CI_REPORT_DIR"
run_check validator-tests python3 -B -m unittest discover -s tests -p test_binance_replay.py
MDC_CI_STEP=complete
printf 'MDC_BINANCE_REPLAY_VERIFY=PASS reports=%s\n' "$MDC_CI_REPORT_DIR"
