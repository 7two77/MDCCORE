"""Run the real storage binary in separate processes; no core/WAL algorithm here."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    output = args.output.resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--locked", "--offline", "--no-deps", "--format-version", "1",
         "--manifest-path", "adapters/mdc_store/Cargo.toml"], cwd=ROOT, text=True))
    executable = Path(metadata["target_directory"]) / "debug/mdc-recovery"
    folder = Path(tempfile.mkdtemp(prefix="recovery-", dir=output.parent))
    cases = []
    expected = "MDC_RECOVERY_VERIFY=PASS operations=38 durable=38 objects=31 revision=2"
    for name in ("wal", "checkpoint"):
        directory, oracle = folder / name, folder / (name + ".bin")
        rows = []
        for mode in ("produce", "consume", "consume"):
            command = [str(executable), mode, str(directory), str(oracle)]
            if mode == "produce" and name == "checkpoint":
                command.append("checkpoint")
            started = time.monotonic()
            child = subprocess.Popen(command, cwd=ROOT, stdout=subprocess.PIPE,
                                     stderr=subprocess.PIPE, text=True)
            try:
                stdout, stderr = child.communicate(timeout=120)
            except subprocess.TimeoutExpired:
                child.kill()
                child.communicate()
                raise RuntimeError("storage verification process timed out")
            if child.returncode or stdout.strip() != expected:
                raise RuntimeError(f"storage process failed: {child.returncode}: {stdout}: {stderr}")
            rows.append({"pid": child.pid, "exit_code": child.returncode,
                         "stdout": stdout.strip(), "elapsed_seconds": time.monotonic() - started})
        if len({r["pid"] for r in rows}) != 3:
            raise ValueError("three distinct processes required")
        # The frozen rich core snapshot is preserved inside both storage paths.
        digest = hashlib.sha256(oracle.read_bytes()).hexdigest()
        control = json.loads((ROOT / "tests/fixtures/snapshot-roundtrip-v1.json").read_text())
        if digest != control["archive_sha256"]:
            raise ValueError("core snapshot differs from frozen 011 control")
        cases.append({"id": name, "processes": rows, "core_snapshot_sha256": digest,
                      "store_files_sha256": {p.name: hashlib.sha256(p.read_bytes()).hexdigest()
                                             for p in sorted(directory.iterdir()) if p.is_file()}})
    result = {"acceptance": "PASS", "kind": "fresh_process_recovery_execution",
              "scope": "Linux process restart; not real power-loss validation",
              "cases": cases}
    output.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({"acceptance": "PASS", "report": str(output), "cases": len(cases),
                      "processes_per_case": [len(c["processes"]) for c in cases]}))


if __name__ == "__main__":
    main()
