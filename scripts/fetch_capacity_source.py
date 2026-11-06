"""Explicit opt-in acquisition of one of two pinned official benchmark archives."""
import argparse
import hashlib
import io
import json
from pathlib import Path
import urllib.request
import zipfile

ROOT = Path(__file__).resolve().parents[1]


def fetch(url, cap):
    with urllib.request.urlopen(url, timeout=30) as response:
        data = response.read(cap + 1)
    if len(data) > cap:
        raise ValueError("download limit")
    return data


def extract(archive, record):
    if hashlib.sha256(archive).hexdigest() != record["archive_sha256"]:
        raise ValueError("archive differs from pinned SHA-256")
    expected = "BTCUSDT-1m-" + record["day"] + ".csv"
    with zipfile.ZipFile(io.BytesIO(archive)) as z:
        if z.namelist() != [expected] or z.getinfo(expected).file_size > 4 * 1024 * 1024:
            raise ValueError("unexpected archive member or size")
        data = z.read(expected)
    if hashlib.sha256(data).hexdigest() != record["csv_sha256"] or len(data.splitlines()) != record["rows"]:
        raise ValueError("CSV differs from pinned content")
    return data


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--day", required=True, choices=["2024-01-01", "2025-01-01"])
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    records = json.loads((ROOT / "adapters/mdc_replay/tests/fixtures/sources.json").read_text())["records"]
    record = next(r for r in records if r["day"] == args.day)
    checksum = fetch(record["checksum_url"], 4096).decode().split()[0]
    if checksum != record["archive_sha256"]:
        raise ValueError("remote checksum differs from pinned archive")
    archive = fetch(record["archive_url"], 2 * 1024 * 1024)
    data = extract(archive, record)
    args.output.mkdir(parents=True, exist_ok=False)
    (args.output / "source.csv").write_bytes(data)
    (args.output / "source.zip").write_bytes(archive)
    receipt = dict(record, acquisition="explicit_network_fetch", availability_quality="modeled", authentic_receipt_time="not_supplied")
    (args.output / "acquisition.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(dict(source_sha256=record["csv_sha256"], day=args.day)))


if __name__ == "__main__":
    main()
