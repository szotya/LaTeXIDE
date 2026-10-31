"""Read-only freshness checks. Reports are observations, not authenticated attestations."""
import hashlib
import json
from pathlib import Path
import re


def read_json(path, limit=8 * 1024 * 1024):
    def unique(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise ValueError("Duplicate JSON field")
            result[key] = value
        return result
    with Path(path).open("rb") as stream:
        raw = stream.read(limit + 1)
    if len(raw) > limit:
        raise ValueError("Evidence input quota exceeded")
    return json.loads(raw, object_pairs_hook=unique)


def digest(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def validate_summary(data, suite, allowed_skips=(), run_id=None, allow_failures=False):
    if not isinstance(data, dict) or data.get("schema") != 1 or data.get("suite") != suite: raise ValueError("Missing or wrong test summary")
    if run_id is not None and data.get("verification_run_id") != run_id: raise ValueError("Stale test summary")
    counts = [data.get(name) for name in ["tests", "passed", "skipped", "failures", "errors"]]
    if any(type(n) is not int or n < 0 for n in counts): raise ValueError("Invalid test counts")
    tests, passed, skipped, failures, errors = counts
    failed_tests = data.get("failed_tests", failures + errors); skipped_tests = data.get("skipped_tests", skipped)
    expected = data.get("expected_failures", 0); unexpected = data.get("unexpected_successes", 0)
    if any(type(n) is not int or n < 0 for n in [failed_tests, skipped_tests, expected, unexpected]): raise ValueError("Invalid case outcome counts")
    ids = data.get("skipped_ids")
    if not isinstance(ids, list) or any(not isinstance(i, str) for i in ids) or len(ids) != skipped or len(set(ids)) != skipped: raise ValueError("Missing skipped identities")
    if tests <= 0 or tests != passed + skipped_tests + failed_tests + expected: raise ValueError("Inconsistent test case outcomes")
    if not allow_failures and (passed <= 0 or failures or errors or failed_tests or expected or unexpected): raise ValueError("Required tests did not pass")
    if not allow_failures and not set(ids).issubset(set(allowed_skips)): raise ValueError("Unqualified test skip")
    return data


def rust_summary(log, suite, allow_failures=False):
    if Path(log).stat().st_size > 16 * 1024 * 1024: raise ValueError("Test summary log quota exceeded")
    text = Path(log).read_text(encoding="utf-8", errors="replace")
    rows = re.findall(r"test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;", text)
    if not rows: raise ValueError("Missing Rust test summary")
    passed = sum(int(r[1]) for r in rows); failures = sum(int(r[2]) for r in rows); skipped = sum(int(r[3]) for r in rows)
    ids = re.findall(r"^test (\S+) \.\.\. ignored", text, flags=re.M)
    return validate_summary({"schema": 1, "suite": suite, "tests": passed + failures + skipped, "passed": passed,
                             "skipped": skipped, "failures": failures, "errors": 0, "skipped_ids": ids}, suite, allow_failures=allow_failures)


def revalidate_report(root, report, current_source, expected_steps, artifact_roots=()):
    """Compare observations with current bytes and caller-owned check definitions."""
    root = Path(root).resolve()
    allowed = [root, *(Path(path).resolve() for path in artifact_roots)]
    identities = dict.fromkeys(["source", "selection", "artifacts", "logs", "tests", "descriptions"], "unrun")

    def result(status, reason=None):
        return {"schema": 1, "status": status, "identities": identities,
                "source_sha256": current_source.get("sha256"), "reason": reason,
                "scope": "stored evidence freshness; no execution, authenticity or provider acceptance"}

    def output(name):
        if not isinstance(name, str):
            raise ValueError("Missing output path")
        path = root / name
        if not any(path.resolve().is_relative_to(base) for base in allowed) or not path.is_file():
            raise ValueError("Output is missing or outside declared roots")
        return path

    try:
        if not isinstance(report, dict) or report.get("schema") != 1:
            return result("invalid_report", "Unknown report schema")
        if report.get("status") != "passed":
            return result("failed_report", "The original run did not pass")
        source = current_source["sha256"]
        if any(report.get(k) != source for k in ["source_sha256", "source_sha256_after"]) or report.get("source_files") != current_source["files"]:
            identities["source"] = "stale_source"
            return result("stale_source", "Source set or bytes changed")
        identities["source"] = "current"
        records = report.get("checks")
        expected_names = [s["name"] for s in expected_steps]
        if not expected_names or not isinstance(records, list) or [r["name"] for r in records] != expected_names or len(set(expected_names)) != len(expected_names):
            return result("invalid_report", "Check selection is missing, duplicated or changed")
        known = {}; descriptions = []
        for sequence, (step, row) in enumerate(zip(expected_steps, records), 1):
            required = step.get("required", True)
            if type(row.get("required")) is not bool or row["required"] != required or row.get("kind") != step["kind"] or type(row.get("sequence")) is not int or row["sequence"] != sequence:
                return result("invalid_report", "Trusted check contract differs")
            if row.get("source_sha256") != source or row.get("report_status") != "passed":
                return result("invalid_report", "Check is bound to a different run result")
            run_id = row.get("verification_run_id")
            if not isinstance(run_id, str) or not re.fullmatch(r"[0-9a-f]{32}", run_id):
                return result("invalid_report", "Missing check run identity")
            if step.get("argv") is None:
                if required or row.get("status") != "unrun" or row.get("exit_code") is not None:
                    return result("invalid_report", "Unselected or required-unrun check was promoted")
                known[row["name"]] = row
                continue
            if row.get("status") != "passed" or type(row.get("exit_code")) is not int or row["exit_code"] != 0 or row.get("timed_out") is not False:
                return result("invalid_report", "Selected check did not pass")
            if any(known.get(name, {}).get("status") != "passed" for name in step.get("depends_on", [])):
                return result("invalid_report", "Check dependency did not pass")
            log = row.get("log", {})
            try:
                if log.get("kind") != "command_log" or digest(output(log.get("path"))) != log.get("sha256"):
                    raise ValueError("Log changed")
            except (OSError, ValueError):
                identities["logs"] = "stale_log"
                return result("stale_log", row["name"])
            identities["logs"] = "current"
            artifacts = row.get("artifacts", [])
            wanted = list(step.get("artifact_paths", []))
            if step.get("artifact_path"):
                wanted.append({"path": step["artifact_path"], "kind": step.get("artifact_kind", "test_artifact")})
            kinds = set(step.get("required_artifact_kinds", [])) | {a["kind"] for a in wanted}
            if not isinstance(artifacts, list) or len(artifacts) > 32 or (step.get("artifact_index") and not artifacts):
                return result("invalid_report", "Artifact set missing or too large")
            if not kinds.issubset({a["kind"] for a in artifacts}) or any(not any(a["path"] == w["path"] and a["kind"] == w["kind"] for a in artifacts) for w in wanted):
                return result("invalid_report", "Required artifact omitted")
            if artifacts:
                try:
                    paths = [output(a["path"]).resolve() for a in artifacts]
                    if len(set(paths)) != len(paths) or row.get("artifact_status") != "current" or any(digest(path) != a["sha256"] for path, a in zip(paths, artifacts)):
                        raise ValueError("Artifact changed")
                except (OSError, ValueError):
                    identities["artifacts"] = "stale_artifact"
                    return result("stale_artifact", row["name"])
                aggregate = artifacts[0]["sha256"] if len(artifacts) == 1 else hashlib.sha256(json.dumps(artifacts, sort_keys=True).encode()).hexdigest()
                if row.get("artifact_sha256") != aggregate:
                    return result("invalid_report", "Artifact-set digest differs")
                identities["artifacts"] = "current"
            consumed = []
            legacy = step.get("consumes_artifact")
            producers = list(step.get("consumes_artifacts", [])) + ([legacy["producer"]] if legacy else [])
            for name in sorted(set(producers)):
                producer = known.get(name, {})
                selected = producer.get("artifacts", [])
                if legacy and name == legacy["producer"]:
                    selected = [a for a in selected if output(a["path"]).resolve() == output(legacy["path"]).resolve()]
                if producer.get("status") != "passed" or not selected:
                    return result("invalid_report", "Producer evidence missing")
                consumed.extend({**a, "producer": name} for a in selected)
            if row.get("consumed_artifacts", []) != consumed:
                return result("invalid_report", "Consumed artifact set differs")
            try:
                summary = None
                if step.get("summary_path"):
                    summary = validate_summary(read_json(output(step["summary_path"])), step["summary_suite"], step.get("allowed_skips", []), run_id)
                elif step.get("summary_format") == "rust":
                    summary = rust_summary(output(log["path"]), row["name"])
                if row.get("test_summary") != summary:
                    raise ValueError("Stored test outcome differs")
                if summary is not None:
                    identities["tests"] = "current"
            except (OSError, ValueError):
                return result("invalid_test_summary", row["name"])
            if step.get("documentation_path"):
                data = read_json(output(step["documentation_path"]))
                source_hashes = {f["path"]: f["sha256"] for f in current_source["files"]}
                if data != row.get("description_identity") or data.get("verification_run_id") != run_id or not data.get("files") or any(source_hashes.get(f["path"]) != f["sha256"] for f in data["files"]):
                    return result("invalid_report", "Description identity differs")
                descriptions.append(data); identities["descriptions"] = "current"
            known[row["name"]] = row
        if report.get("description_identity", []) != descriptions:
            return result("invalid_report", "Top-level description identity differs")
        for row in records:
            try:
                if any(digest(output(a["path"])) != a["sha256"] for a in row.get("artifacts", [])):
                    raise ValueError("Late artifact change")
            except (OSError, ValueError):
                identities["artifacts"] = "stale_artifact"
                return result("stale_artifact", row["name"])
            if row.get("log"):
                try:
                    if digest(output(row["log"]["path"])) != row["log"]["sha256"]:
                        raise ValueError("Late log change")
                except (OSError, ValueError):
                    identities["logs"] = "stale_log"
                    return result("stale_log", row["name"])
        if any(digest(root / f["path"]) != f["sha256"] for f in current_source["files"]):
            identities["source"] = "stale_source"
            return result("stale_source", "Input bytes changed during revalidation")
        identities["selection"] = "current"
        return result("current")
    except (OSError, ValueError, KeyError, TypeError, AttributeError) as error:
        return result("invalid_report", str(error))
