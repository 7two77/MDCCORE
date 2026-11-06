#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.."
if [[ "${MDC_CAPACITY_STORAGE_V2:-false}" == true ]]; then
  if [[ "${MDC_CAPACITY_COMPARE_016:-false}" == true ]]; then
    printf 'Select exactly one comparison mode\n' >&2; exit 2
  fi
  export MDC_STORAGE_DATASET=${MDC_CAPACITY_DATASET:-synthetic-1440}
  export MDC_STORAGE_POINTS=${MDC_CAPACITY_POINTS:-1,64,256,1440}
  exec bash .github/ci/storage.sh
fi
export MDC_CI_PROFILE=capacity-study
export MDC_CI_REPORT_DIR=${MDC_CI_REPORT_DIR:-ci-evidence}
mkdir -p "$MDC_CI_REPORT_DIR"
MDC_CI_STEP=metadata
finish() {
  MDC_CAPACITY_EXIT=$?
  trap - EXIT
  python3 -B .github/ci/record.py "$MDC_CAPACITY_EXIT" "$MDC_CI_STEP"
  exit "$MDC_CAPACITY_EXIT"
}
trap finish EXIT
python3 -B .github/ci/record.py running metadata
MDC_CI_STEP=dependencies
cargo fetch --locked --manifest-path adapters/mdc_replay/Cargo.toml
MDC_CAPACITY_INPUT=()
case "${MDC_CAPACITY_DATASET:-synthetic-1440}" in
  synthetic-1440) MDC_CAPACITY_INPUT=(--synthetic-day --source-label synthetic-study --unit ms) ;;
  binance-2024|binance-2025)
    MDC_CI_STEP=explicit-acquisition
    MDC_CAPACITY_YEAR=${MDC_CAPACITY_DATASET#binance-}
    python3 -B scripts/fetch_capacity_source.py --day "$MDC_CAPACITY_YEAR-01-01" --output "$MDC_CI_REPORT_DIR/acquisition"
    MDC_CAPACITY_SOURCE_SHA=$(python3 -B -c 'import json,sys; print(json.load(open(sys.argv[1]))["csv_sha256"])' "$MDC_CI_REPORT_DIR/acquisition/acquisition.json")
    MDC_CAPACITY_UNIT=ms
    if [[ "$MDC_CAPACITY_YEAR" == 2025 ]]; then MDC_CAPACITY_UNIT=us; fi
    MDC_CAPACITY_INPUT=(--source "$MDC_CI_REPORT_DIR/acquisition/source.csv" --expected-source-sha256 "$MDC_CAPACITY_SOURCE_SHA" --source-label "pinned-binance-$MDC_CAPACITY_YEAR" --unit "$MDC_CAPACITY_UNIT")
    ;;
  *) printf 'Unsupported capacity dataset\n' >&2; exit 2 ;;
esac
MDC_CI_STEP=study
if [[ "${MDC_CAPACITY_COMPARE_016:-false}" == true ]]; then
  python3 -B scripts/exact_work.py run "${MDC_CAPACITY_INPUT[@]}" --points "${MDC_CAPACITY_POINTS:-1,16,32,64,128,256,1440}" --repetitions "${MDC_CAPACITY_REPETITIONS:-3}" --output "$MDC_CI_REPORT_DIR/exact-work"
else
  python3 -B scripts/run_capacity.py "${MDC_CAPACITY_INPUT[@]}" --points "${MDC_CAPACITY_POINTS:-1,16,32,64,128,256,1440}" --repetitions "${MDC_CAPACITY_REPETITIONS:-3}" --output "$MDC_CI_REPORT_DIR/study"
fi
MDC_CI_STEP=complete
