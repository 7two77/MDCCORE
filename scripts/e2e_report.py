"""Independent structural/evidence validator. It never implements MDC calculus."""
from pathlib import Path
import hashlib
from collections import Counter
import re

DEFAULT_REPORT = "docs/evidence/mdc-e2e-conformance-001-impl-011.json"

CASE_IDS = {f"E2E-A{i:02}" for i in range(1, 13)} | {f"N{i:02}" for i in range(1, 12)}
SEMANTIC = {"E2E-A05": "PASS", "E2E-A06": "UNRESOLVED", "E2E-A07": "FAIL",
            "N04": "UNRESOLVED", "N05": "FAIL", "N07": "FAIL"}
KINDS = {"Observation", "Measurement", "Relation", "Composition", "State", "Transition", "Evaluation"}


def source_fingerprint(root):
    root = Path(root)
    files = [root / "Cargo.toml", root / "Cargo.lock"]
    for folder in ("src", "examples", "tests", "scripts"):
        files.extend(p for p in (root / folder).rglob("*")
                     if p.is_file() and "__pycache__" not in p.parts and p.suffix != ".pyc")
    entries = {str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest()
               for p in sorted(set(files))}
    digest = hashlib.sha256()
    for name, value in sorted(entries.items()):
        digest.update(name.encode() + b"\0" + value.encode() + b"\n")
    return digest.hexdigest(), entries


def node_map(nodes):
    result = {}
    for node in nodes:
        reference = node["reference"]
        match = re.fullmatch(r"([A-Za-z]+)\((\d+)\)", reference)
        if not match or match[1] not in KINDS or int(match[2]) != node["identity"]:
            raise ValueError("invalid semantic node reference/identity")
        encoded = node["basis_hex"]
        if not encoded or bytes.fromhex(encoded).hex() != encoded:
            raise ValueError("invalid canonical bytes")
        if reference in result:
            raise ValueError("duplicate semantic node")
        result[reference] = (node["identity"], encoded)
    return result


