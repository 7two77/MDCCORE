"""Tampered and incomplete process artifacts must not earn replay verification."""
import json
import os
from pathlib import Path
import shutil
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
from check_binance_replay import verify_folder


class ReplayValidatorTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.folder = Path(self.temp.name)
        source = Path(os.environ["MDC_BINANCE_REPORT_DIR"])
        for name in ("binance-replay-1.json", "binance-replay-2.json", "binance-replay-1.bin", "binance-replay-2.bin"):
            shutil.copyfile(source / name, self.folder / name)

    def test_real_fresh_process_outputs_pass(self):
        self.assertEqual(verify_folder(self.folder)["technical_confirmation"], "UNRESOLVED")

    def test_claimed_measured_quality_future_time_or_forged_basis_fail(self):
        report = json.loads((self.folder / "binance-replay-1.json").read_text())
        for field, value in (("availability_quality", "measured"), ("decision_us", 0), ("eligible", [])):
            with self.subTest(field=field):
                mutated = {**report, field: value}
                (self.folder / "binance-replay-1.json").write_text(json.dumps(mutated))
                with self.assertRaises(ValueError):
                    verify_folder(self.folder)
        report["eligible"][0]["evidence_basis_hex"] = "00"
        (self.folder / "binance-replay-1.json").write_text(json.dumps(report))
        with self.assertRaises(ValueError):
            verify_folder(self.folder)

    def test_mutated_snapshot_fails(self):
        path = self.folder / "binance-replay-1.bin"
        path.write_bytes(path.read_bytes() + b"x")
        with self.assertRaises(ValueError):
            verify_folder(self.folder)

    def test_missing_second_process_fails(self):
        (self.folder / "binance-replay-2.json").unlink()
        with self.assertRaises(FileNotFoundError):
            verify_folder(self.folder)

    def test_different_second_report_fails(self):
        (self.folder / "binance-replay-2.json").write_text("{}")
        with self.assertRaises(ValueError):
            verify_folder(self.folder)


if __name__ == "__main__":
    unittest.main()
