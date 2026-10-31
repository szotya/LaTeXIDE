"""P00 source observations and closure boundaries, never provider acceptance."""
import hashlib
import json
from pathlib import Path, PurePosixPath


def load(root, name):
    with (Path(root) / name).open("rb") as stream:
        raw = stream.read(1024 * 1024 + 1)
    if len(raw) > 1024 * 1024:
        raise ValueError("Readiness input quota exceeded")
    return json.loads(raw)


def unique(rows, key):
    result = {row[key]: row for row in rows}
    if len(result) != len(rows):
        raise ValueError("Duplicate readiness identity")
    return result


def catalog(root):
    audit = unique(load(root, "docs/ide-audit-2026-10-31/backlog.json"), "id")
    packages = unique(load(root, "docs/development/package-specs.json"), "id")
    ports = unique(load(root, "integration/registry/ports.v1.json")["packages"], "package_id")
    owners = unique(load(root, "integration/registry/owners.v1.json")["requirements"], "requirement_id")
    assigned = {}
    for package in packages.values():
        for requirement in package["requirements"]:
            if requirement in assigned:
                raise ValueError("Multiple primary owners")
            assigned[requirement] = package["id"]
    if set(audit) != set(owners) or set(audit) != set(assigned) or set(packages) != set(ports):
        raise ValueError("Capability allocation differs")
    for key, owner in owners.items():
        if owner["package_id"] != assigned[key] or owner["prerequisites"] != audit[key]["prerequisites"]:
            raise ValueError("Owner or prerequisite differs from the canonical requirement")
    for key, port in ports.items():
        if port["service_entry"] != packages[key]["entry_path"] or port["test_path"] != packages[key]["test_path"]:
            raise ValueError("Port source differs from package allocation")
    active = set(); done = set()

    def visit(key):
        if key not in owners or key in active:
            raise ValueError("Unknown or cyclic requirement")
        if key in done:
            return
        active.add(key)
        for parent in owners[key]["prerequisites"]:
            visit(parent)
        active.remove(key); done.add(key)

    for key in owners:
        visit(key)
    return audit, packages, ports, owners


def observe(root, name):
    if not isinstance(name, str) or not name or "\\" in name or ":" in name or "\x00" in name:
        raise ValueError("Non-relative source observation")
    parts = PurePosixPath(name).parts
    if PurePosixPath(name).is_absolute() or any(p in {".", ".."} for p in parts):
        raise ValueError("Source observation escapes repository")
    root = Path(root).resolve(); path = root
    for part in parts:
        path = path / part
        if path.is_symlink():
            return {"path": name, "state": "unverified_symlink", "sha256": None}
    if not path.resolve().is_relative_to(root):
        raise ValueError("Source observation escapes repository")
    if path.is_dir():
        return {"path": name, "state": "reference_directory", "sha256": None}
    if not path.is_file():
        return {"path": name, "state": "absent", "sha256": None}
    size = path.stat().st_size
    checksum = None
    if size <= 4 * 1024 * 1024:
        with path.open("rb") as stream:
            checksum = hashlib.file_digest(stream, "sha256").hexdigest()
    return {"path": name, "state": "source_only", "sha256": checksum, "bytes": size,
            "hash_state": "measured" if checksum else "over_limit", "execution": "unrun"}


def capability_inventory(root):
    audit, packages, ports, owners = catalog(root)
    package_rows = {}
    for key, port in sorted(ports.items()):
        package_rows[key] = {"contract": port["contract_state"], "acceptance": "unqualified",
                             "service_source": observe(root, port["service_entry"]),
                             "test_source": observe(root, port["test_path"]),
                             "manifest_source": observe(root, f"integration/requests/{key}.json")}
    requirements = {key: {"owner": owners[key]["package_id"], "baseline": item["observed_status"],
                           "acceptance": "unqualified", "legacy_reference_sources": [observe(root, p) for p in item["paths"]]}
                    for key, item in sorted(audit.items())}
    return {"schema": 1, "scope": "current source-only observations; baseline is historical, no provider or test acceptance",
            "packages": package_rows, "requirements": requirements}


