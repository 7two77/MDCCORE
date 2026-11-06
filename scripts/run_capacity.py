"""Bounded host measurements via production Rust APIs; no trading algorithms."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import signal
import subprocess
import struct
import sys
import time
from capacity_report import LIMITS, read_json, run_files, sha, summaries, validate
from e2e_report import source_fingerprint
from ci_metadata import ci_fingerprints

ROOT = Path(__file__).resolve().parents[1]


def invoke(command, root, name, timeout, *, env=None, pipe_stdin=False):
    """wait4 owns reaping; an unreaped child PID cannot be reused before killpg."""
    root = Path(root)
    stdout, stderr = root / (name + ".stdout"), root / (name + ".stderr")
    stdout.parent.mkdir(parents=True, exist_ok=True)
    if stdout.exists() or stderr.exists():
        raise ValueError("measurement output already exists")
    start = time.perf_counter_ns()
    expired = False
    with stdout.open("xb") as out, stderr.open("xb") as err:
        try:
            child = subprocess.Popen(list(map(str, command)), cwd=ROOT, stdin=subprocess.PIPE if pipe_stdin else subprocess.DEVNULL,
                                     stdout=out, stderr=err, env=env, start_new_session=True)
        except OSError as e:
            err.write(str(e).encode());err.flush();out.flush()
            return dict(exit_code=127,timeout=False,wall_ns=time.perf_counter_ns()-start,user_cpu_us=0,system_cpu_us=0,peak_rss_kib=0,
                        stdout_file=str(stdout.relative_to(root)),stderr_file=str(stderr.relative_to(root)),stdout_sha256=sha(stdout),stderr_sha256=sha(stderr),spawn_error=str(e))
        try:
            while True:
                pid, status, usage = os.wait4(child.pid, os.WNOHANG)
                if pid:
                    break
                if time.perf_counter_ns() - start > timeout * 1e9:
                    os.killpg(child.pid, signal.SIGKILL)
                    _, status, usage = os.wait4(child.pid, 0)
                    expired = True
                    break
                time.sleep(0.005)
            child.returncode = os.waitstatus_to_exitcode(status)
        finally:
            if child.returncode is None:
                os.killpg(child.pid, signal.SIGKILL)
                _, status, _ = os.wait4(child.pid, 0)
                child.returncode = os.waitstatus_to_exitcode(status)
            if child.stdin:
                child.stdin.close()
    return dict(exit_code=child.returncode, timeout=expired, wall_ns=time.perf_counter_ns() - start,
                user_cpu_us=round(usage.ru_utime * 1e6), system_cpu_us=round(usage.ru_stime * 1e6), peak_rss_kib=usage.ru_maxrss,
                stdout_file=str(stdout.relative_to(root)), stderr_file=str(stderr.relative_to(root)),
                stdout_sha256=sha(stdout), stderr_sha256=sha(stderr))


def parsed(root, entry):
    if entry["timeout"] or entry["exit_code"]:
        return None
    try:
        return read_json(Path(root) / entry["stdout_file"])
    except (ValueError,OSError):
        entry["parse_error"]=True
        return None


def synthetic(unit):
    start, quantum = (1704067200000, 1000) if unit == "ms" else (1735689600000000, 1)
    minute = 60000000 // quantum
    return "".join(f"{start+i*minute},1,1,1,1,1,{start+(i+1)*minute-1},1,1,0,0,0\n" for i in range(1440)).encode()


def output_bytes(root):
    return sum(p.stat().st_size for p in Path(root).rglob("*") if p.is_file())


def cpu_model():
    try:
        return next((line.split(":", 1)[1].strip() for line in Path("/proc/cpuinfo").read_text().splitlines()
                     if line.startswith("model name") and ":" in line), platform.processor())
    except OSError:
        return platform.processor()


def measure(args):
    if platform.system() != "Linux" or not hasattr(os, "wait4"):
        raise ValueError("Linux wait4 measurement profile required")
    points = [int(v) for v in args.points.split(",")]
    if not points or len(points) > 16 or points != sorted(set(points)) or any(not 1 <= v <= 4096 for v in points):
        raise ValueError("points must be 1..4096, unique, sorted, at most 16")
    if not 3 <= args.repetitions <= 10 or not 0.05 <= args.timeout <= 600 or not 128 <= args.max_output_mib <= 4096 or not 124 <= args.journal_budget_bytes <= LIMITS["max_journal_bytes"]:
        raise ValueError("invalid repetition/time/storage budget")
    if any(os.environ.get(k) for k in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS")):
        raise ValueError("controlled profile requires no extra RUSTFLAGS")
    if args.synthetic_day:
        data, origin = synthetic(args.unit), "synthetic_fixture"
    else:
        if not args.expected_source_sha256:
            raise ValueError("provided CSV requires --expected-source-sha256")
        with args.source.open("rb") as f:
            data = f.read(4 * 1024 * 1024 + 1)
        origin = "provided_csv"
    source_hash = hashlib.sha256(data).hexdigest()
    if len(data) > 4 * 1024 * 1024 or (args.expected_source_sha256 and source_hash != args.expected_source_sha256):
        raise ValueError("source byte/hash mismatch")
    out = args.output.resolve()
    for name in ("src", "examples", "tests", "scripts", "adapters"):
        if out == ROOT / name or ROOT / name in out.parents:
            raise ValueError("measurement output must be outside source directories")
    out.mkdir(parents=True, exist_ok=False)
    args._created_output = True
    (out / "source.csv").write_bytes(data)
    fingerprint, files = source_fingerprint(ROOT)
    env = os.environ.copy()
    env.update(CARGO_INCREMENTAL="0", CARGO_PROFILE_RELEASE_DEBUG="0", CARGO_PROFILE_RELEASE_OPT_LEVEL="3")
    command = ["cargo", "build", "--locked", "--offline", "--release", "--manifest-path", str(ROOT / "adapters/mdc_replay/Cargo.toml"),
               "--bin", "mdc-replay-capacity", "--bin", "mdc-binance-replay"]
    build = invoke(command, out, "build", 600, env=env)
    if build["exit_code"]:
        raise ValueError("release build failed; see retained build artifacts")
    metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--locked", "--offline", "--no-deps", "--format-version", "1", "--manifest-path", str(ROOT / "adapters/mdc_replay/Cargo.toml")], cwd=ROOT, env=env))
    (out / "bin").mkdir()
    binaries = {}
    for name in ("mdc-replay-capacity", "mdc-binance-replay"):
        shutil.copy2(Path(metadata["target_directory"]) / "release" / name, out / "bin" / name)
        binaries[name] = dict(file="bin/" + name, sha256=sha(out / "bin" / name))
    def git(*a):
        return subprocess.check_output(["git", *a], cwd=ROOT, text=True).strip()
    selected_limits={**LIMITS,"max_journal_bytes":args.journal_budget_bytes}
    report = dict(schema="MDC-CAPACITY-001/1", status="RUNNING", status_meaning="planned measurement coverage, not completion of every requested run",
                  points=points, repetitions=args.repetitions, limits=selected_limits, probe_kind="default_limits" if selected_limits==LIMITS else "explicit_reduced_journal_budget", cases=[], summary=[],
                  source=dict(file="source.csv", sha256=source_hash, bytes=len(data), origin=origin, label=args.source_label, authenticity="not_attested"),
                  profile=dict(symbol=args.symbol, unit=args.unit, lag_ms=args.lag_ms, anchor=args.anchor, availability_quality="modeled"),
                  budgets=dict(timeout_seconds=args.timeout, max_output_bytes=args.max_output_mib * 1024 * 1024, recovery_timeout_seconds=args.timeout),
                  method=dict(clock="perf_counter_ns parent / Instant API", child_reaping="wait4", poll_interval_ns=5000000, cpu_unit="us", rss_unit="KiB", rss_note="includes fork/exec baseline; not allocator accounting", cache="uncontrolled/warm; no host cache flush", percentiles="none"),
                  host=dict(os=platform.platform(), machine=platform.machine(), cpu_model=cpu_model(),logical_cpus=os.cpu_count(),python_pointer_width=struct.calcsize("P")*8,python_byteorder=sys.byteorder, filesystem=subprocess.check_output(["stat", "-f", "-c", "%T", str(out)], text=True).strip()),
                  build=dict(profile="release", binaries=binaries, command=command, process=build,
                             rustc=subprocess.check_output(["rustc", "-vV"], text=True, env=env).strip(), cargo=subprocess.check_output(["cargo", "--version"], text=True, env=env).strip(),
                             commit_sha=git("rev-parse", "HEAD"), tree_sha=git("rev-parse", "HEAD^{tree}"), worktree_dirty=bool(git("status", "--porcelain=v1", "-uall")),
                             implementation_source_sha256=fingerprint, source_files=files, ci_file_sha256=ci_fingerprints(ROOT),
                             selected_flags={k: env[k] for k in ("CARGO_INCREMENTAL", "CARGO_PROFILE_RELEASE_DEBUG", "CARGO_PROFILE_RELEASE_OPT_LEVEL")}, extra_rustflags_present=any(env.get(k) for k in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS"))))
    driver = out / binaries["mdc-replay-capacity"]["file"]
    control = invoke([out / binaries["mdc-binance-replay"]["file"], "decision", ROOT / "adapters/mdc_replay/tests/fixtures/BTCUSDT-1m-2024-01-01-first3.csv", "BTCUSDT", "ms", "100", "close", "1704067320049000", out / "control.json", out / "control.bin"], out, "control", args.timeout)
    if control["exit_code"]:
        raise ValueError("release control did not execute")
    report["control"] = dict(report_file="control.json", snapshot_file="control.bin", process=control)
    for n in points:
        for repetition in range(1, args.repetitions + 1):
            key = f"n{n:04}-r{repetition:02}"
            directory = out / "cases" / key / "run"
            directory.parent.mkdir(parents=True, exist_ok=False)
            row = dict(requested=n, repetition=repetition, directory=str(directory.relative_to(out)), phases={}, outcome="INCOMPLETE", committed=None,
                       commit_api_ns=None, recovery_api_ns=None, journal_bytes=None)
            # Reserve one whole bounded run; never silently prune earlier samples.
            need = selected_limits["max_journal_bytes"] + len(data) + 4 * 1024 * 1024
            if output_bytes(out) + need > report["budgets"]["max_output_bytes"] or shutil.disk_usage(out).free < need + 64 * 1024 * 1024:
                row["outcome"] = "StorageBudgetStop"
                report["cases"].append(row)
                report["status"] = "INCOMPLETE"
                report["summary"] = summaries(report["cases"])
                (out / "capacity-report.json").write_text(json.dumps(report, indent=2) + "\n")
                raise ValueError("measurement storage budget exhausted; partial report retained")
            def call(phase, *tail):
                entry = invoke([driver, phase, directory, *tail], out, "cases/" + key + "/" + phase, args.timeout)
                row["phases"][phase] = entry
                return parsed(out, entry)
            create = call("create", out / "source.csv", args.symbol, args.unit, str(args.lag_ms), args.anchor, str(n),str(args.journal_budget_bytes))
            if create:
                report["build"].setdefault("target",create["target"])
            if create and create["outcome"] == "PreflightRejected":
                row.update(outcome="PreflightRejected", committed=0,first_constraint=create.get("error",create.get("error_class")))
            elif create and create["outcome"] == "Created":
                commit = call("commit")
                # Always recover after a timed-out/lost/indeterminate commit.
                recover = call("recover")
                if recover and recover["outcome"] == "Recovered":
                    real = run_files(directory, data, report["profile"],selected_limits)
                    row.update(committed=real["committed"], journal_bytes=real["file_bytes"]["JOURNAL"], file_sha256=real["file_sha256"], file_bytes=real["file_bytes"], recovery_api_ns=recover["open_ns"])
                    if real["committed"]:
                        call("retry", str(real["committed"]))
                    if commit and commit["outcome"] in ("Completed", "Limited"):
                        row.update(outcome=commit["outcome"], commit_api_ns=sum(s["api_ns"] for s in commit["steps"]),first_constraint=commit.get("error"))
                    else:
                        row["outcome"] = "TimeoutRecovered" if row["phases"]["commit"]["timeout"] else "IndeterminateRecovered"
                        row["first_constraint"]=(commit or {}).get("error",row["outcome"])
                else:
                    row["outcome"] = "RecoveryUnmeasured"
            elif directory.exists():
                # An interrupted create may or may not have published its empty HEAD.
                recovered = call("recover")
                row["outcome"] = "CreateInterruptedRecovered" if recovered and recovered["outcome"] == "Recovered" else "CreateIndeterminate"
                if recovered and recovered["outcome"]=="Recovered":
                    real=run_files(directory,data,report["profile"],selected_limits)
                    row.update(committed=real["committed"],journal_bytes=real["file_bytes"]["JOURNAL"],file_sha256=real["file_sha256"],file_bytes=real["file_bytes"],recovery_api_ns=recovered["open_ns"])
            else:
                row["outcome"] = "CreateUnmeasured"
            report["cases"].append(row)
            report["summary"] = summaries(report["cases"])
            (out / "capacity-report.json").write_text(json.dumps(report, indent=2) + "\n")
            print(json.dumps(dict(requested=n, repetition=repetition, outcome=row["outcome"], committed=row["committed"])), flush=True)
    if source_fingerprint(ROOT)[0] != fingerprint:
        raise ValueError("source changed during measurement")
    report["status"] = "COMPLETE"
    (out / "capacity-report.json").write_text(json.dumps(report, indent=2) + "\n")
    result = validate(out)
    (out / "capacity-validation.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result), flush=True)


def main():
    parser = argparse.ArgumentParser()
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--source", type=Path)
    source.add_argument("--synthetic-day", action="store_true")
    parser.add_argument("--expected-source-sha256")
    parser.add_argument("--source-label", default="caller-provided input")
    parser.add_argument("--symbol", choices=["BTCUSDT", "ETHUSDT"], default="BTCUSDT")
    parser.add_argument("--unit", choices=["ms", "us"], default="ms")
    parser.add_argument("--lag-ms", type=int, default=100)
    parser.add_argument("--anchor", choices=["close", "end"], default="close")
    parser.add_argument("--points", default="1,16,32,64,128,256,1440")
    parser.add_argument("--repetitions", type=int, default=3)
    parser.add_argument("--timeout", type=float, default=120)
    parser.add_argument("--max-output-mib", type=int, default=1536)
    parser.add_argument("--journal-budget-bytes",type=int,default=LIMITS["max_journal_bytes"])
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    args._created_output = False
    try:
        measure(args)
    except (ValueError, OSError, subprocess.SubprocessError,KeyError,TypeError) as e:
        if args._created_output and (args.output / "capacity-report.json").exists():
            report = read_json(args.output / "capacity-report.json")
            report["status"] = "INCOMPLETE"
            report["validation_error"] = str(e)
            completed={(c["requested"],c["repetition"]) for c in report["cases"]}
            report["unexecuted"]=[dict(requested=n,repetition=r) for n in report["points"] for r in range(1,report["repetitions"]+1) if (n,r) not in completed]
            (args.output / "capacity-report.json").write_text(json.dumps(report, indent=2) + "\n")
        raise SystemExit("MDC_CAPACITY=INCOMPLETE " + str(e))


if __name__ == "__main__":
    main()
