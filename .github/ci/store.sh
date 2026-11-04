#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.."
export MDC_CI_PROFILE=linux-store
export MDC_CI_REPORT_DIR=${MDC_CI_REPORT_DIR:-ci-evidence/store}
mkdir -p "$MDC_CI_REPORT_DIR"
MDC_CI_STEP=metadata
finish() {
  MDC_STORE_EXIT=$?
  trap - EXIT
  python3 -B .github/ci/record.py "$MDC_STORE_EXIT" "$MDC_CI_STEP"
  exit "$MDC_STORE_EXIT"
}
trap finish EXIT
python3 -B .github/ci/record.py running metadata
run_check() {
  MDC_CI_STEP=$1
  shift
  "$@" 2>&1 | tee "$MDC_CI_REPORT_DIR/$MDC_CI_STEP.log"
}
# New dependencies belong only to the independent host crate. Runtime checks
# use a locked offline resolution after this explicit download step.
run_check dependencies cargo fetch --locked --manifest-path adapters/mdc_store/Cargo.toml
run_check format cargo fmt --manifest-path adapters/mdc_store/Cargo.toml -- --check
run_check tests-default cargo test --locked --offline --manifest-path adapters/mdc_store/Cargo.toml
run_check tests-fault-injection cargo test --locked --offline --manifest-path adapters/mdc_store/Cargo.toml --features fault-injection
run_check clippy-default cargo clippy --locked --offline --manifest-path adapters/mdc_store/Cargo.toml --all-targets -- -D warnings
run_check clippy-fault-injection cargo clippy --locked --offline --manifest-path adapters/mdc_store/Cargo.toml --features fault-injection --all-targets -- -D warnings
run_check production-build cargo build --locked --offline --manifest-path adapters/mdc_store/Cargo.toml --bin mdc-recovery
run_check fresh-process-recovery python3 -B scripts/run_store_verification.py --output "$MDC_CI_REPORT_DIR/recovery-execution.json"
MDC_CI_STEP=complete
printf 'MDC_STORE_VERIFY=PASS reports=%s\n' "$MDC_CI_REPORT_DIR"