def validate_execution(report):
    errors = []
    try:
        if report["fixture_id"] != "MDC-E2E-CONFORMANCE-001" or report["profile"] != "MDC-SCALAR-001":
            errors.append("wrong fixture/profile")
        if report["acceptance"] != "PASS" or report["harness_status"] != "PASS" or report["blockers"]:
            errors.append("execution did not pass")
        cases = report["cases"]
        if len(cases) != len(CASE_IDS) or {c["id"] for c in cases} != CASE_IDS:
            errors.append("missing/duplicate mandatory cases")
        runs = report["runs"]
        if len(runs) != 3 or {r["id"] for r in runs} != {"run-1", "run-2", "run-permuted"}:
            errors.append("missing independent/permuted runs")
        run_names = {r["id"] for r in runs}
        for case in cases:
            cid = case["id"]
            if case["mandatory"] is not True or case["executed"] is not True or case["harness_status"] != "PASS":
                errors.append(f"{cid}: not executed/passed")
            expected = SEMANTIC.get(cid)
            if case["expected"]["semantic_result"] != expected or case["actual"]["semantic_result"] != expected:
                errors.append(f"{cid}: semantic expectation mismatch")
            if not case["evidence_refs"] or not case["evidence"] or not isinstance(case["diagnostics"], list):
                errors.append(f"{cid}: missing evidence")
            for reference in case["evidence_refs"]:
                valid = (reference in run_names or any(reference.startswith(r + "/") or reference.startswith("runs/" + r) for r in run_names)
                         or reference in {"case:" + cid, "cases/" + cid + "/evidence", cid})
                if not valid:
                    errors.append(f"{cid}: unresolved evidence reference")
        graphs = []
        for run in runs:
            graph = node_map(run["canonical_graph"])
            if {r.split("(", 1)[0] for r in graph} != KINDS:
                errors.append(run["id"] + ": missing semantic levels")
            if not any(ref.startswith("Source(") for ref in run["lineage"]):
                errors.append(run["id"] + ": missing source lineage")
            if not set(graph).issubset(set(run["lineage"])):
                errors.append(run["id"] + ": incomplete lineage")
            if not run["operations"] or not run["evaluations"]:
                errors.append(run["id"] + ": missing operation/evaluation evidence")
            counts = Counter(o["operation"] for o in run["operations"])
            if counts != Counter(admit=4, measure=4, relate=2, compose=2, derive_state=2, transition=1, evaluate=1):
                errors.append(run["id"] + ": incomplete downstream operation coverage")
            prefix = {}
            witnessed_nonempty = False
            for operation in run["operations"]:
                before, after = node_map(operation["before"]), node_map(operation["after"])
                witnessed_nonempty |= bool(before)
                if operation["upstream_unchanged"] is not True or before != after:
                    errors.append(run["id"] + ": upstream content changed")
                if before != prefix:
                    errors.append(run["id"] + ": snapshot omitted existing upstream objects")
                output = operation["output"]["reference"]
                if output not in graph or output in prefix:
                    errors.append(run["id"] + ": invalid operation output")
                else:
                    prefix[output] = graph[output]
            if not witnessed_nonempty:
                errors.append(run["id"] + ": vacuous snapshots")
            if prefix != graph:
                errors.append(run["id"] + ": operations did not construct full graph")
            graphs.append((graph, sorted(run["lineage"])))
        if not graphs or any(g != graphs[0] for g in graphs[1:]):
            errors.append("canonical graphs/lineages differ")
        if not report["input"]:
            errors.append("missing declared inputs/contracts")
        if report["input"].get("representation_schema") != "TEST-REP-01/version1":
            errors.append("wrong technical representation schema")
        for cid, reverse in (("E2E-A11", True), ("E2E-A12", False), ("N10", True)):
            case = next(c for c in cases if c["id"] == cid)
            representations = [e for e in case["evidence"] if isinstance(e, dict) and e.get("external_representation") is True]
            observed_graphs = [e for e in case["evidence"] if isinstance(e, dict) and "canonical_graph" in e]
            if len(representations) != 1 or not observed_graphs:
                errors.append(cid + ": missing real projection evidence")
                continue
            projection = representations[0]
            if projection["reverse_attempt"] is not reverse or projection["core_unchanged"] is not True:
                errors.append(cid + ": missing reverse/non-interference observation")
            before, after = projection["before_hex"], projection["after_hex"]
            if not before or not after or (before != after) is not reverse:
                errors.append(cid + ": external mutation/mapping evidence inconsistent")
            if any(node_map(g["canonical_graph"]) != graphs[0][0] for g in observed_graphs):
                errors.append(cid + ": core graph changed after projection")
            transition_bases = [value[1] for ref, value in graphs[0][0].items() if ref.startswith("Transition(")]
            if projection["source_basis_hex"] not in transition_bases:
                errors.append(cid + ": projection source bytes mismatch")
    except (KeyError, TypeError, ValueError, AttributeError, StopIteration) as error:
        errors.append("invalid evidence schema: " + str(error))
    return errors


def validate_bundle(bundle, current_source=None):
    errors = []
    try:
        metadata = bundle["metadata"]
        if current_source is not None and metadata["implementation_source_sha256"] != current_source:
            errors.append("stale implementation fingerprint")
        for key in ("rustc", "cargo", "target_triple", "features", "implementation_source_sha256", "source_files"):
            if key not in metadata or metadata[key] is None:
                errors.append("missing metadata: " + key)
        processes = bundle["processes"]
        if len(processes) < 2:
            errors.append("two separate process executions required")
        for index, process in enumerate(processes):
            if process["exit_code"] != 0:
                errors.append(f"process {index} failed")
            errors.extend(f"process {index}: {e}" for e in validate_execution(process["report"]))
        process_ids = [p["report"]["process_id"] for p in processes]
        if len(set(process_ids)) != len(process_ids) or not all(isinstance(pid, int) and pid > 0 for pid in process_ids):
            errors.append("process executions are not distinct")
        graphs = [(node_map(p["report"]["runs"][0]["canonical_graph"]), sorted(p["report"]["runs"][0]["lineage"])) for p in processes]
        if not graphs or any(g != graphs[0] for g in graphs[1:]):
            errors.append("cross-process canonical graphs differ")
        if bundle["acceptance"] != "PASS" or bundle["harness_status"] != "PASS":
            errors.append("bundle not accepted")
    except (KeyError, TypeError, ValueError, AttributeError, StopIteration) as error:
        errors.append("invalid bundle: " + str(error))
    return errors
