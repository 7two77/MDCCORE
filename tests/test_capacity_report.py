import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
import hashlib
import io
import zipfile

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
from capacity_report import read_json, sha, validate
from run_capacity import invoke
from fetch_capacity_source import extract


class CapacityReportTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / "report"
        shutil.copytree(Path(os.environ["MDC_CAPACITY_REPORT_DIR"]), self.root)
        self.report = read_json(self.root / "capacity-report.json")

    def save(self):
        (self.root / "capacity-report.json").write_text(json.dumps(self.report))

    def reject(self, mutate):
        mutate()
        self.save()
        with self.assertRaises((ValueError, OSError, KeyError, TypeError)):
            validate(self.root)

    def test_real_complete_measurement_matrix_passes(self):
        self.assertEqual(validate(self.root)["cases"], len(self.report["points"]) * self.report["repetitions"])

    def test_missing_matrix_cell_fails(self):
        self.reject(lambda: self.report["cases"].pop())

    def test_duplicate_cell_fails(self):
        self.reject(lambda: self.report["cases"].append(self.report["cases"][0]))

    def test_stale_source_fingerprint_fails(self):
        self.reject(lambda: self.report["build"].update(implementation_source_sha256="0" * 64))

    def test_changed_binary_fails(self):
        binary = self.root / self.report["build"]["binaries"]["mdc-replay-capacity"]["file"]
        self.reject(lambda: binary.write_bytes(binary.read_bytes() + b"x"))

    def test_changed_input_fails(self):
        self.reject(lambda: (self.root / "source.csv").write_bytes(b"other"))

    def test_forged_summary_fails(self):
        self.reject(lambda: self.report["summary"].pop())

    def test_no_failed_sample_may_be_hidden(self):
        self.reject(lambda: self.report["summary"][-1].update(outcomes=["Completed"] * self.report["repetitions"]))

    def test_wrong_resource_units_fail(self):
        self.reject(lambda: self.report["method"].update(rss_unit="bytes"))

    def test_non_release_build_claim_fails(self):
        self.reject(lambda: self.report["build"].update(profile="debug"))

    def test_modified_limits_fail(self):
        self.reject(lambda: self.report["limits"].update(max_journal_bytes=1024 * 1024 * 1024))

    def test_measured_availability_claim_fails(self):
        self.reject(lambda: self.report["profile"].update(availability_quality="measured"))

    def test_unattested_official_origin_claim_fails(self):
        self.reject(lambda: self.report["source"].update(origin="verified_official"))

    def test_incomplete_report_never_passes(self):
        self.reject(lambda: self.report.update(status="INCOMPLETE"))

    def test_timeout_never_becomes_a_completed_measurement(self):
        self.reject(lambda: self.report["cases"][0]["phases"]["commit"].update(timeout=True))

    def test_negative_cpu_resource_fails(self):
        self.reject(lambda: self.report["cases"][0]["phases"]["create"].update(user_cpu_us=-1))

    def test_stdout_digest_is_checked(self):
        entry = self.report["cases"][0]["phases"]["create"]
        self.reject(lambda: (self.root / entry["stdout_file"]).write_text("{}"))

    def test_resealed_impossible_clock_fails(self):
        entry = self.report["cases"][0]["phases"]["create"]
        path = self.root / entry["stdout_file"]
        value = read_json(path)
        value["input_ns"] = -1
        path.write_text(json.dumps(value))
        entry["stdout_sha256"] = sha(path)
        self.save()
        with self.assertRaises(ValueError):
            validate(self.root)

    def test_omitted_committed_prefix_fails(self):
        self.reject(lambda: self.report["cases"][0].update(committed=0))

    def test_head_is_required(self):
        self.reject(lambda: (self.root / self.report["cases"][0]["directory"] / "HEAD").unlink())

    def test_corrupt_journal_fails(self):
        path = self.root / self.report["cases"][0]["directory"] / "JOURNAL"
        self.reject(lambda: path.write_bytes(path.read_bytes() + b"uncommitted"))

    def test_preflight_rejection_cannot_claim_full_completion(self):
        self.reject(lambda: self.report["cases"][-1].update(outcome="Completed"))

    def test_artifact_paths_cannot_escape(self):
        self.reject(lambda: self.report["build"]["binaries"]["mdc-replay-capacity"].update(file="../elsewhere"))

    def test_duplicate_json_keys_fail(self):
        path = self.root / "capacity-report.json"
        path.write_text(path.read_text().replace('{', '{"schema":"forged",', 1))
        with self.assertRaises(ValueError):
            validate(self.root)

    def test_existing_output_is_not_modified(self):
        path = self.root / "capacity-report.json"
        before = path.read_bytes()
        r = subprocess.run([sys.executable, "-B", str(ROOT / "scripts/run_capacity.py"), "--synthetic-day", "--points", "1,2,3,1440", "--output", str(self.root)], cwd=ROOT, capture_output=True, timeout=30)
        self.assertNotEqual(r.returncode, 0)
        self.assertEqual(path.read_bytes(), before)

    def test_bad_source_hash_creates_no_output(self):
        out = Path(self.temp.name) / "no-output"
        r = subprocess.run([sys.executable, "-B", str(ROOT / "scripts/run_capacity.py"), "--source", str(self.root / "source.csv"), "--expected-source-sha256", "0" * 64, "--output", str(out)], cwd=ROOT, capture_output=True, timeout=30)
        self.assertNotEqual(r.returncode, 0)
        self.assertFalse(out.exists())

    def test_duplicate_points_create_no_output(self):
        out = Path(self.temp.name) / "no-output"
        r = subprocess.run([sys.executable, "-B", str(ROOT / "scripts/run_capacity.py"), "--synthetic-day", "--points", "1,1", "--output", str(out)], cwd=ROOT, capture_output=True, timeout=30)
        self.assertNotEqual(r.returncode, 0)
        self.assertFalse(out.exists())

    def test_timeout_kills_and_reaps_the_owned_child(self):
        root = Path(self.temp.name) / "timeout"
        root.mkdir()
        record = invoke([sys.executable, "-c", "import time; time.sleep(10)"], root, "child", 0.05)
        self.assertTrue(record["timeout"])
        self.assertEqual(record["exit_code"], -9)
        self.assertGreater(record["wall_ns"], 0)

    def test_reusing_another_repetition_directory_fails(self):
        self.reject(lambda: self.report["cases"][1].update(directory=self.report["cases"][0]["directory"]))

    def test_changed_ci_configuration_identity_fails(self):
        self.reject(lambda: self.report["build"].update(ci_file_sha256={}))

    def test_explicit_reduced_budget_probe_retains_a_valid_prefix(self):
        folder = Path(os.environ["MDC_CAPACITY_LIMIT_REPORT_DIR"])
        checked = validate(folder)
        report = read_json(folder / "capacity-report.json")
        self.assertEqual(checked["acceptance"], "PASS")
        self.assertTrue(any(c["outcome"] == "Limited" and c["committed"] > 0 for c in report["cases"]))


