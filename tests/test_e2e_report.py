"""Adversarial checks of the independent evidence validator using actual output.
Run scripts/run_e2e.py first to create the input evidence.
"""
import copy
import json
from pathlib import Path
import sys
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
from e2e_report import DEFAULT_REPORT, source_fingerprint, validate_bundle  # noqa: E402


class EvidenceAcceptanceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.good = json.loads((ROOT / DEFAULT_REPORT).read_text())

    def reject(self, mutate):
        bundle = copy.deepcopy(self.good)
        mutate(bundle["processes"][0]["report"])
        self.assertTrue(validate_bundle(bundle), "forged PASS must be rejected")

    def test_actual_report_matches_the_current_implementation(self):
        self.assertEqual(validate_bundle(self.good, source_fingerprint(ROOT)[0]), [])

    def test_missing_case(self):
        self.reject(lambda r: r["cases"].pop())

    def test_duplicate_case(self):
        self.reject(lambda r: r["cases"].__setitem__(1, copy.deepcopy(r["cases"][0])))

    def test_unexecuted_pass(self):
        self.reject(lambda r: r["cases"][0].update(executed=False))

    def test_failed_case_despite_top_level_pass(self):
        self.reject(lambda r: r["cases"][0].update(harness_status="FAIL"))

    def test_wrong_semantic_result(self):
        def mutate(r):
            next(c for c in r["cases"] if c["id"] == "E2E-A07")["actual"]["semantic_result"] = "UNRESOLVED"
        self.reject(mutate)

    def test_empty_evidence(self):
        self.reject(lambda r: r["cases"][0].update(evidence=[]))

    def test_unresolvable_evidence_ref(self):
        self.reject(lambda r: r["cases"][0].update(evidence_refs=["nonexistent-run"]))

    def test_missing_independent_run(self):
        self.reject(lambda r: r["runs"].pop())

    def test_changed_graph_with_same_ids(self):
        self.reject(lambda r: r["runs"][1]["canonical_graph"][0].update(basis_hex="00"))

    def test_false_upstream_unchanged_marker(self):
        def mutate(r):
            operation = next(o for o in r["runs"][0]["operations"] if o["before"])
            operation["after"][0]["basis_hex"] = "00"
        self.reject(mutate)

    def test_missing_source_lineage(self):
        def mutate(r):
            for run in r["runs"]:
                run["lineage"] = [ref for ref in run["lineage"] if not ref.startswith("Source(")]
        self.reject(mutate)

    def test_projection_artifact_cannot_be_omitted(self):
        def mutate(r):
            case = next(c for c in r["cases"] if c["id"] == "E2E-A12")
            case["evidence"] = [e for e in case["evidence"] if not e.get("external_representation")]
        self.reject(mutate)

    def test_missing_downstream_operation(self):
        self.reject(lambda r: r["runs"][0]["operations"].pop())

    def test_missing_upstream_prefix(self):
        def mutate(r):
            op = next(o for o in r["runs"][0]["operations"] if len(o["before"]) > 2)
            op["before"].pop()
            op["after"].pop()
        self.reject(mutate)

    def test_stale_source_fingerprint(self):
        self.assertTrue(validate_bundle(self.good, "0" * 64))

    def test_process_evidence_cannot_be_reused_as_two_processes(self):
        bundle = copy.deepcopy(self.good)
        bundle["processes"][1] = copy.deepcopy(bundle["processes"][0])
        self.assertTrue(validate_bundle(bundle))


if __name__ == "__main__":
    unittest.main()
