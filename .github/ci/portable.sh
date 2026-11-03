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
MDC_CI_STEP=complete