class AcquisitionTests(unittest.TestCase):
    def archive(self, extra=False):
        data = b"synthetic-unit-fixture\n"
        b = io.BytesIO()
        with zipfile.ZipFile(b, "w") as z:
            z.writestr("BTCUSDT-1m-2024-01-01.csv", data)
            if extra:
                z.writestr("../unexpected", b"x")
        raw = b.getvalue()
        return raw, dict(day="2024-01-01", rows=1, archive_sha256=hashlib.sha256(raw).hexdigest(), csv_sha256=hashlib.sha256(data).hexdigest())

    def test_pinned_archive_and_raw_content_match(self):
        raw, record = self.archive()
        self.assertEqual(extract(raw, record), b"synthetic-unit-fixture\n")

    def test_bad_archive_digest_fails(self):
        raw, record = self.archive()
        record["archive_sha256"] = "0" * 64
        with self.assertRaises(ValueError):
            extract(raw, record)

    def test_bad_csv_digest_fails(self):
        raw, record = self.archive()
        record["csv_sha256"] = "0" * 64
        with self.assertRaises(ValueError):
            extract(raw, record)

    def test_extra_or_escaping_member_is_not_extracted(self):
        raw, record = self.archive(extra=True)
        with self.assertRaises(ValueError):
            extract(raw, record)


if __name__ == "__main__":
    unittest.main()
