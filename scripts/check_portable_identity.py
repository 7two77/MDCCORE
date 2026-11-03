"""Independent protocol vectors and cross-target execution evidence validator."""
import argparse
from collections import Counter
import hashlib
import json
import os
import subprocess
from pathlib import Path
import re
from e2e_report import source_fingerprint

ROOT = Path(__file__).resolve().parents[1]
HEADER = b"MDC-ID\0\x01\x00\x01\x01"
PROTOCOL = "MDC-ID-FNV1A64-LE-v1"
COUNTS = Counter(observation=5, measurement=4, relation=2, composition=2,
                 state=2, transition=1, evaluation=1,
                 **{"evaluation-key": 6, "invalid": 6, "lifecycle": 1, "reevaluation": 1})


def reference_digest(body):
    value = 0xcbf29ce484222325
    for byte in HEADER + len(body).to_bytes(8, "little") + body:
        value = ((value ^ byte) * 0x100000001b3) & 0xffffffffffffffff
    return value


def check_vectors():
    count = 0
    for line in (ROOT / "tests/fixtures/portable-identity-v1.tsv").read_text().splitlines():
        if line.startswith("#"):
            continue
        name, encoded, preimage, digest = line.split("\t")
        body = b"" if encoded == "-" else bytes.fromhex(encoded)
        if HEADER + len(body).to_bytes(8, "little") + body != bytes.fromhex(preimage):
            raise ValueError(name + ": wrong preimage")
        if reference_digest(body) != int(digest, 16):
            raise ValueError(name + ": wrong digest")
        count += 1
    if count != 6:
        raise ValueError("missing protocol vectors")


def check_log(data):
    lines = data.decode("ascii").splitlines()
    if not lines or lines[0] != "protocol=" + PROTOCOL or lines[-1] != "MDC_PORTABLE_IDENTITY_VERIFY=PASS":
        raise ValueError("wrong protocol or incomplete execution")
    kinds, lineages = Counter(), 0
    for line in lines[1:-1]:
        fields = line.split("\t")
        if fields[0] == "lineage":
            if len(fields) != 2 or not fields[1]:
                raise ValueError("missing lineage")
            for reference in fields[1].split(";"):
                if not re.fullmatch(r"(?:[0-9a-f]{2})+", reference):
                    raise ValueError("invalid lineage encoding")
            lineages += 1
            continue
        if len(fields) != 3 or fields[0] not in COUNTS:
            raise ValueError("wrong semantic record")
        kind, digest, body = fields
        if not re.fullmatch(r"[0-9a-f]{16}", digest) or not re.fullmatch(r"(?:[0-9a-f]{2})+", body):
            raise ValueError("invalid bytes/digest")
        if reference_digest(bytes.fromhex(body)) != int(digest, 16):
            raise ValueError("identity does not match captured body")
        kinds[kind] += 1
    if kinds != COUNTS or lineages != 17:
        raise ValueError("incomplete graph, failures or lineage")
    digest = hashlib.sha256(data).hexdigest()
    expected = (ROOT / "tests/fixtures/portable-graph-v1.sha256").read_text().strip()
    if digest != expected:
        raise ValueError("portable graph differs from frozen v1 control")
    return digest


def check_target(info, profile):
    if profile == "portable-i686-unknown-linux-gnu":
        expected = {"arch": "x86", "pointer_width": 32, "endian": "little"}
    elif profile == "portable-s390x-unknown-linux-gnu":
        expected = {"arch": "s390x", "pointer_width": 64, "endian": "big"}
    elif profile in {"default", "std-alloc", "alloc"}:
        expected = {"arch": "x86_64", "pointer_width": 64, "endian": "little"}
    else:
        raise ValueError("unknown matrix profile")
    if info != expected:
        raise ValueError("compiled target does not match matrix claim")


def compare(root):
    rows, seen = [], set()
    fingerprint, files = source_fingerprint(ROOT)
    expected_commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    expected_run = os.environ.get("GITHUB_RUN_ID")
    for log in sorted(Path(root).rglob("portable-1.log")):
        folder = log.parent
        data = log.read_bytes()
        digest = check_log(data)
        if data != (folder / "portable-2.log").read_bytes():
            raise ValueError("two processes differ")
        metadata = json.loads((folder / "metadata.json").read_text())
        target = json.loads((folder / "target.json").read_text())
        if metadata["status"] != "PASS" or metadata["worktree_dirty"] is not False or metadata["implementation_source_sha256"] != fingerprint:
            raise ValueError("unverified/stale/dirty execution")
        pair = (metadata["rustc"].split()[1], metadata["profile"])
        if pair in seen:
            raise ValueError("duplicate matrix configuration")
        seen.add(pair)
        check_target(target, metadata["profile"])
        if metadata["source_files"] != files:
            raise ValueError("source manifest mismatch")
        for name, ci_digest in metadata["ci_file_sha256"].items():
            if hashlib.sha256((ROOT / name).read_bytes()).hexdigest() != ci_digest:
                raise ValueError("CI configuration mismatch")
        if not expected_commit or not expected_run or metadata["commit_sha"] != expected_commit or metadata["github_run_id"] != expected_run:
            raise ValueError("mixed or unidentified checkouts/runs")
        rows.append({"rust": pair[0], "profile": pair[1], "target": target, "portable_sha256": digest})
    profiles = {"default", "std-alloc", "alloc", "portable-i686-unknown-linux-gnu", "portable-s390x-unknown-linux-gnu"}
    if seen != {(rust, profile) for rust in ("1.85.1", "1.98.0") for profile in profiles}:
        raise ValueError("missing matrix configurations")
    return {"acceptance": "PASS", "implementation_source_sha256": fingerprint,
            "commit_sha": expected_commit, "github_run_id": expected_run, "configurations": rows}


def self_test(data):
    check_log(data)
    variants = [b"", data.replace(PROTOCOL.encode(), b"unknown", 1),
                data.replace(b"MDC_PORTABLE_IDENTITY_VERIFY=PASS", b"NOT_EXECUTED"),
                data.replace(b"observation\t", b"measurement\t", 1),
                data.replace(b"observation\t", b"other\t", 1),
                data + b"extra\n", data.replace(b"lineage\t", b"removed\t", 1)]
    for item in variants:
        try:
            check_log(item)
        except (ValueError, UnicodeError):
            continue
        raise ValueError("negative validator control accepted")
    return len(variants)


def main():
    parser = argparse.ArgumentParser()
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--log", type=Path)
    group.add_argument("--compare-root", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    check_vectors()
    if args.log:
        data = args.log.read_bytes()
        result = {"acceptance": "PASS", "portable_sha256": check_log(data), "negative_controls": self_test(data)}
    else:
        result = compare(args.compare_root)
    if args.output:
        args.output.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result))


if __name__ == "__main__":
    main()
