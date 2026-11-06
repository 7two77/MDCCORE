import os
from pathlib import Path
import shutil
import sys
import tempfile
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
from replay_run_report import verify_folder


class RecoveryReportTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.folder = Path(self.temp.name) / "artifacts"
        shutil.copytree(Path(os.environ["MDC_REPLAY_RECOVERY_DIR"]), self.folder)

    def test_real_processes_pass(self):
        self.assertEqual(verify_folder(self.folder)["durable_through"], 4)

    def test_missing_head_fails(self):
        (self.folder / "run-resumed/HEAD").unlink()
        with self.assertRaises(FileNotFoundError):
            verify_folder(self.folder)

    def test_forged_journal_fails(self):
        path = self.folder / "run-resumed/JOURNAL"
        data = bytearray(path.read_bytes())
        data[len(data) // 2] ^= 1
        path.write_bytes(data)
        with self.assertRaises(ValueError):
            verify_folder(self.folder)

    def test_exported_snapshot_does_not_substitute_for_committed_bundle(self):
        (self.folder / "recovery-resumed-3.bin").write_bytes(b"substituted")
        with self.assertRaises(ValueError):
            verify_folder(self.folder)


if __name__ == "__main__":
    unittest.main()
