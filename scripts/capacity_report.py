"""Independent integrity/measurement checks. Core semantics stay in Rust."""
import hashlib
import json
from pathlib import Path
import statistics
import struct
from replay_run_report import frame, digest, PREFIX
from e2e_report import source_fingerprint
from ci_metadata import ci_fingerprints

LIMITS = dict(zip(("max_decisions", "max_manifest_bytes", "max_record_bytes", "max_journal_bytes", "max_report_bytes", "max_snapshot_bytes"),
                  (256, 16384, 24 * 1024 * 1024, 64 * 1024 * 1024, 1024 * 1024, 16 * 1024 * 1024)))


def sha(path):
    h = hashlib.sha256()
    with Path(path).open("rb") as f:
        for b in iter(lambda: f.read(1024 * 1024), b""):
            h.update(b)
    return h.hexdigest()


def read_json(path):
    def pairs(items):
        result = {}
        for key, value in items:
            if key in result:
                raise ValueError("duplicate JSON key")
            result[key] = value
        return result
    return json.loads(Path(path).read_text(), object_pairs_hook=pairs,
                      parse_constant=lambda v: (_ for _ in ()).throw(ValueError("nonfinite JSON")))


def local(root, name):
    root = Path(root).resolve()
    path = (root / name).resolve()
    if path == root or root not in path.parents:
        raise ValueError("artifact path escaped output directory")
    return path


def run_files(folder, expected_source, profile,expected_limits=LIMITS):
    """Validate bounded v1 records for any plan, without implementing State."""
    folder = Path(folder)
    manifest = (folder / "MANIFEST").read_bytes()
    source = (folder / "SOURCE").read_bytes()
    head = (folder / "HEAD").read_bytes()
    if len(manifest) > 16384 or len(head) > 512 or source != expected_source:
        raise ValueError("run input/size differs")
    body, _, end = frame(manifest, 0, b"manifest", b"MDC-RMAN-001\0\0\0\0", 16384)
    if end != len(manifest) or len(body) < 112 or struct.unpack_from("<HHB", body) != (1, 1, 1):
        raise ValueError("manifest format")
    source_size, = struct.unpack_from("<Q", body, 5)
    symbol, unit, anchor, lag = struct.unpack_from("<BBBQ", body, 45)
    wanted = (0 if profile["symbol"] == "BTCUSDT" else 1, 0 if profile["unit"] == "ms" else 1,
              0 if profile["anchor"] == "close" else 1, profile["lag_ms"] * 1000)
    if (symbol, unit, anchor, lag) != wanted or source_size != len(source) or body[13:45] != digest(source):
        raise ValueError("manifest source/profile")
    limits = struct.unpack_from("<6Q", body, 56)
    n, = struct.unpack_from("<Q", body, 104)
    if limits != tuple(expected_limits.values()) or not 1 <= n <= 256 or len(body) != 112 + n * 8:
        raise ValueError("limits or plan size changed")
    plan = list(struct.unpack_from("<" + "Q" * n, body, 112))
    if any(a >= b for a, b in zip(plan, plan[1:])):
        raise ValueError("unordered plan")
    run = digest(PREFIX + b"run-id/v1\0" + body)
    h, _, end = frame(head, 0, b"head", b"MDC-RHED-001\0\0\0\0", 256)
    if len(h) != 144 or end != len(head) or h[:32] != run or h[32:64] != digest(manifest):
        raise ValueError("HEAD identity")
    offset, count = struct.unpack_from("<QQ", h, 64)
    if not count <= n or not offset <= expected_limits["max_journal_bytes"] or (folder / "JOURNAL").stat().st_size != offset:
        raise ValueError("certified journal range")
    bundles = []
    latest = bytes(32)
    with (folder / "JOURNAL").open("rb") as stream:
        header = stream.read(124)
        hb, previous, end = frame(header, 0, b"journal", b"MDC-RJNL-001\0\0\0\0", 128)
        if end != 124 or hb != struct.pack("<H", 1) + run + digest(manifest):
            raise ValueError("journal header")
        for index in range(1, count + 1):
            prefix = stream.read(26)
            if len(prefix) != 26:
                raise ValueError("missing record")
            size, = struct.unpack_from("<Q", prefix, 18)
            if size > LIMITS["max_record_bytes"]:
                raise ValueError("record bound")
            data = prefix + stream.read(size + 32)
            b, tip, end = frame(data, 0, b"decision", b"MDC-RDEC-001\0\0\0\0", LIMITS["max_record_bytes"])
            if end != len(data) or len(b) < 192 or b[:32] != run or struct.unpack_from("<QQ", b, 32) != (index, plan[index - 1]) or b[144:176] != previous:
                raise ValueError("record run/order/chain")
            report_len, = struct.unpack_from("<Q", b, 176)
            at = 184 + report_len
            if report_len > LIMITS["max_report_bytes"] or at + 8 > len(b):
                raise ValueError("report length")
            report = b[184:at]
            snapshot_len, = struct.unpack_from("<Q", b, at)
            snapshot = b[at + 8:]
            if snapshot_len > LIMITS["max_snapshot_bytes"] or len(snapshot) != snapshot_len or digest(report) != b[80:112] or digest(snapshot) != b[112:144]:
                raise ValueError("bundle binding")
            info = json.loads(report)
            if (info["availability_quality"] != "modeled" or info["decision_us"] != plan[index - 1]
                    or info["decision_id"] != b[48:80].hex() or info["raw_dataset_sha256"] != digest(source).hex()
                    or info["core_snapshot_sha256"] != digest(snapshot).hex()):
                raise ValueError("decision context")
            key = b"BINANCE-SPOT-1M-MODELED-001/decision/v1\0" + body[45:56] + struct.pack("<QQ", plan[index - 1], len(info["eligible"]))
            for evidence in info["eligible"]:
                basis = bytes.fromhex(evidence["evidence_basis_hex"])
                if evidence["available_us"] > plan[index - 1] or digest(basis).hex() != evidence["evidence_id"]:
                    raise ValueError("future or forged evidence")
                key += struct.pack("<Q", len(basis)) + basis
            if digest(key) != b[48:80]:
                raise ValueError("decision key")
            bundles.append(dict(index=index, cutoff=plan[index - 1], report_bytes=len(report), snapshot_bytes=len(snapshot),
                                report_sha256=digest(report).hex(), snapshot_sha256=digest(snapshot).hex()))
            latest = digest(PREFIX + b"bundle/v1\0" + b[:144])
            previous = tip
        if stream.tell() != offset or stream.read(1) or previous != h[80:112] or latest != h[112:144]:
            raise ValueError("HEAD tip or EOF")
    return dict(run_id=run.hex(), cutoffs=plan, committed=count, bundles=bundles,
                file_sha256={name: sha(folder / name) for name in ("SOURCE", "MANIFEST", "HEAD", "JOURNAL")},
                file_bytes={name: (folder / name).stat().st_size for name in ("SOURCE", "MANIFEST", "HEAD", "JOURNAL")})


