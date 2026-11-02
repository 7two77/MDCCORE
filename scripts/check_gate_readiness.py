#!/usr/bin/env python3
"""Check current execution evidence, or explicitly inspect historical preparation."""
import argparse
import json
from pathlib import Path
import sys
from e2e_report import CASE_IDS, source_fingerprint, validate_bundle


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest-only", action="store_true")
    parser.add_argument("--report", default="docs/evidence/mdc-e2e-conformance-001.json")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    report = Path(args.report)
    if not report.is_absolute():
        report = root / report
    if report.exists() and not args.manifest_only:
        try:
            bundle = json.loads(report.read_text())
            errors = validate_bundle(bundle, source_fingerprint(root)[0])
        except (ValueError, OSError) as error:
            errors = [str(error)]
        print(json.dumps({"acceptance": "FAIL" if errors else "PASS", "validation_errors": errors}))
        return 1 if errors else 0
    manifest = json.loads((root / "tests/fixtures/mdc_e2e_gate_v1.json").read_text())
    cases = manifest["assertions"] + manifest["negative_cases"]
    valid = (len(cases) == 23 and {c["id"] for c in cases} == CASE_IDS
             and all(c["mandatory"] and c["harness_status"] == "NOT_EXECUTED" for c in cases)
             and manifest["acceptance"] == "BLOCKED" and bool(manifest["blockers"]))
    print(json.dumps({"historical_manifest_valid": valid, "gate_status": "NOT_EXECUTED",
                      "acceptance": "BLOCKED", "blockers": manifest["blockers"]}))
    return 2 if valid else 1


if __name__ == "__main__":
    sys.exit(main())
