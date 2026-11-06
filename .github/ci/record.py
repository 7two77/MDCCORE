"""Bind CI logs to the core fingerprint, CI configuration, and checkout commit."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
from e2e_report import source_fingerprint  # noqa: E402
from ci_metadata import ci_fingerprints  # noqa: E402


def output(command):
    return subprocess.check_output(command, cwd=ROOT, text=True).strip()


def main():
    status, step = sys.argv[1:]
    commit = None
    worktree_dirty = None
    try:
        if Path(output(["git", "rev-parse", "--show-toplevel"])).resolve() == ROOT:
            commit = output(["git", "rev-parse", "HEAD"])
            worktree_dirty = bool(output(["git", "--no-optional-locks", "status", "--porcelain=v1", "-uall"]))
    except subprocess.CalledProcessError:
        pass
    fingerprint, files = source_fingerprint(ROOT)
    data = {
        "kind": "ci_execution", "schema_version": 1,
        "status": "RUNNING" if status == "running" else "PASS" if status == "0" else "FAIL",
        "exit_code": None if status == "running" else int(status), "last_step": step,
        "profile": os.environ["MDC_CI_PROFILE"], "commit_sha": commit,
        "worktree_dirty": worktree_dirty,
        "github_run_id": os.environ.get("GITHUB_RUN_ID"),
        "rustc": output(["rustc", "-vV"]), "cargo": output(["cargo", "--version"]),
        "python": sys.version, "implementation_source_sha256": fingerprint,
        "source_files": files,
        "ci_file_sha256": ci_fingerprints(ROOT),
    }
    target = Path(os.environ["MDC_CI_REPORT_DIR"]) / "metadata.json"
    target.write_text(json.dumps(data, indent=2) + "\n")


if __name__ == "__main__":
    main()
