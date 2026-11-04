"""Independent checks for the frozen MODELED Binance replay, not a trading engine."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import struct
import subprocess
from check_portable_identity import check_target
from e2e_report import source_fingerprint

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "adapters/mdc_replay/tests/fixtures"
PREFIX = b"BINANCE-SPOT-1M-MODELED-001\0"


def verify_folder(folder):
    folder = Path(folder)
    control = json.loads((FIXTURES / "control-v1.json").read_text())
    sources = json.loads((FIXTURES / "sources.json").read_text())
    for record in sources["records"]:
        if hashlib.sha256((FIXTURES / record["sample_file"]).read_bytes()).hexdigest() != record["sample_sha256"]:
            raise ValueError("official sample fingerprint differs")
    report = json.loads((folder / "binance-replay-1.json").read_text())
    if report != control["expected_report"]:
        raise ValueError("replay differs from frozen modeled control")
    if (folder / "binance-replay-1.json").read_bytes() != (folder / "binance-replay-2.json").read_bytes():
        raise ValueError("replay reports differ between processes")
    for e in report["eligible"]:
        body = bytes.fromhex(e["evidence_basis_hex"])
        if not body.startswith(PREFIX) or hashlib.sha256(body).hexdigest() != e["evidence_id"]:
            raise ValueError("evidence digest does not bind its canonical bytes")
        symbol, unit, anchor, lag, open_us, event, available, *values = struct.unpack("<BBBQQQQqqqqqqQqq", body[len(PREFIX):])
        if (symbol, unit, anchor, lag) != (0, 0, 0, 100000):
            raise ValueError("wrong modeled profile")
        if event != open_us + 60000000 - 1000 or available != event + lag:
            raise ValueError("wrong inclusive close or availability model")
        if (event, available, values[:5]) != (e["event_us"], e["available_us"], e["ohlcv_e8"]):
            raise ValueError("payload and canonical evidence differ")
        if available > report["decision_us"]:
            raise ValueError("future evidence crossed decision cutoff")
    snapshot = (folder / "binance-replay-1.bin").read_bytes()
    if snapshot != (folder / "binance-replay-2.bin").read_bytes():
        raise ValueError("core snapshots differ between processes")
    digest = hashlib.sha256(snapshot).hexdigest()
    if len(snapshot) != control["core_snapshot_bytes"] or digest != report["core_snapshot_sha256"]:
        raise ValueError("core snapshot differs from frozen control")
    return {"processes": 2, "availability_quality": "modeled", "eligible_bars": len(report["eligible"]),
            "technical_confirmation": report["technical_confirmation"], "core_snapshot_sha256": digest,
            "report_sha256": hashlib.sha256((folder / "binance-replay-1.json").read_bytes()).hexdigest()}


def compare(root):
    rows, seen = [], set()
    fingerprint, files = source_fingerprint(ROOT)
    expected_commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    expected_run = os.environ.get("GITHUB_RUN_ID")
    ci_files = [ROOT / ".github/workflows/ci.yml", *sorted((ROOT / ".github/ci").glob("*"))]
    ci_hashes = {str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest() for p in ci_files if p.is_file()}
    for log in sorted(Path(root).rglob("binance-replay-1.json")):
        folder = log.parent
        record = verify_folder(folder)
        metadata = json.loads((folder / "metadata.json").read_text())
        target = json.loads((folder / "target.json").read_text())
        if (metadata["status"] != "PASS" or metadata["worktree_dirty"] is not False
                or metadata["implementation_source_sha256"] != fingerprint or metadata["source_files"] != files):
            raise ValueError("unverified, stale or dirty execution")
        pair = (metadata["rustc"].split()[1], metadata["profile"])
        if pair in seen:
            raise ValueError("duplicate replay matrix configuration")
        seen.add(pair)
        check_target(target, "default" if pair[1] == "binance-replay" else pair[1])
        if metadata["ci_file_sha256"] != ci_hashes:
            raise ValueError("CI configuration mismatch")
        if not expected_run or metadata["commit_sha"] != expected_commit or metadata["github_run_id"] != expected_run:
            raise ValueError("mixed or unidentified checkouts/runs")
        rows.append({"rust": pair[0], "profile": pair[1], "target": target, **record})
    profiles = {"binance-replay", "portable-i686-unknown-linux-gnu", "portable-s390x-unknown-linux-gnu"}
    if seen != {(rust, profile) for rust in ("1.85.1", "1.98.0") for profile in profiles}:
        raise ValueError("missing replay matrix configurations")
    return {"acceptance": "PASS", "commit_sha": expected_commit, "github_run_id": expected_run,
            "implementation_source_sha256": fingerprint, "configurations": rows}


def main():
    parser = argparse.ArgumentParser()
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--folder", type=Path)
    group.add_argument("--compare-root", type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    result = compare(args.compare_root) if args.compare_root else {"acceptance": "PASS", **verify_folder(args.folder)}
    args.output.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result))


if __name__ == "__main__":
    main()