def same_files(a, b):
    for name in ("SOURCE", "MANIFEST", "HEAD", "JOURNAL"):
        with (Path(a) / name).open("rb") as left, (Path(b) / name).open("rb") as right:
            while True:
                x, y = left.read(1024 * 1024), right.read(1024 * 1024)
                if x != y:
                    raise ValueError("full retained bytes differ")
                if not x:
                    break


def process_entry(root, entry):
    out = local(root, entry["stdout_file"])
    err = local(root, entry["stderr_file"])
    if sha(out) != entry["stdout_sha256"] or sha(err) != entry["stderr_sha256"]:
        raise ValueError("process artifact digest")
    for key in ("wall_ns", "user_cpu_us", "system_cpu_us", "peak_rss_kib"):
        if type(entry[key]) is not int or entry[key] < 0:
            raise ValueError("invalid resource unit/value")
    if type(entry["timeout"]) is not bool or type(entry["exit_code"]) is not int:
        raise ValueError("process exit/timeout type")


def phase(root, entry):
    process_entry(root, entry)
    out = local(root, entry["stdout_file"])
    if entry["timeout"] or entry["exit_code"] != 0:
        return None
    value = read_json(out)
    if value["schema"] != "MDC-CAPACITY-PHASE-001":
        raise ValueError("phase schema")
    for key, number in value.items():
        if key.endswith("_ns") and (type(number) is not int or not 0 <= number <= entry["wall_ns"] + 10000000):
            raise ValueError("phase clock inconsistent with process wall")
    return value


def summaries(cases):
    result = []
    for requested in sorted({c["requested"] for c in cases}):
        rows = [c for c in cases if c["requested"] == requested]
        metrics = {}
        for key in ("commit_api_ns", "recovery_api_ns", "journal_bytes", "committed"):
            values = [c[key] for c in rows if c.get(key) is not None]
            metrics[key] = None if not values else dict(samples=values, min=min(values), median=statistics.median(values), max=max(values))
        result.append(dict(requested=requested, outcomes=[c["outcome"] for c in rows], first_constraints=[c.get("first_constraint") for c in rows], metrics=metrics))
    return result


