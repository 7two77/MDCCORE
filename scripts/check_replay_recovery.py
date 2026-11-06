"""Require real native recovery artifacts from both declared Rust toolchains."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
from check_portable_identity import check_target
from e2e_report import source_fingerprint
from replay_run_report import ROOT, verify_folder


def compare(root):
    fingerprint, files = source_fingerprint(ROOT)
    commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    run = os.environ.get("GITHUB_RUN_ID")
    ci_files = [ROOT / ".github/workflows/ci.yml", *sorted((ROOT / ".github/ci").glob("*"))]
    hashes = {str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest() for p in ci_files if p.is_file()}
    rows, seen = [], set()
    for path in sorted(Path(root).rglob("replay-recovery-execution.json")):
        folder = path.parent
        result = verify_folder(folder)
        receipt = json.loads(path.read_text())
        metadata = json.loads((folder.parent / "metadata.json").read_text())
        target = json.loads((folder.parent / "target.json").read_text())
        if (metadata["status"] != "PASS" or metadata["worktree_dirty"] is not False
                or metadata["source_files"] != files or metadata["implementation_source_sha256"] != fingerprint
                or metadata["ci_file_sha256"] != hashes or metadata["commit_sha"] != commit
                or not run or metadata["github_run_id"] != run or metadata["profile"] != "binance-replay"):
            raise ValueError("unverified/dirty/stale/mixed recovery execution")
        if receipt["implementation_source_sha256"] != fingerprint or any(receipt[k] != v for k, v in result.items()):
            raise ValueError("recovery receipt does not match real files")
        check_target(target, "default")
        rust = metadata["rustc"].split()[1]
        if rust in seen:
            raise ValueError("duplicate native toolchain")
        seen.add(rust)
        rows.append({"rust": rust, "target": target, "result": result})
    if seen != {"1.85.1", "1.98.0"} or rows[0]["result"] != rows[1]["result"]:
        raise ValueError("missing native toolchain or recovery bytes differ")
    return {"acceptance": "PASS", "commit_sha": commit, "github_run_id": run,
            "implementation_source_sha256": fingerprint, "configurations": rows}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--compare-root", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    result = compare(args.compare_root)
    args.output.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result))


if __name__ == "__main__":
    main()
