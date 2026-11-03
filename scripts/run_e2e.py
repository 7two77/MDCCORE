#!/usr/bin/env python3
"""Execute the Rust fixture twice and independently validate its evidence."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys

from e2e_report import DEFAULT_REPORT, source_fingerprint, validate_bundle, validate_execution


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", default=DEFAULT_REPORT)
    parser.add_argument("--no-default-features", action="store_true")
    parser.add_argument("--features", default="")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    fingerprint, files = source_fingerprint(root)
    command = ["cargo", "run", "--quiet", "--locked", "--offline", "--example", "mdc_e2e"]
    if args.no_default_features:
        command.append("--no-default-features")
    if args.features:
        command.extend(["--features", args.features])
    rustc = subprocess.check_output(["rustc", "-vV"], text=True, cwd=root)
    cargo = subprocess.check_output(["cargo", "--version"], text=True, cwd=root).strip()
    target = next(line.removeprefix("host: ") for line in rustc.splitlines() if line.startswith("host: "))
    bundle = {"schema_version": 1, "fixture_id": "MDC-E2E-CONFORMANCE-001",
              "profile": "MDC-SCALAR-001", "kind": "execution_evidence",
              "acceptance": "BLOCKED", "harness_status": "BLOCKED",
              "metadata": {"implementation_source_sha256": fingerprint, "source_files": files,
                           "rustc": rustc, "cargo": cargo, "target_triple": target,
                           "features": {"default": not args.no_default_features, "explicit": args.features},
                           "command": command}, "processes": [], "validation_errors": []}
    for _ in range(2):
        process = subprocess.Popen(command, cwd=root, text=True, stdout=subprocess.PIPE,
                                   stderr=subprocess.PIPE, env=os.environ.copy())
        timed_out = False
        try:
            stdout, stderr = process.communicate(timeout=180)
        except subprocess.TimeoutExpired:
            timed_out = True
            process.kill()
            stdout, stderr = process.communicate()
        try:
            report = json.loads(stdout)
        except json.JSONDecodeError:
            report = {}
        entry = {"launcher_pid": process.pid, "exit_code": process.returncode,
                 "report": report, "stderr": stderr, "timed_out": timed_out}
        if not report:
            entry["unparsed_stdout"] = stdout
        bundle["processes"].append(entry)
        bundle["validation_errors"].extend(validate_execution(report))
        if process.returncode:
            bundle["validation_errors"].append("fixture process failed")
    if not bundle["validation_errors"]:
        bundle["acceptance"] = "PASS"
        bundle["harness_status"] = "PASS"
    errors = validate_bundle(bundle, current_source=source_fingerprint(root)[0])
    if errors:
        bundle["validation_errors"].extend(errors)
        bundle["acceptance"] = "FAIL"
        bundle["harness_status"] = "FAIL"
    output = Path(args.output)
    if not output.is_absolute():
        output = root / output
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(bundle, indent=2) + "\n")
    print(json.dumps({"acceptance": bundle["acceptance"], "report": str(output),
                      "cases_per_process": [len(p["report"].get("cases", [])) for p in bundle["processes"]],
                      "validation_errors": bundle["validation_errors"]}))
    return 0 if bundle["acceptance"] == "PASS" else 1


if __name__ == "__main__":
    sys.exit(main())
