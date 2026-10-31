#!/usr/bin/env python3
"""Generate port DTOs and wire-shape vectors; no service is implemented."""
from __future__ import annotations

import argparse
import copy
import hashlib
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SRC = ROOT / "crates/latexide-contracts/src"
VECTORS = ROOT / "tests/contracts/v1"
TEXT = "A😀\né"
REVISION = 9_007_199_254_740_993
DIGEST = hashlib.sha256(TEXT.encode()).hexdigest()
COMMON = {"WorkspaceSnapshot", "SourceAnchor", "Utf16Range", "RevisionedEdit", "SourceDiagnostic", "PlatformError"}
MANUAL = {"ContractVersion", "ContractError", "IntegrationManifest", "IntegrationOrder", "OwnershipRegistry", "OwnedPath", "OwnedRequirement"}


def encoded(value):
    return json.dumps(value, ensure_ascii=False, indent=2) + "\n"


def camel(value):
    return "".join(p[:1].upper() + p[1:] for p in value.replace("-", "_").split("_"))


def declarations(interface):
    return {name: ("enum" if marker else "struct", body.strip())
            for name, marker, body in re.findall(r"(\w+)\s*(::)?\s*\{([^}]+)\}", interface)}


def fields(body):
    return [(n.strip(), t.strip()) for n, t in (p.strip().split(":", 1) for p in body.split(",") if p.strip())]


def sample(kind, key, definitions):
    kind = kind.strip()
    if kind == "String":
        if "sha" in key or key == "checksum": return DIGEST
        return {"path": "main.tex", "document_id": "main.tex", "snapshot_id": "snapshot-7", "base_snapshot": "snapshot-7",
                "source_sha": "1" * 40, "generation": "generation-1", "root": "main.tex", "cwd": ".", "project_root": ".",
                "canonical_path": "main.tex", "request_id": "fixture-request", "session_id": "fixture-session",
                "id": "fixture-id", "draft": "Árvíz 😀", "text": "A", "source": "@article{k1,title={Fixture}}",
                "replacement": "ő", "key": "k1", "name": "fixture", "command": "fixture.command", "value": "true",
                "cpu_vendor": "fixture-x64", "executable": "fixture-tool", "pdf_path": "preview.pdf",
                "synctex_path": "preview.synctex.gz"}.get(key, "fixture")
    if kind == "bool": return key not in {"network", "write", "partial_edit_allowed"}
    if kind == "char": return "M"
    if kind == "f64": return 1.0
    if kind in {"u32", "u64", "usize", "i32"}:
        if key in {"revision", "expected_revision"}: return REVISION
        return {"schema": 1, "major": 1, "minor": 0, "line": 0, "character": 0, "start": 0,
                "start_utf16": 0, "length": 1, "length_utf16": 1, "elapsed_ms": 0, "max_bytes": 1048576,
                "deadline_ms": 1000, "max_files": 20, "max_results": 100, "owner_uid": 1000, "current_uid": 1000,
                "mode": 448, "size": len(TEXT.encode()), "last_sequence": 1, "sequence": 2,
                "duration_us": 42, "median_us": 42, "p95_us": 42, "count": 1, "api_major": 1, "sdk_major": 1}.get(key, 1)
    if kind.startswith("Option<"):
        if key in {"active_turn", "cursor", "next_cursor", "old_path"}: return None
        return sample(kind[7:-1], key, definitions)
    if kind.startswith("Vec<"):
        inner = kind[4:-1]
        if inner == "u8": return list(TEXT.encode())
        if inner == "String":
            if key in {"prerequisites", "warnings", "duplicate_keys"}: return []
            return [{"files": "main.tex", "allow_paths": "main.tex", "writable_roots": ".", "env_keys": "PATH",
                     "args": "--fixture", "tool_versions": "fixture/1.0"}.get(key, "fixture")]
        if key == "diagnostics": return []
        return [sample(inner, key, definitions)]
    if kind == "WorkspaceSnapshot":
        return {"id": "snapshot-7", "epoch": 7, "files": [{"path": "main.tex", "revision": REVISION,
                "content_sha256": DIGEST, "utf8": list(TEXT.encode()), "dirty": True}]}
    if kind == "SourceAnchor":
        return {"document_id": "main.tex", "revision": REVISION, "content_sha256": DIGEST,
                "line": 0, "character": 0, "range": {"start": 0, "length": 1}}
    if kind == "Utf16Range": return {"start": 0, "length": 1}
    if kind == "RevisionedEdit":
        return {"document_id": "main.tex", "expected_revision": REVISION, "start_utf16": 0, "length_utf16": 1, "replacement": "ő"}
    if kind == "ContractVersion": return {"major": 1, "minor": 0}
    if kind == "IntegrationManifest":
        return {"schema": 1, "package_id": "P07", "contract_version": 1, "requirement_ids": ["EDT-02"],
                "source_paths": ["apps/macos/Sources/LaTeXIDEKit/Features/Editor/Wrap.swift"], "hook_targets": ["Cargo.toml"],
                "test_commands": ["swift test --filter EditorFeatureTests"], "compatibility": "contract-v1"}
    if kind == "OwnershipRegistry":
        return {"paths": [{"package_id": "P00", "path": "Cargo.toml"}, {"package_id": "P07", "path": "apps/macos/Sources/LaTeXIDEKit/Features/Editor/"}],
                "requirements": [{"package_id": "P07", "requirement_id": "EDT-02", "prerequisites": []}]}
    if kind == "PlatformError": return "unrun_workflow"
    if kind == "ContractError": return "invalid_payload"
    category, body = definitions[kind]
    if category == "enum":
        variant = body.split(",")[0].strip()
        match = re.fullmatch(r"(\w+)\(([^)]+)\)", variant)
        if match:
            return {"type": re.sub(r"(?<!^)(?=[A-Z])", "_", match[1]).lower(), "value": sample(match[2], key, definitions)}
        name = re.sub(r"(?<!^)(?=[A-Z])", "_", variant).lower()
        return {"type": name} if "(" in body else name
    return {name: sample(field_kind, name, definitions) for name, field_kind in fields(body)}