def validate(folder):
    folder = Path(folder)
    report = read_json(folder / "capacity-report.json")
    if (report["schema"] != "MDC-CAPACITY-001/1" or type(report["repetitions"]) is not int or not 3 <= report["repetitions"] <= 10
            or not report["points"] or any(type(n) is not int or not 1 <= n <= 4096 for n in report["points"])
            or report["points"] != sorted(set(report["points"]))):
        raise ValueError("matrix/schema/repetitions")
    fingerprint, files = source_fingerprint(Path(__file__).resolve().parents[1])
    if report["build"]["implementation_source_sha256"] != fingerprint or report["build"]["source_files"] != files or report["build"]["ci_file_sha256"] != ci_fingerprints(Path(__file__).resolve().parents[1]):
        raise ValueError("stale implementation fingerprint")
    for binary in report["build"]["binaries"].values():
        if sha(local(folder, binary["file"])) != binary["sha256"]:
            raise ValueError("binary fingerprint")
    process_entry(folder, report["build"]["process"])
    process_entry(folder, report["control"]["process"])
    if any(report[k]["process"]["timeout"] or report[k]["process"]["exit_code"] for k in ("build", "control")):
        raise ValueError("unexecuted build/control")
    if report["build"]["selected_flags"] != {"CARGO_INCREMENTAL": "0", "CARGO_PROFILE_RELEASE_DEBUG": "0", "CARGO_PROFILE_RELEASE_OPT_LEVEL": "3"} or report["build"]["extra_rustflags_present"]:
        raise ValueError("uncontrolled build flags")
    method = report["method"]
    if (method["clock"] != "perf_counter_ns parent / Instant API" or method["child_reaping"] != "wait4"
            or method["cpu_unit"] != "us" or method["rss_unit"] != "KiB" or method["poll_interval_ns"] != 5000000 or method["percentiles"] != "none"):
        raise ValueError("measurement method/units changed")
    if report["source"]["origin"] not in ("provided_csv", "synthetic_fixture") or report["source"]["authenticity"] != "not_attested":
        raise ValueError("source provenance claim")
    source = local(folder, report["source"]["file"]).read_bytes()
    if len(source) > 4 * 1024 * 1024 or len(source) != report["source"]["bytes"] or digest(source).hex() != report["source"]["sha256"]:
        raise ValueError("source identity")
    expected_limits={**LIMITS,"max_journal_bytes":report["limits"]["max_journal_bytes"]}
    if report["build"]["profile"] != "release" or report["profile"]["availability_quality"] != "modeled" or report["limits"] != expected_limits or not 124<=expected_limits["max_journal_bytes"]<=LIMITS["max_journal_bytes"]:
        raise ValueError("build/profile/limits")
    if report["probe_kind"] != ("default_limits" if expected_limits==LIMITS else "explicit_reduced_journal_budget"):
        raise ValueError("probe limits label")
    target=report["build"]["target"]
    native={"x86_64":("x86_64",64,"little"),"aarch64":("aarch64",64,"little"),"i686":("x86",32,"little"),"i386":("x86",32,"little"),"s390x":("s390x",64,"big")}.get(report["host"]["machine"])
    if native is None or target != dict(zip(("arch","pointer_width","endian"),native)) or target["pointer_width"]!=report["host"]["python_pointer_width"] or target["endian"]!=report["host"]["python_byteorder"]:
        raise ValueError("native measurement target mismatch")
    expected = {(n, r) for n in report["points"] for r in range(1, report["repetitions"] + 1)}
    seen, references, normalized = set(), {}, []
    for row in report["cases"]:
        key = (row["requested"], row["repetition"])
        if key in seen or key not in expected:
            raise ValueError("duplicate/unknown matrix cell")
        seen.add(key)
        prefix=f"cases/n{key[0]:04}-r{key[1]:02}"
        if row["directory"] != prefix+"/run" or any(p["stdout_file"]!=prefix+"/"+name+".stdout" or p["stderr_file"]!=prefix+"/"+name+".stderr" for name,p in row["phases"].items()):
            raise ValueError("reused or misplaced sample artifacts")
        create = phase(folder, row["phases"]["create"])
        if create is None or create["phase"] != "create" or create["requested"] != key[0] or create["source_sha256"] != report["source"]["sha256"] or create["limits"] != expected_limits:
            raise ValueError("unverified creation")
        if create["target"]!=target:
            raise ValueError("mixed compiled target")
        if create["replay_limits"]!={"max_input_bytes":4194304,"max_rows":4096,"max_line_bytes":4096,"max_visible_bars":256}:
            raise ValueError("replay limits changed")
        if len(create["cutoffs"]) != key[0]:
            raise ValueError("insufficient normalized source rows")
        run = local(folder, row["directory"])
        if create["outcome"] == "PreflightRejected":
            if run.exists() or row["outcome"] != "PreflightRejected" or row["committed"] != 0 or key[0] <= LIMITS["max_decisions"] or row["first_constraint"]!=create.get("error",create.get("error_class")):
                raise ValueError("unexpected preflight rejection")
        else:
            commit = phase(folder, row["phases"]["commit"])
            recover = phase(folder, row["phases"]["recover"])
            if commit is None or recover is None or create["outcome"] != "Created" or recover["outcome"] != "Recovered":
                raise ValueError("incomplete measurement cannot earn PASS")
            if commit["phase"] != "commit" or recover["phase"] != "recover":
                raise ValueError("mixed measurement phases")
            if create["committed"]!=0 or commit["target"]!=target or recover["target"]!=target:
                raise ValueError("non-fresh or mixed-target run")
            real = run_files(run, source, report["profile"],expected_limits)
            if real["cutoffs"] != create["cutoffs"] or real["committed"] != row["committed"] or real["bundles"] != recover["bundles"] or real["file_sha256"] != row["file_sha256"] or real["file_bytes"] != row["file_bytes"]:
                raise ValueError("reported run differs from actual files")
            if commit["committed"] != real["committed"] or commit["cutoffs"] != real["cutoffs"] or recover["run_id"] != real["run_id"]:
                raise ValueError("cursor/run binding")
            next_index=None if real["committed"]==key[0] else real["committed"]+1
            if commit["next_index"]!=next_index or recover["next_index"]!=next_index or row["first_constraint"]!=commit.get("error"):
                raise ValueError("next cursor/constraint differs")
            steps = commit["steps"]
            if [s["index"] for s in steps] != list(range(1, len(steps) + 1)):
                raise ValueError("step sequence")
            if commit["outcome"] == "Completed":
                if real["committed"] != key[0] or len(steps) != key[0] or any(s["outcome"] != "Committed" for s in steps):
                    raise ValueError("incomplete completion")
            elif commit["outcome"] == "Limited":
                if not (commit["failed_head_unchanged"] and commit["failed_journal_length_unchanged"] and commit["error_class"] == "Limit"
                        and real["committed"] < key[0] and commit["failed_index"] == real["committed"] + 1
                        and len(steps) == real["committed"] + 1 and steps[-1]["outcome"] == "Limited" and all(s["outcome"]=="Committed" for s in steps[:-1])):
                    raise ValueError("invalid retained-prefix limit")
            else:
                raise ValueError("unexpected measurement outcome")
            if row["outcome"] != commit["outcome"] or row["journal_bytes"] != real["file_bytes"]["JOURNAL"] or row["commit_api_ns"] != sum(s["api_ns"] for s in steps) or row["recovery_api_ns"] != recover["open_ns"]:
                raise ValueError("metric summary differs")
            if any(type(s["api_ns"]) is not int or s["api_ns"] < 0 for s in steps) or row["commit_api_ns"] > row["phases"]["commit"]["wall_ns"]:
                raise ValueError("invalid step clocks")
            retry = phase(folder, row["phases"]["retry"]) if real["committed"] else None
            if real["committed"] and (retry is None or retry["outcome"] != "Retried" or not retry["retry_head_unchanged"] or not retry["retry_journal_length_unchanged"] or retry["committed"] != real["committed"]):
                raise ValueError("retry changed prefix")
            if key[0] in references:
                same_files(references[key[0]], run)
            else:
                references[key[0]] = run
        normalized.append(row)
    if seen != expected or report["summary"] != summaries(normalized) or report["status"] != "COMPLETE":
        raise ValueError("missing matrix/biased summary/incomplete report")
    control = read_json(local(folder, report["control"]["report_file"]))
    snapshot = local(folder, report["control"]["snapshot_file"])
    fixture = Path(__file__).resolve().parents[1] / "adapters/mdc_replay/tests/fixtures/control-v1.json"
    if control != read_json(fixture)["expected_report"] or sha(snapshot) != control["core_snapshot_sha256"]:
        raise ValueError("release control changed")
    return dict(acceptance="PASS", source_sha256=report["source"]["sha256"], points=report["points"],
                repetitions=report["repetitions"], cases=len(seen), outcomes=[r["outcome"] for r in normalized])
