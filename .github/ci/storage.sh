#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.."
export MDC_CI_PROFILE=${MDC_CI_PROFILE:-storage-study}
export MDC_CI_REPORT_DIR=${MDC_CI_REPORT_DIR:-ci-evidence}
mkdir -p "$MDC_CI_REPORT_DIR"
MDC_CI_STEP=metadata
finish() {
  MDC_STORAGE_EXIT=$?
  trap - EXIT
  python3 -B .github/ci/record.py "$MDC_STORAGE_EXIT" "$MDC_CI_STEP"
  exit "$MDC_STORAGE_EXIT"
}
trap finish EXIT
python3 -B .github/ci/record.py running metadata
MDC_CI_STEP=dependencies
cargo fetch --locked --manifest-path adapters/mdc_replay/Cargo.toml
MDC_STORAGE_INPUT=()
case "${MDC_STORAGE_DATASET:-synthetic-1440}" in
  synthetic-1440) MDC_STORAGE_INPUT=(--synthetic-day --unit ms) ;;
  binance-2024|binance-2025)
    MDC_CI_STEP=explicit-acquisition
    MDC_STORAGE_YEAR=${MDC_STORAGE_DATASET#binance-}
    python3 -B scripts/fetch_capacity_source.py --day "$MDC_STORAGE_YEAR-01-01" --output "$MDC_CI_REPORT_DIR/acquisition"
    MDC_STORAGE_SHA=$(python3 -B -c 'import json,sys; print(json.load(open(sys.argv[1]))["csv_sha256"])' "$MDC_CI_REPORT_DIR/acquisition/acquisition.json")
    MDC_STORAGE_UNIT=ms
    if [[ "$MDC_STORAGE_YEAR" == 2025 ]]; then MDC_STORAGE_UNIT=us; fi
    MDC_STORAGE_INPUT=(--source "$MDC_CI_REPORT_DIR/acquisition/source.csv" --expected-source-sha256 "$MDC_STORAGE_SHA" --unit "$MDC_STORAGE_UNIT") ;;
  *) printf 'Unsupported dataset\n' >&2; exit 2 ;;
esac
MDC_CI_STEP=study
python3 -B scripts/run_storage_v2.py run "${MDC_STORAGE_INPUT[@]}" --points "${MDC_STORAGE_POINTS:-1,64,256,1440}" --repetitions "${MDC_STORAGE_REPETITIONS:-3}" --output "$MDC_CI_REPORT_DIR/study"
MDC_CI_STEP=complete
