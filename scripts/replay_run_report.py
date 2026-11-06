"""Independent LE/SHA-256 checks for real replay-run files and exported bundles."""
import hashlib
import json
from pathlib import Path
import struct

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "adapters/mdc_replay/tests/fixtures"
PLAN = [1704067260098999, 1704067260099000, 1704067320049000, 1704067320099000]
PREFIX = b"MDC-REPLAY-RUN-001/"


def digest(b):
    return hashlib.sha256(b).digest()


def frame(data, offset, name, magic, cap):
    if len(data) - offset < 58 or data[offset:offset + 16] != magic:
        raise ValueError("truncated frame or wrong magic")
    version, length = struct.unpack_from("<HQ", data, offset + 16)
    if version != 1 or length > cap or offset + 58 + length > len(data):
        raise ValueError("frame version/length")
    end = offset + 26 + length
    h = digest(PREFIX + name + b"/v1\0" + data[offset:end])
    if h != data[end:end + 32]:
        raise ValueError("frame checksum")
    return data[offset + 26:end], h, end + 32


def vectors():
    return {name: bytes.fromhex(value) for line in (FIXTURES / "replay-run-wire-v1.tsv").read_text().splitlines()
            if not line.startswith("#") for name, value in [line.split("\t")]}


def verify_run(folder, export_prefix):
    folder = Path(folder)
    manifest = (folder / "MANIFEST").read_bytes()
    source = (folder / "SOURCE").read_bytes()
    head = (folder / "HEAD").read_bytes()
    journal = (folder / "JOURNAL").read_bytes()
    v = vectors()
    if manifest != v["manifest"] or source != (FIXTURES / "BTCUSDT-1m-2024-01-01-first3.csv").read_bytes():
        raise ValueError("captured manifest/source differs")
    body, _, end = frame(manifest, 0, b"manifest", b"MDC-RMAN-001\0\0\0\0", 16384)
    if end != len(manifest):
        raise ValueError("manifest trailing data")
    run = digest(PREFIX + b"run-id/v1\0" + body)
    if run != v["run-id"]:
        raise ValueError("run id differs")
    h, _, end = frame(head, 0, b"head", b"MDC-RHED-001\0\0\0\0", 256)
    if end != len(head) or len(h) != 144:
        raise ValueError("HEAD trailing/invalid size")
    offset, count = struct.unpack_from("<QQ", h, 64)
    if h[:32] != run or h[32:64] != digest(manifest) or count != len(PLAN) or offset != len(journal):
        raise ValueError("HEAD identity/prefix/cursor differs")
    if not journal.startswith(v["journal-empty"]):
        raise ValueError("journal header differs")
    position = len(v["journal-empty"])
    previous = v["journal-empty"][-32:]
    results = []
    for index, cutoff in enumerate(PLAN, 1):
        b, tip, position = frame(journal, position, b"decision", b"MDC-RDEC-001\0\0\0\0", 24 * 1024 * 1024)
        if len(b) < 192 or b[:32] != run or struct.unpack_from("<QQ", b, 32) != (index, cutoff) or b[144:176] != previous:
            raise ValueError("record run/order/chain mismatch")
        decision_id, report_hash, snapshot_hash = b[48:80], b[80:112], b[112:144]
        report_len, = struct.unpack_from("<Q", b, 176)
        at = 184 + report_len
        if report_len > 1024 * 1024 or at + 8 > len(b):
            raise ValueError("report bounds")
        report = b[184:at]
        snapshot_len, = struct.unpack_from("<Q", b, at)
        snapshot = b[at + 8:]
        if snapshot_len != len(snapshot) or snapshot_len > 16 * 1024 * 1024 or digest(report) != report_hash or digest(snapshot) != snapshot_hash:
            raise ValueError("bundle length/digest mismatch")
        info = json.loads(report)
        if (info["decision_us"] != cutoff or info["decision_id"] != decision_id.hex()
                or info["raw_dataset_sha256"] != digest(source).hex() or info["availability_quality"] != "modeled"
                or info["core_snapshot_sha256"] != snapshot_hash.hex()):
            raise ValueError("report context differs")
        key = b"BINANCE-SPOT-1M-MODELED-001/decision/v1\0" + body[45:56] + struct.pack("<QQ", cutoff, len(info["eligible"]))
        for e in info["eligible"]:
            basis = bytes.fromhex(e["evidence_basis_hex"])
            if e["available_us"] > cutoff or digest(basis).hex() != e["evidence_id"]:
                raise ValueError("ineligible/forged evidence")
            key += struct.pack("<Q", len(basis)) + basis
        if digest(key) != decision_id:
            raise ValueError("decision id does not bind eligible evidence")
        if info["technical_confirmation"] != ["NO_INPUT", "UNRESOLVED", "UNRESOLVED", "PASS"][index - 1]:
            raise ValueError("unexpected technical result")
        if report != (folder.parent / f"{export_prefix}-{index}.json").read_bytes() or snapshot != (folder.parent / f"{export_prefix}-{index}.bin").read_bytes():
            raise ValueError("export and committed bundle differ")
        if index == 3 and info != json.loads((FIXTURES / "control-v1.json").read_text())["expected_report"]:
            raise ValueError("014 late-witness control changed")
        latest = digest(PREFIX + b"bundle/v1\0" + b[:144])
        results.append({"index": index, "cutoff": cutoff, "snapshot_sha256": snapshot_hash.hex(),
                        "report_sha256": report_hash.hex(), "technical_confirmation": info["technical_confirmation"]})
        previous = tip
    if position != offset or previous != h[80:112] or latest != h[112:144]:
        raise ValueError("HEAD tip/latest bundle mismatch")
    return {"run_id": run.hex(), "durable_through": count, "journal_sha256": digest(journal).hex(), "decisions": results}


def verify_folder(folder):
    folder = Path(folder)
    reference = verify_run(folder / "run-reference", "recovery-reference")
    resumed = verify_run(folder / "run-resumed", "recovery-resumed")
    if reference != resumed:
        raise ValueError("continuous and restarted runs differ")
    for name in ("SOURCE", "MANIFEST", "HEAD", "JOURNAL"):
        if (folder / "run-reference" / name).read_bytes() != (folder / "run-resumed" / name).read_bytes():
            raise ValueError("persistent bytes differ")
    return {"acceptance": "PASS", "profile": "MDC-REPLAY-RUN-001/1", **reference}