def p00_readiness(root, report, validation):
    audit, packages, ports, owners = catalog(root)
    rules = load(root, "integration/registry/p00-acceptance.v1.json")
    if rules.get("schema") != 1 or rules.get("package_id") != "P00" or set(rules["requirements"]) != {"QA-08", "QA-14", "OS-10"}:
        raise ValueError("Unknown P00 readiness sidecar")
    for key, value in rules["requirements"].items():
        if owners[key]["package_id"] != "P00" or value["prerequisites"] != owners[key]["prerequisites"]:
            raise ValueError("Readiness cannot remove canonical dependencies")
        criteria = value["criteria"]
        expected = {"QA-08": {"contract-and-coordinator", "common-runtime", "manifest-queue", "mac-adapter-regression"},
                    "QA-14": {"stored-evidence", "inventory-regression", "guide-identity"},
                    "OS-10": {"contributor-reference", "standalone-entry"}}
        if set(unique(criteria, "id")) != expected[key]:
            raise ValueError("Required independent criteria omitted")
        for criterion in criteria:
            checks = criterion["checks"]
            if not isinstance(checks, list) or any(not isinstance(c, str) or not c for c in checks) or len(set(checks)) != len(checks) or (not checks and criterion["id"] != "stored-evidence"):
                raise ValueError("Invalid independent check list")
    rows = unique(report["checks"], "name")
    current = report.get("status") == "passed" and validation.get("status") == "current"
    requirements = {}; external = {}
    for key, rule in rules["requirements"].items():
        criteria = []
        for criterion in rule["criteria"]:
            status = "unrun"
            if not current:
                status = validation.get("status", "failed") if validation.get("status") != "current" else "failed"
            elif report.get("profile") in criterion.get("profiles", [report.get("profile")]):
                checks = [rows.get(n) for n in criterion["checks"]]
                status = "passed" if all(c and c["status"] == "passed" for c in checks) else "unrun" if any(not c or c["status"] == "unrun" for c in checks) else "failed"
            criteria.append({**criterion, "status": status})
        dependencies = []

        def collect(parent, direct):
            if any(d["requirement_id"] == parent for d in dependencies):
                return
            package = owners[parent]["package_id"]
            item = {"requirement_id": parent, "package_id": package, "direct": direct, "acceptance": "unqualified"}
            dependencies.append(item)
            if package != "P00":
                external[parent] = {**item, "manifest_present": observe(root, f"integration/requests/{package}.json")["state"] == "source_only",
                                    "required_evidence": audit[parent]["acceptance"], "provider_paths": audit[parent]["paths"]}
            for next_parent in owners[parent]["prerequisites"]:
                collect(next_parent, False)

        for parent in rule["prerequisites"]:
            collect(parent, True)
        requirements[key] = {"owner": "P00", "acceptance": "partial", "direct_prerequisites": rule["prerequisites"],
                             "unqualified_dependencies": dependencies, "criteria": criteria}
    scopes = []
    for scope in rules["validation_scopes"]:
        if not scope["owners"] or any(owner not in packages for owner in scope["owners"]) or scope["classification"] not in {"native_validation", "future_platform", "separate_program"} or scope["blocks_current_mac_common_slice"] is not False:
            raise ValueError("Invalid scoped validation owner or classification")
        scopes.append({**scope, "status": "unrun", "dag_prerequisite": False})
    statuses = [c["status"] for r in requirements.values() for c in r["criteria"]]
    independent = "passed" if all(s == "passed" for s in statuses) else "unrun" if all(s == "unrun" for s in statuses) else "partial"
    return {"schema": 1, "package_id": "P00", "acceptance": "partial", "independent_status": independent,
            "evidence_status": validation.get("status", "unrun"), "requirements": requirements,
            "external_package_blockers": [external[k] for k in sorted(external)], "validation_scopes": scopes,
            "scope": "independent coordinator criteria plus unqualified real prerequisites; no automatic acceptance"}