def rust_type(kind):
    if kind.startswith("&["): return "Vec<" + kind[2:-1] + ">"
    if kind == "&str": return "String"
    return kind.lstrip("&")


def expand_braces(pattern):
    match = re.search(r"\{([^}]+)\}", pattern)
    if not match: return [pattern]
    return [out for part in match[1].split(",") for out in expand_braces(pattern[:match.start()] + part + pattern[match.end():])]


def render():
    packages = json.loads((ROOT / "docs/development/package-specs.json").read_text(encoding="utf-8"))
    backlog = json.loads((ROOT / "docs/ide-audit-2026-10-31/backlog.json").read_text(encoding="utf-8"))
    owner = {req: p["id"] for p in packages for req in p["requirements"]}
    outputs = {}; modules = []; descriptors = []; vector_manifest = []
    for p in packages:
        identifier = p["id"]
        assert re.fullmatch(r"P(?:0\d|1\d|2[0-3])|U(?:0[1-9]|1\d|20)", identifier)
        assert re.fullmatch(r"[a-z][a-z0-9-]*", p["slug"])
        module = "verticals" if identifier.startswith("U") else "integration_port" if identifier == "P00" else p["slug"].replace("-", "_")
        definitions = declarations(p["interface"])
        definitions["OwnedRequirement"] = ("struct", "package_id:String, requirement_id:String, prerequisites:Vec<String>")
        signature = p["interface"].splitlines()[0]
        function, arguments, result = re.fullmatch(r"pub fn (\w+)\((.*)\)\s*->\s*(.*);", signature).groups()
        request_type = camel(function) + "Request"
        request_fields = [(name, rust_type(kind)) for name, kind in fields(arguments)]
        result_match = re.fullmatch(r"Result<\s*(.+?)\s*,\s*(\w+)\s*>", result)
        response_type, error_type = result_match.groups() if result_match else (result, "ContractError")
        trait_name = "VerticalPort" if identifier.startswith("U") else camel(p["slug"]) + "Port"
        snapshot_expression = next((f"Some(&self.{name}.id)" for name, kind in request_fields if kind == "WorkspaceSnapshot"), None)
        if identifier == "P10": snapshot_expression = "Some(&self.bundle.snapshot_id)"
        if module not in modules:
            modules.append(module)
            own = {name: value for name, value in definitions.items() if name not in COMMON | MANUAL}
            if identifier == "P00": own = {}
            all_types = " ".join(body for _, body in own.values()) + signature
            imports = sorted({name for name in COMMON | MANUAL if re.search(r"\b" + name + r"\b", all_types)} | {"ContractRequest"})
            code = "// Generated by integration/registry/generate_contracts.py; DTOs and port traits only.\n"
            code += "use serde::{Deserialize, Serialize};\nuse crate::{" + ", ".join(imports) + "};\n\n"
            for name, (category, body) in own.items():
                code += "#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]\n"
                if category == "struct":
                    code += "#[serde(deny_unknown_fields)]\n" + f"pub struct {name} {{\n" + "".join(f"    pub {field}: {kind},\n" for field, kind in fields(body)) + "}\n\n"
                else:
                    tag = 'tag = "type", content = "value", ' if "(" in body else ""
                    code += '#[serde(' + tag + 'rename_all = "snake_case")]\n' + f"pub enum {name} {{ {body} }}\n\n"
            code += "#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]\n#[serde(deny_unknown_fields)]\n"
            code += f"pub struct {request_type} {{\n" + "".join(f"    pub {name}: {kind},\n" for name, kind in request_fields) + "}\n\n"
            code += f"impl ContractRequest for {request_type} {{\n"
            code += ('    fn package_id(&self) -> &str { if self.manifest.id.starts_with(\'U\') { &self.manifest.id } else { "" } }\n' if identifier.startswith("U") else f'    fn package_id(&self) -> &str {{ "{identifier}" }}\n')
            if snapshot_expression: code += f"    fn snapshot_id(&self) -> Option<&str> {{ {snapshot_expression} }}\n"
            code += "}\n\n" + f"pub trait {trait_name} {{\n    fn {function}(&self, {arguments}) -> {result};\n}}\n"
            outputs[SRC / (module + ".rs")] = code
        payload = {name: sample(kind, name, definitions) for name, kind in request_fields}
        if identifier.startswith("U"): payload["manifest"]["id"] = identifier
        active_snapshot = "snapshot-7" if snapshot_expression else None
        envelope = {"version": {"major": 1, "minor": 0}, "package_id": identifier,
                    "request_id": identifier.lower() + "-fixture", "snapshot_id": active_snapshot, "payload": payload}
        context = {"active_snapshot": active_snapshot, "cancelled": False, "elapsed_ms": 0, "deadline_ms": None, "max_payload_bytes": 1048576}
        success = {"name": "roundtrip", "context": context, "request": envelope, "expect_boundary": "ok",
                   "response_example": sample(response_type, "response", definitions), "service_error_example": sample(error_type, "error", definitions)}
        stale = {"name": "stale", "context": dict(context, active_snapshot="snapshot-new" if active_snapshot else None), "request": envelope,
                 "expect_boundary": "stale_snapshot" if active_snapshot else "not_applicable"}
        if not active_snapshot: stale["reason"] = "pure port has no snapshot binding; owner tests service-specific state"
        cases = [success, stale,
                 {"name": "quota", "context": dict(context, max_payload_bytes=1), "request": envelope, "expect_boundary": "payload_too_large"},
                 {"name": "cancel", "context": dict(context, cancelled=True), "request": envelope, "expect_boundary": "cancelled"},
                 {"name": "deadline", "context": dict(context, elapsed_ms=10, deadline_ms=10), "request": envelope, "expect_boundary": "deadline"},
                 {"name": "malformed", "context": context, "raw": "{", "expect_boundary": "malformed_json"}]
        future = copy.deepcopy(envelope); future["version"]["major"] = 2
        cases.append({"name": "future_version", "context": context, "request": future, "expect_boundary": "incompatible_version"})
        path = VECTORS / (identifier.lower() + ".json")
        outputs[path] = encoded({"schema": 1, "package_id": identifier,
            "scope": "serialization and common boundary vectors; no service invoked; response examples are not behavior oracles", "cases": cases})
        vector_manifest.append({"package_id": identifier, "file": path.relative_to(ROOT).as_posix(), "case_count": 7,
                                "sha256": hashlib.sha256(outputs[path].encode()).hexdigest()})
        descriptors.append({"package_id": identifier, "module": module, "port_trait": trait_name, "request_type": request_type,
            "response_type": response_type, "error_type": error_type, "fixture": path.relative_to(ROOT).as_posix(),
            "service_entry": p["entry_path"], "test_path": p["test_path"], "test_command": p["test_command"],
            "contract_state": "defined-v1", "service_state": "not implemented by P00"})
    marker = "// BEGIN GENERATED PORTS"
    prefix = (SRC / "lib.rs").read_text(encoding="utf-8").split(marker)[0].rstrip() + "\n\n"
    outputs[SRC / "lib.rs"] = prefix + marker + "\n" + "".join(f"pub mod {m};\n" for m in modules) + "// END GENERATED PORTS\n"
    paths = []
    for p in packages:
        paths.extend({"package_id": p["id"], "path": path} for path in p["owned_paths"])
        if p["id"] == "P00":
            for pattern in p.get("integration_only_paths", []):
                for item in expand_braces(pattern):
                    if item.endswith("/"): paths.append({"package_id": "P00", "path": item})
                    else:
                        matches = [path for path in ROOT.glob(item) if path.is_file()]
                        if not matches:
                            raise ValueError(f"integration_only_paths entry matches no file: {item}")
                        paths.extend({"package_id": "P00", "path": path.relative_to(ROOT).as_posix()} for path in matches)
    paths = [{"package_id": package, "path": path} for package, path in sorted({(p["package_id"], p["path"]) for p in paths})]
    outputs[ROOT / "integration/registry/owners.v1.json"] = encoded({"paths": paths,
        "requirements": [{"package_id": owner[x["id"]], "requirement_id": x["id"], "prerequisites": x["prerequisites"]} for x in backlog]})
    outputs[ROOT / "integration/registry/ports.v1.json"] = encoded({"schema": 1, "version": {"major": 1, "minor": 0},
        "scope": "port DTOs and test seams; no production provider registration", "packages": descriptors})
    outputs[VECTORS / "manifest.json"] = encoded({"schema": 1, "version": {"major": 1, "minor": 0}, "vectors": vector_manifest})
    return outputs


def main():
    parser = argparse.ArgumentParser(); parser.add_argument("--check", action="store_true"); args = parser.parse_args()
    outputs = render(); changed = []
    for path, content in outputs.items():
        assert path.resolve().is_relative_to(ROOT)
        if args.check:
            if not path.exists() or path.read_bytes() != content.encode("utf-8"): changed.append(path.relative_to(ROOT).as_posix())
        else:
            path.parent.mkdir(parents=True, exist_ok=True); path.write_text(content, encoding="utf-8", newline="")
    if changed: raise SystemExit("Stale contract artifacts: " + ", ".join(changed))
    print(f"{'Checked' if args.check else 'Generated'} {len(outputs)} contract artifacts; 44 packages / 308 vectors; no service implementation.")


if __name__ == "__main__": main()
