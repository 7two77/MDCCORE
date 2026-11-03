"""Verify preserved history and the original integration tree without freezing future code."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
PLAN = ROOT / "docs/evidence/integration-plan-v0.1.json"


def git(*args):
    return subprocess.check_output(["git", *args], cwd=ROOT)


def tree(revision):
    result = {}
    for record in git("ls-tree", "-rz", revision).split(b"\0"):
        if not record:
            continue
        metadata, path = record.split(b"\t", 1)
        mode, kind, oid = metadata.decode().split()
        result[path.decode()] = (mode, kind, oid)
    return result


def fingerprint(revision, files):
    digest = hashlib.sha256()
    for name in sorted(files):
        parts = Path(name).parts
        included = name in {"Cargo.toml", "Cargo.lock"} or parts[0] in {"src", "examples", "tests", "scripts"}
        if included and "__pycache__" not in parts and not name.endswith(".pyc"):
            data = git("show", revision + ":" + name)
            value = hashlib.sha256(data).hexdigest()
            digest.update(name.encode() + b"\0" + value.encode() + b"\n")
    return digest.hexdigest()


def verify(revision="HEAD", candidate=False):
    plan = json.loads(PLAN.read_text())
    if plan["package"] != "MDC-INTEGRATION-001":
        raise ValueError("unexpected integration plan")
    resolved = git("rev-parse", "--verify", revision + "^{commit}").decode().strip()
    anchors = [plan["main_before"], *[m["commit"] for m in plan["milestones"]]]
    for commit in anchors:
        if subprocess.run(["git", "merge-base", "--is-ancestor", commit, resolved], cwd=ROOT).returncode:
            raise ValueError("required ancestor missing: " + commit)
    for before, after in zip(anchors[1:], anchors[2:]):
        if subprocess.run(["git", "merge-base", "--is-ancestor", before, after], cwd=ROOT).returncode:
            raise ValueError("implementation milestones are not a linear stack")
    roots = git("rev-list", "--max-parents=0", resolved).decode().splitlines()
    if not set(plan["expected_roots"]).issubset(roots):
        raise ValueError("original roots were lost")
    bridges = [line.split()[0] for line in git("log", "--merges", "--format=%H %P", resolved).decode().splitlines()
               if line.split()[1:] == plan["bridge_parents"]]
    if not bridges:
        raise ValueError("history-preserving integration merge missing")
    bridge = bridges[0]
    # Candidate branches preserve the exact 010 core. Future main/features only
    # verify the immutable bridge tree, permitting subsequent semantic work.
    selected = resolved if candidate else bridge
    original = tree(plan["implementation_tip"])
    actual = tree(selected)
    allowed = {"README.md", ".github/workflows/ci.yml"}
    if set(plan["allowed_changes_to_implementation"]) != allowed:
        raise ValueError("unexpected implementation exceptions")
    if len(original) != plan["implementation_files"]:
        raise ValueError("implementation file inventory mismatch")
    for name, entry in original.items():
        if name not in allowed and actual.get(name) != entry:
            raise ValueError("implementation file changed: " + name)
    base_tree = tree(plan["main_before"])
    for item in plan["original_main_files"]:
        original_entry = base_tree[item["original_path"]]
        if actual.get(item["preserved_path"]) != original_entry:
            raise ValueError("original main file lost or changed: " + item["original_path"])
        content = git("show", selected + ":" + item["preserved_path"])
        if hashlib.sha256(content).hexdigest() != item["sha256"]:
            raise ValueError("original main file checksum mismatch")
    source = fingerprint(selected, actual)
    if source != plan["implementation_source_sha256"]:
        raise ValueError("core source fingerprint changed")
    return {"acceptance": "PASS", "revision": resolved,
            "mode": "candidate" if candidate else "history",
            "bridge_commit": bridge, "verified_tree_commit": selected,
            "main_before": plan["main_before"], "implementation_tip": plan["implementation_tip"],
            "preserved_milestones": plan["milestones"], "roots": roots,
            "protected_implementation_files": len(original) - len(allowed),
            "original_main_files_preserved": len(plan["original_main_files"]),
            "implementation_source_sha256": source}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--revision", default="HEAD")
    parser.add_argument("--candidate", action="store_true")
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    try:
        result = verify(args.revision, args.candidate)
    except (ValueError, KeyError, subprocess.CalledProcessError) as error:
        result = {"acceptance": "FAIL", "error": str(error)}
    if args.output:
        args.output.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result))
    return 0 if result["acceptance"] == "PASS" else 1


if __name__ == "__main__":
    sys.exit(main())
