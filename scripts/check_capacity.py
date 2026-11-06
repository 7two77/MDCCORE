"""Validate a saved study or compare exact resource-study bytes across CI jobs."""
import argparse
import json
import os
from pathlib import Path
import subprocess
from capacity_report import LIMITS, read_json, same_files, validate
from ci_metadata import ci_fingerprints
from e2e_report import source_fingerprint

ROOT = Path(__file__).resolve().parents[1]


def compare_local(folders):
    if len(folders)!=2:
        raise ValueError("two native toolchain studies required")
    reports=[]
    for folder in folders:
        checked=validate(folder);report=read_json(Path(folder)/"capacity-report.json")
        reports.append((Path(folder),report,checked))
    a,b=reports
    if {a[1]["build"]["rustc"].split()[1],b[1]["build"]["rustc"].split()[1]}!={"1.85.1","1.98.0"}:
        raise ValueError("missing declared local toolchain")
    for key in ("points","repetitions","profile","limits"):
        if a[1][key]!=b[1][key]:raise ValueError("unlike local configurations")
    if a[1]["source"]["sha256"]!=b[1]["source"]["sha256"]:raise ValueError("unlike source inputs")
    for left,right in zip(a[1]["cases"],b[1]["cases"]):
        if (left["requested"],left["repetition"],left["outcome"],left["committed"],left.get("first_constraint"))!=(right["requested"],right["repetition"],right["outcome"],right["committed"],right.get("first_constraint")):
            raise ValueError("local cross-toolchain outcomes differ")
        if "file_sha256" in left:same_files(a[0]/left["directory"],b[0]/right["directory"])
    return dict(acceptance="PASS",verification_scope="local studies, not hosted CI",configurations=[dict(rust=r[1]["build"]["rustc"].split()[1],checked=r[2],summary=r[1]["summary"],host=r[1]["host"]) for r in reports])


def compare(root, study=False):
    root = Path(root)
    fingerprint, files = source_fingerprint(ROOT)
    expected_commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    expected_run = os.environ.get("GITHUB_RUN_ID")
    groups, rows, seen = {}, [], set()
    for path in sorted(root.rglob("capacity-report.json")):
        folder = path.parent
        checked = validate(folder)
        report = read_json(path)
        metadata_path = next((p / "metadata.json" for p in folder.parents if (p / "metadata.json").exists() and (p == root or root in p.parents)), None)
        if metadata_path is None:
            raise ValueError("missing parent CI metadata")
        metadata = read_json(metadata_path)
        if (metadata["status"] != "PASS" or metadata["worktree_dirty"] is not False or metadata["implementation_source_sha256"] != fingerprint
                or metadata["source_files"] != files or metadata["ci_file_sha256"] != ci_fingerprints(ROOT)
                or metadata["commit_sha"] != expected_commit or not expected_run or metadata["github_run_id"] != expected_run
                or metadata["profile"] != ("capacity-study" if study else "binance-replay")):
            raise ValueError("stale/dirty/mixed CI capacity execution")
        build = report["build"]
        if build["commit_sha"] != expected_commit or build["worktree_dirty"] is not False or build["rustc"] != metadata["rustc"]:
            raise ValueError("build checkout/compiler mismatch")
        rust = build["rustc"].split()[1]
        probe = report["probe_kind"]
        key = (rust, probe)
        if key in seen:
            raise ValueError("duplicate native capacity configuration")
        seen.add(key)
        if not study and (report["points"] != [1, 2, 3, 1440] or report["repetitions"] != 3
                          or report["source"]["origin"] != "synthetic_fixture"
                          or (probe == "explicit_reduced_journal_budget" and report["limits"]["max_journal_bytes"] != 32768)):
            raise ValueError("regular CI smoke profile changed")
        if probe == "explicit_reduced_journal_budget" and not any(c["outcome"] == "Limited" and c["committed"] > 0 for c in report["cases"]):
            raise ValueError("retained-prefix limit probe was not executed")
        semantic = {k: report[k] for k in ("points", "repetitions", "profile", "limits")}
        semantic["source_sha256"] = report["source"]["sha256"]
        if probe in groups:
            previous_folder, previous = groups[probe]
            prior = {k: previous[k] for k in ("points", "repetitions", "profile", "limits")}
            prior["source_sha256"] = previous["source"]["sha256"]
            if prior != semantic:
                raise ValueError("unlike studies cannot be byte-compared")
            for a, b in zip(previous["cases"], report["cases"]):
                if (a["requested"], a["repetition"], a["outcome"], a["committed"],a.get("first_constraint")) != (b["requested"], b["repetition"], b["outcome"], b["committed"],b.get("first_constraint")):
                    raise ValueError("cross-toolchain capacity outcomes differ")
                if "file_sha256" in a:
                    same_files(previous_folder / a["directory"], folder / b["directory"])
        else:
            groups[probe] = (folder, report)
        rows.append(dict(rust=rust, probe=probe, checked=checked, summary=report["summary"], host=report["host"]))
    probes = {"default_limits"} if study else {"default_limits", "explicit_reduced_journal_budget"}
    if seen != {(rust, probe) for rust in ("1.85.1", "1.98.0") for probe in probes}:
        raise ValueError("missing capacity matrix configurations")
    return dict(acceptance="PASS", commit_sha=expected_commit, github_run_id=expected_run,
                implementation_source_sha256=fingerprint, configurations=rows, timing_slo="not_asserted")


def main():
    parser = argparse.ArgumentParser()
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--folder", type=Path)
    mode.add_argument("--compare-root", type=Path)
    mode.add_argument("--compare-local",nargs=2,type=Path)
    parser.add_argument("--study", action="store_true")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    result = validate(args.folder) if args.folder else compare_local(args.compare_local) if args.compare_local else compare(args.compare_root, args.study)
    args.output.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result))


if __name__ == "__main__":
    main()
