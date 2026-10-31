#!/usr/bin/env python3
"""Portable checks for package coverage, ownership and generated-plan integrity."""
from __future__ import annotations

import csv
import hashlib
import io
import json
import re
import sys
from pathlib import Path

from build_development_packages import AUDIT, DATA, ROOT, render


def main() -> None:
    audit = json.loads(AUDIT.read_text(encoding="utf-8"))
    manifest = json.loads((DATA / "packages.json").read_text(encoding="utf-8"))
    packages = manifest["packages"]
    assert len(packages) == 44
    assert manifest["core_packages"] == 24 and manifest["optional_packages"] == 20
    assert manifest["requirement_count"] == len(audit) == 226
    assert manifest["audit_sha256"] == hashlib.sha256(AUDIT.read_bytes()).hexdigest()
    owner = {}
    paths = []
    for package in packages:
        assert package["status"] == "planned"
        assert len(package["review_focus"]) == 5
        assert len(package["requirements"]) == package["requirement_count"]
        for req in package["requirements"]:
            assert req not in owner, f"Multiple owners: {req}"
            owner[req] = package["id"]
        for path in package["owned_paths"]:
            assert not Path(path).is_absolute() and ".." not in Path(path).parts
            paths.append((package["id"], path))
        for field in ("entry_path", "test_path"):
            assert any(package[field] == x or (x.endswith("/") and package[field].startswith(x)) for x in package["owned_paths"]), (package["id"], field)
        for path in package["read_paths"]:
            assert (ROOT / path).exists(), (package["id"], "Missing baseline", path)
        plan = ROOT / package["plan"]
        text = plan.read_text(encoding="utf-8")
        assert text.startswith("# ") and "## Global Constraints" in text and "## Review Focus" in text
        assert "- [ ]" in text and "**Spec:**" in text
        assert not re.search(r"\bTBD\b", text), f"Undecided placeholder: {package['id']}"
        assert "/Users/" not in text and "/Volumes/" not in text
    assert set(owner) == {x["id"] for x in audit}
    for i, (a, left) in enumerate(paths):
        for b, right in paths[i + 1:]:
            if a == b:
                continue
            assert not (left == right or (left.endswith("/") and right.startswith(left)) or (right.endswith("/") and left.startswith(right))), ("Shared ownership", a, b, left, right)
    by_id = {item["id"]: item for item in audit}
    visited, active = set(), set()

    def visit(req):
        assert req not in active, f"Requirement cycle at {req}"
        if req in visited:
            return
        active.add(req)
        for dep in by_id[req]["prerequisites"]:
            assert dep in by_id
            visit(dep)
        active.remove(req); visited.add(req)

    for req in by_id:
        visit(req)
    rows = list(csv.DictReader(io.StringIO((DATA / "requirement-map.csv").read_text(encoding="utf-8-sig"))))
    assert len(rows) == len({x["requirement_id"] for x in rows}) == 226
    assert all(owner[x["requirement_id"]] == x["package_id"] for x in rows)
    issue_path = DATA / "github-issues.json"
    if issue_path.exists():
        issues = json.loads(issue_path.read_text(encoding="utf-8"))["issues"]
        assert len(issues) == len({x["number"] for x in issues}) == 44
        assert {x["package_id"] for x in issues} == {x["id"] for x in packages}
        assert all(x["url"] == f"https://github.com/szotya/LaTeXIDE/issues/{x['number']}" for x in issues)
    for path, expected in render().items():
        assert path.read_bytes() == expected.encode("utf-8"), f"Stale generated file: {path.relative_to(ROOT)}"
    links = 0
    docs = [ROOT / p["plan"] for p in packages]
    docs += list(DATA.glob("*.md")) + [ROOT / "docs/superpowers/specs/2026-10-31-parallel-development-design.md"]
    for path in docs:
        text = re.sub(r"```.*?```", "", path.read_text(encoding="utf-8"), flags=re.S)
        for target in re.findall(r"\]\(([^)]+)\)", text):
            target = target.split("#")[0].strip("<>")
            if not target or re.match(r"[a-z]+:", target):
                continue
            assert (path.parent / target).exists(), f"Broken link: {path.relative_to(ROOT)} -> {target}"
            links += 1
    print(json.dumps({"result": "PASS", "packages": 44, "requirements": 226, "unique_primary_owners": 226,
                      "cross_package_owned_path_conflicts": 0, "acyclic_requirement_graph": True,
                      "generated_artifacts": len(render()), "checked_relative_links": links,
                      "product_implementation": "planned; not exercised by this validator"}, ensure_ascii=False))


if __name__ == "__main__":
    try:
        main()
    except (AssertionError, ValueError) as error:
        print(f"FAIL: {error}", file=sys.stderr)
        raise SystemExit(1)
