"""Cross-target comparison receipts must describe the verified graph bytes."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
from check_portable_identity import compare, source_fingerprint  # noqa: E402


class ComparisonEvidenceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        supplied = os.environ.get("MDC_PORTABLE_LOG")
        cls.data = (Path(supplied).read_bytes() if supplied else subprocess.check_output(
            ["cargo", "run", "--quiet", "--locked", "--offline", "--example", "portable_identity"], cwd=ROOT))
        cls.fingerprint, cls.files = source_fingerprint(ROOT)
        cls.commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.environment = patch.dict(os.environ, {"GITHUB_RUN_ID": "comparison-unit-fixture"})
        self.environment.start()
        self.addCleanup(self.environment.stop)
        self.folders = []
        for rust in ("1.85.1", "1.98.0"):
            for profile, arch, width, endian in [
                ("default", "x86_64", 64, "little"),
                ("std-alloc", "x86_64", 64, "little"),
                ("alloc", "x86_64", 64, "little"),
                ("portable-i686-unknown-linux-gnu", "x86", 32, "little"),
                ("portable-s390x-unknown-linux-gnu", "s390x", 64, "big"),
            ]:
                folder = self.root / f"{rust}-{profile}"
                folder.mkdir()
                self.folders.append(folder)
                for name in ("portable-1.log", "portable-2.log"):
                    (folder / name).write_bytes(self.data)
                metadata = {"status": "PASS", "worktree_dirty": False,
                            "implementation_source_sha256": self.fingerprint,
                            "source_files": self.files,
                            "ci_file_sha256": {".github/workflows/ci.yml": hashlib.sha256((ROOT / ".github/workflows/ci.yml").read_bytes()).hexdigest()},
                            "rustc": f"rustc {rust} (unit fixture)", "profile": profile,
                            "commit_sha": self.commit, "github_run_id": "comparison-unit-fixture"}
                (folder / "metadata.json").write_text(json.dumps(metadata))
                (folder / "target.json").write_text(json.dumps({"arch": arch, "pointer_width": width, "endian": endian}))

    def test_summary_reports_the_portable_bytes_digest_instead_of_a_ci_file_digest(self):
        result = compare(self.root)
        self.assertEqual(result["acceptance"], "PASS")
        self.assertEqual(len(result["configurations"]), 10)
        self.assertEqual({r["portable_sha256"] for r in result["configurations"]},
                         {hashlib.sha256(self.data).hexdigest()})

    def test_missing_matrix_member_is_rejected(self):
        shutil.rmtree(self.folders[0])
        with self.assertRaisesRegex(ValueError, "missing matrix"):
            compare(self.root)

    def test_duplicate_matrix_member_is_rejected(self):
        shutil.copytree(self.folders[0], self.root / "duplicate")
        with self.assertRaisesRegex(ValueError, "duplicate matrix"):
            compare(self.root)

    def test_wrong_checkout_run_source_and_target_are_rejected(self):
        path = self.folders[0] / "metadata.json"
        original = json.loads(path.read_text())
        for key, value in [("commit_sha", "another-commit"), ("github_run_id", "another-run"),
                           ("implementation_source_sha256", "stale"), ("worktree_dirty", True)]:
            with self.subTest(key=key):
                path.write_text(json.dumps({**original, key: value}))
                with self.assertRaises(ValueError):
                    compare(self.root)
        path.write_text(json.dumps(original))
        (self.folders[0] / "target.json").write_text(json.dumps({"arch": "x86_64", "pointer_width": 32, "endian": "big"}))
        with self.assertRaisesRegex(ValueError, "target"):
            compare(self.root)


if __name__ == "__main__":
    unittest.main()
