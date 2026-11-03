"""Compare real producer/consumer artifacts; no MDC algorithm lives here."""
import argparse
import hashlib
import json
from pathlib import Path
from check_portable_identity import compare

CONTROL = Path(__file__).resolve().parents[1] / "tests/fixtures/snapshot-roundtrip-v1.json"


def verify_folder(folder):
    folder = Path(folder)
    control = json.loads(CONTROL.read_text())
    producer = (folder / "snapshot-producer.log").read_bytes()
    if producer != control["process_receipt"].encode():
        raise ValueError("snapshot producer does not match frozen receipt")
    for name in ("snapshot-consumer-1.log", "snapshot-consumer-2.log"):
        if (folder / name).read_bytes() != producer:
            raise ValueError("snapshot consumer result differs")
    record = {"artifact_folder": folder.name, "processes": 3}
    for kind, filename in (("archive", "snapshot.bin"), ("manifest", "snapshot.bin.state")):
        data = (folder / filename).read_bytes()
        digest = hashlib.sha256(data).hexdigest()
        if len(data) != control[kind + "_bytes"] or digest != control[kind + "_sha256"]:
            raise ValueError(f"snapshot {kind} differs from frozen control")
        record[kind + "_bytes"] = len(data)
        record[kind + "_sha256"] = digest
    return record


def verify(root):
    metadata = compare(root)
    records = []
    archives, manifests = set(), set()
    for log in sorted(Path(root).rglob("portable-1.log")):
        record = verify_folder(log.parent)
        archives.add(record["archive_sha256"])
        manifests.add(record["manifest_sha256"])
        records.append(record)
    if len(records) != 10 or len(archives) != 1 or len(manifests) != 1:
        raise ValueError("snapshot bytes differ across ten configurations")
    return {"acceptance": "PASS", "commit_sha": metadata["commit_sha"], "github_run_id": metadata["github_run_id"], "implementation_source_sha256": metadata["implementation_source_sha256"], "configurations": records}


def main():
    p = argparse.ArgumentParser()
    mode = p.add_mutually_exclusive_group(required=True)
    mode.add_argument("--compare-root", type=Path)
    mode.add_argument("--folder", type=Path)
    p.add_argument("--output", required=True, type=Path)
    args = p.parse_args()
    result = (verify(args.compare_root) if args.compare_root else
              {"acceptance": "PASS", **verify_folder(args.folder)})
    args.output.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result))


if __name__ == "__main__":
    main()
