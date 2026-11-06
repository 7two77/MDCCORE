"""Drive production CLI processes; arithmetic and State algorithms stay in Rust."""
import argparse
import json
from pathlib import Path
import subprocess
from replay_run_report import PLAN, ROOT, verify_folder


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--folder", required=True, type=Path)
    args = parser.parse_args()
    folder = args.folder.resolve()
    folder.mkdir(parents=True, exist_ok=True)
    manifest = ROOT / "adapters/mdc_replay/Cargo.toml"
    metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--locked", "--offline", "--no-deps", "--format-version", "1", "--manifest-path", str(manifest)], cwd=ROOT))
    binary = Path(metadata["target_directory"]) / "debug/mdc-replay-run"
    calls = []

    def call(*a, success=True):
        result = subprocess.run([str(binary), *map(str, a)], cwd=ROOT, stdin=subprocess.DEVNULL, capture_output=True, text=True, timeout=60)
        calls.append({"args": list(map(str, a)), "exit": result.returncode, "stdout": result.stdout, "stderr": result.stderr})
        if (result.returncode == 0) != success:
            raise ValueError(f"CLI failed: {a}: {result.stderr}")
        return result

    for name in ("reference", "resumed"):
        run = folder / f"run-{name}"
        call("create", run, ROOT / "adapters/mdc_replay/tests/fixtures/BTCUSDT-1m-2024-01-01-first3.csv", "BTCUSDT", "ms", "100", "close", ",".join(map(str, PLAN)))
        if name == "reference":
            call("finish", run)
        else:
            for index in range(1, 5):
                call("step", run, index)
                status = call("status", run)
                if f"durable_through={index}" not in status.stdout:
                    raise ValueError("restart cursor differs")
        before = [(run / file).read_bytes() for file in ("HEAD", "JOURNAL")]
        call("step", run, 3)  # Caller lost/discarded a previous reply.
        call("step", run, 5, success=False)
        if before != [(run / file).read_bytes() for file in ("HEAD", "JOURNAL")]:
            raise ValueError("retry/error changed durable prefix")
        for index in range(1, 5):
            call("export", run, index, folder / f"recovery-{name}-{index}.json", folder / f"recovery-{name}-{index}.bin")
    result = verify_folder(folder)
    result["process_calls"] = calls
    result["implementation_source_sha256"] = __import__("e2e_report").source_fingerprint(ROOT)[0]
    (folder / "replay-recovery-execution.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({"acceptance": "PASS", "decisions": result["durable_through"], "processes": len(calls), "run_id": result["run_id"]}))


if __name__ == "__main__":
    main()
