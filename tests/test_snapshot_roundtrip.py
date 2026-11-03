"""Frozen artifact checks must reject tampering and missing process receipts."""
import os
from pathlib import Path
import shutil
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
from check_snapshot_roundtrip import verify_folder  # noqa: E402


class SnapshotEvidenceTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.folder = Path(self.temp.name)
        source = Path(os.environ["MDC_SNAPSHOT_REPORT"])
        for name in ("snapshot.bin", "snapshot.bin.state", "snapshot-producer.log",
                     "snapshot-consumer-1.log", "snapshot-consumer-2.log"):
            shutil.copyfile(source / name, self.folder / name)

    def test_real_artifacts_match_frozen_control(self):
        self.assertEqual(verify_folder(self.folder)["processes"], 3)

    def test_archive_and_manifest_tampering_are_rejected(self):
        for name in ("snapshot.bin", "snapshot.bin.state"):
            with self.subTest(name=name):
                path = self.folder / name
                original = path.read_bytes()
                path.write_bytes(original[:-1] + bytes([original[-1] ^ 1]))
                with self.assertRaisesRegex(ValueError, "frozen control"):
                    verify_folder(self.folder)
                path.write_bytes(original)

    def test_consumer_mismatch_is_rejected(self):
        (self.folder / "snapshot-consumer-2.log").write_text("MDC_SNAPSHOT_VERIFY=FAIL\n")
        with self.assertRaisesRegex(ValueError, "consumer"):
            verify_folder(self.folder)

    def test_missing_and_wrong_producer_receipts_are_rejected(self):
        path = self.folder / "snapshot-producer.log"
        path.write_text("MDC_SNAPSHOT_VERIFY=PASS operations=0\n")
        with self.assertRaisesRegex(ValueError, "producer"):
            verify_folder(self.folder)
        path.unlink()
        with self.assertRaises(FileNotFoundError):
            verify_folder(self.folder)


if __name__ == "__main__":
    unittest.main()
