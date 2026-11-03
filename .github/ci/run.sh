#!/usr/bin/env bash
# The same checks run in GitHub Actions and a local hosted environment.
set -euo pipefail

MDC_CI_ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
cd "$MDC_CI_ROOT"
MDC_CI_PROFILE=${1:-default}
case "$MDC_CI_PROFILE" in
  default) MDC_CI_FEATURES=() ;;
  std-alloc) MDC_CI_FEATURES=(--features std,alloc) ;;
  alloc) MDC_CI_FEATURES=(--no-default-features --features alloc) ;;
  *) printf 'Unknown CI profile: %s\n' "$MDC_CI_PROFILE" >&2; exit 2 ;;
esac
export MDC_CI_REPORT_DIR=${MDC_CI_REPORT_DIR:-"$MDC_CI_ROOT/ci-evidence/$MDC_CI_PROFILE"}
export MDC_CI_PROFILE
mkdir -p "$MDC_CI_REPORT_DIR"
MDC_CI_CURRENT_REPORT=$(python3 -B -c 'from scripts.e2e_report import DEFAULT_REPORT; print(DEFAULT_REPORT)')
MDC_CI_SAVED_REPORT=$(mktemp)
cp "$MDC_CI_CURRENT_REPORT" "$MDC_CI_SAVED_REPORT"
MDC_CI_STEP=metadata
finish() {
  MDC_CI_EXIT=$?
  set +e
  cp "$MDC_CI_SAVED_REPORT" "$MDC_CI_CURRENT_REPORT"
  MDC_CI_RESTORE_EXIT=$?
  rm -f "$MDC_CI_SAVED_REPORT"
  if (( MDC_CI_RESTORE_EXIT != 0 )); then MDC_CI_EXIT=1; fi
  python3 .github/ci/record.py "$MDC_CI_EXIT" "$MDC_CI_STEP"
  MDC_CI_RECORD_EXIT=$?
  if (( MDC_CI_RECORD_EXIT != 0 )); then MDC_CI_EXIT=1; fi
  exit "$MDC_CI_EXIT"
}
trap finish EXIT

run_check() {
  MDC_CI_STEP=$1
  shift
  "$@" 2>&1 | tee "$MDC_CI_REPORT_DIR/$MDC_CI_STEP.log"
}

python3 .github/ci/record.py running metadata
run_check format cargo fmt --all -- --check
run_check tests cargo test --locked --offline "${MDC_CI_FEATURES[@]}"
run_check library-check cargo check --locked --offline "${MDC_CI_FEATURES[@]}"
run_check clippy cargo clippy --locked --offline "${MDC_CI_FEATURES[@]}" --all-targets -- -D warnings

if [[ "$MDC_CI_PROFILE" == default ]]; then
  MDC_CI_STEP=no-alloc-rejection
  if cargo check --locked --offline --no-default-features >"$MDC_CI_REPORT_DIR/$MDC_CI_STEP.log" 2>&1; then
    printf 'Unexpected success without alloc\n' >&2
    exit 1
  fi
  if ! grep -Fq 'mdc_core requires allocation:' "$MDC_CI_REPORT_DIR/$MDC_CI_STEP.log"; then
    cat "$MDC_CI_REPORT_DIR/$MDC_CI_STEP.log"
    exit 1
  fi
fi

for MDC_CI_EXAMPLE in source_measurement relation_composition state_transition conformance invalid_artifacts identity_migration; do
  run_check "example-$MDC_CI_EXAMPLE" cargo run --quiet --locked --offline "${MDC_CI_FEATURES[@]}" --example "$MDC_CI_EXAMPLE"
done
for MDC_CI_LIFECYCLE_RUN in 1 2; do
  run_check "lifecycle-$MDC_CI_LIFECYCLE_RUN" cargo run --quiet --locked --offline "${MDC_CI_FEATURES[@]}" --example lifecycle
done
run_check lifecycle-determinism cmp "$MDC_CI_REPORT_DIR/lifecycle-1.log" "$MDC_CI_REPORT_DIR/lifecycle-2.log"
for MDC_CI_RETENTION_RUN in 1 2; do
  run_check "evaluation-retention-$MDC_CI_RETENTION_RUN" cargo run --quiet --locked --offline "${MDC_CI_FEATURES[@]}" --example evaluation_retention
done
run_check evaluation-retention-determinism cmp "$MDC_CI_REPORT_DIR/evaluation-retention-1.log" "$MDC_CI_REPORT_DIR/evaluation-retention-2.log"
for MDC_CI_PORTABLE_RUN in 1 2; do
  run_check "portable-$MDC_CI_PORTABLE_RUN" cargo run --quiet --locked --offline "${MDC_CI_FEATURES[@]}" --example portable_identity
done
run_check portable-determinism cmp "$MDC_CI_REPORT_DIR/portable-1.log" "$MDC_CI_REPORT_DIR/portable-2.log"
cargo run --quiet --locked --offline "${MDC_CI_FEATURES[@]}" --example identity_target > "$MDC_CI_REPORT_DIR/target.json"
run_check portable-check python3 -B scripts/check_portable_identity.py --log "$MDC_CI_REPORT_DIR/portable-1.log" --output "$MDC_CI_REPORT_DIR/portable-verification.json"
run_check portable-validator-tests env MDC_PORTABLE_LOG="$MDC_CI_REPORT_DIR/portable-1.log" python3 -B -m unittest discover -s tests -p test_portable_identity.py
run_check e2e python3 scripts/run_e2e.py "${MDC_CI_FEATURES[@]}" --output "$MDC_CI_REPORT_DIR/execution.json"
run_check evidence-check python3 scripts/check_gate_readiness.py --report "$MDC_CI_REPORT_DIR/execution.json"
# The legacy Python suite reads a fixed path. Supply fresh output for this run,
# then restore the historical tracked bundle on both success and failure.
cp "$MDC_CI_REPORT_DIR/execution.json" "$MDC_CI_CURRENT_REPORT"
run_check validator-tests python3 -m unittest discover -s tests -p test_e2e_report.py
MDC_CI_STEP=complete
printf 'MDC_CI_VERIFY=PASS profile=%s reports=%s\n' "$MDC_CI_PROFILE" "$MDC_CI_REPORT_DIR"
