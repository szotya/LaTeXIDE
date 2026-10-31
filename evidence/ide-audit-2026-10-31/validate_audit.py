"""Validate cross-file audit data, prerequisites, local references and evidence."""
from pathlib import Path
from collections import Counter
from datetime import datetime, timezone
import csv
import hashlib
import json
import re

ROOT = Path(__file__).resolve().parents[2]
DATA = ROOT / "docs/ide-audit-2026-10-31"
EVIDENCE = Path(__file__).resolve().parent


def read_json(path):
    return json.loads(path.read_text())


def main():
    backlog = read_json(DATA / "backlog.json")
    sources = read_json(DATA / "sources.json")
    comparison = read_json(DATA / "comparison.json")
    counts = read_json(DATA / "counts.json")
    by_id = {item["id"]: item for item in backlog}
    source_ids = {item["id"] for item in sources}
    assert len(backlog) == len(by_id) == counts["total"] == 226
    assert len(sources) == len(source_ids) == counts["sources"] == 51
    assert len(comparison) == len({item["id"] for item in comparison}) == 50
    assert len({item["url"] for item in sources}) == len(sources)
    assert dict(Counter(item["priority"] for item in backlog)) == counts["priority"]
    assert dict(Counter(item["observed_status"] for item in backlog)) == counts["observed_status"]
    assert dict(Counter(item["scope"] for item in backlog)) == counts["scope"]
    visiting, visited = set(), set()

    def visit(identifier):
        assert identifier not in visiting, f"dependency cycle: {identifier}"
        if identifier in visited:
            return
        visiting.add(identifier)
        for prerequisite in by_id[identifier]["prerequisites"]:
            assert prerequisite in by_id, (identifier, prerequisite)
            visit(prerequisite)
        visiting.remove(identifier)
        visited.add(identifier)

    for item in backlog:
        for field in ["current_boundary", "proposed_change", "acceptance", "platform_contract"]:
            assert item[field].strip(), (item["id"], field)
        assert item["proposed_state"] == "planned"
        assert set(item["source_ids"]) <= source_ids
        assert all((ROOT / path).exists() for path in item["paths"]), item["id"]
        visit(item["id"])
    for item in comparison:
        assert set(item["backlog_ids"]) <= by_id.keys()
        assert set(item["source_ids"]) <= source_ids
    for name, items in [("backlog", backlog), ("comparison", comparison)]:
        with (DATA / (name + ".csv")).open(encoding="utf-8-sig", newline="") as handle:
            rows = list(csv.DictReader(handle))
        assert len(rows) == len(items)
        assert [row["id"] for row in rows] == [item["id"] for item in items]
    markdown_backlog = (DATA / "IMPLEMENTATION_BACKLOG.md").read_text()
    rows = re.findall(r'<a id="([a-z]+-\d+)"', markdown_backlog)
    assert len(rows) == len(set(rows)) == len(backlog)
    assert set(rows) == {identifier.lower() for identifier in by_id}
    files = [ROOT / "AGENTS.md", ROOT / "README.md", *sorted((ROOT / "docs").rglob("*.md")),
             *sorted((ROOT / "skills/latexide-platform-development").rglob("*.md"))]
    broken, references = [], 0
    for file in files:
        text = re.sub(r"```.*?```", "", file.read_text(), flags=re.S)
        for target in re.findall(r"!?\[[^\]\n]*\]\(([^\s)]+)(?:\s+[^)]*)?\)", text):
            if target.startswith(("https://", "http://", "mailto:", "#", "<repo>", "codex:")):
                continue
            target = target.split("#", 1)[0]
            if not target:
                continue
            references += 1
            if not (file.parent / target).exists():
                broken.append({"file": str(file.relative_to(ROOT)), "target": target})
    assert not broken, broken
    rust_log = (EVIDENCE / "rust-tests.log").read_text()
    rust_results = re.findall(r"test result: ok\. (\d+) passed; (\d+) failed", rust_log)
    assert sum(int(passed) for passed, _ in rust_results) == 56
    assert not any(int(failed) for _, failed in rust_results)
    native = read_json(EVIDENCE / "swift-tests-final-result.json")
    assert native["exit_code"] == 0 and native["inputs_changed_during_run"] == []
    assert "Executed 59 tests, with 4 tests skipped and 0 failures" in (EVIDENCE / "swift-tests-final.log").read_text()
    probes = read_json(EVIDENCE / "audit-probe-results.json")
    assert [item["id"] for item in probes["observations"]] == [f"OBS-{n:02}" for n in range(1, 8)]
    for item in read_json(EVIDENCE / "verification-results.json"):
        if item["name"] != "swift-tests":
            assert item.get("exit_code") == 0, item
    for item in read_json(EVIDENCE / "ipc-enabled-verification-results.json"):
        assert item.get("exit_code") == 0, item
    expected_inputs = read_json(EVIDENCE / "native-verification-inputs.json")
    production_changes = []
    for path, digest in expected_inputs.items():
        if "/Tests/" in path:
            continue
        file = ROOT / path
        if not file.exists() or hashlib.sha256(file.read_bytes()).hexdigest() != digest:
            production_changes.append(path)
    # A new definition-navigation increment appeared after the audited test run.
    # Preserve this as an explicit evidence boundary, never as a current-source PASS.
    if production_changes:
        main_report = (ROOT / "docs/IDE_AUDIT_2026-10-31.md").read_text()
        assert "Frissességi kiegészítés a lezáráskor" in main_report
        assert by_id["LANG-06"]["observed_status"] == "partial"
        assert (EVIDENCE / "snapshot-delta.json").exists()
    report = dict(validated_at_utc=datetime.now(timezone.utc).isoformat(), backlog_items=len(backlog),
                  domains=len({item["domain"] for item in backlog}), comparison_rows=len(comparison),
                  primary_sources=len(sources), dependency_graph="acyclic", csv_json_markdown="consistent",
                  markdown_documents=len(files), relative_targets=references, broken_relative_links=broken,
                  audit_rust_tests_passed=56, own_final_native=dict(executed=59, passed=55, skipped=4, failed=0),
                  reproduced_observations=len(probes["observations"]),
                  production_inputs_changed_since_final_native=production_changes,
                  current_source_qualification="Later definition-navigation changes are source-only/unrun in this audit; recorded test results apply to their hashed inputs",
                  scope="Artifact/data/evidence checks, not an additional platform runtime or agent onboarding test")
    (EVIDENCE / "artifact-validation.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps(report, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
