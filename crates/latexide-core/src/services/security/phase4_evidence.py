#!/usr/bin/env python3
"""Own host-generation TeX evidence, using fixed existing local tools only."""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import sys
import unicodedata

sys.dont_write_bytecode = True
from phase2_evidence import command, nul_arguments, runtime, save, sha


def host_inputs(run: Path, role: str) -> dict:
    values = nul_arguments(run / f"{role}-host-inputs.nul")
    if len(values) < 6:
        raise ValueError("Incomplete own host input manifest")
    workspace, generation, original, main, manifest_hash, count = values[:6]
    count, generation = int(count), int(generation)
    original = Path(original)
    expected = {"main.tex", "részek/szöveg.tex", f"{role}-fonts.conf"}
    if original != run / "Árvíztűrő projekt" or main != "main.tex" or count != 3 or generation < 1 or len(values) != 6 + 5 * count:
        raise ValueError("Unexpected own host manifest; no foreign paths are opened")
    workspace_bytes = bytes.fromhex(workspace)
    if len(workspace_bytes) != 16:
        raise ValueError("Invalid host workspace binding")
    manifest = bytearray(b"latexide-p18-host-input-generation-v1\0")
    manifest.extend(workspace_bytes)
    manifest.extend(generation.to_bytes(8, "big"))

    def field(value: bytes) -> None:
        manifest.extend(len(value).to_bytes(8, "big"))
        manifest.extend(value)

    field(str(original).encode("utf-8"))
    field(main.encode("utf-8"))
    manifest.extend(count.to_bytes(8, "big"))
    files = {}
    order = []
    for index in range(count):
        relative, revision, disk_digest, content_digest, length = values[6 + 5 * index:11 + 5 * index]
        if relative not in expected or relative in files:
            raise ValueError("Undeclared or duplicate own input")
        revision, length = int(revision), int(length)
        if not 0 < revision or not 0 <= length <= 1024 * 1024:
            raise ValueError("Own input revision/quota mismatch")
        disk = original / relative
        snapshot = run / f"{role}-generation/input" / relative
        if disk.is_symlink() or snapshot.is_symlink():
            raise ValueError("Unexpected own input symlink")
        if sha(disk) != disk_digest or sha(snapshot) != content_digest or snapshot.stat().st_size != length:
            raise ValueError("Host/source/captured compiler bytes differ")
        if disk_digest != content_digest:
            raise ValueError("This real TeX fixture uses saved disk bytes; dirty buffers are separately tested")
        if snapshot.stat().st_mode & 0o277:
            raise ValueError("Snapshot not private/read-only")
        field(relative.encode("utf-8"))
        manifest.extend(revision.to_bytes(8, "big"))
        manifest.extend(length.to_bytes(8, "big"))
        manifest.extend(bytes.fromhex(content_digest))
        manifest.extend(bytes.fromhex(disk_digest))
        files[relative] = {"revision": revision, "disk_sha256": disk_digest, "compiler_sha256": content_digest, "bytes": length}
        order.append(relative)
    if set(files) != expected or order != sorted(order) or hashlib.sha256(manifest).hexdigest() != manifest_hash:
        raise ValueError("Independent host-generation digest reconstruction failed")
    raw = (original / "main.tex").read_bytes()
    if not raw.startswith(b"\xef\xbb\xbf") or b"\n" in raw.replace(b"\r\n", b""):
        raise ValueError("BOM/CRLF fixture was not preserved")
    return {"workspace": workspace, "generation": generation, "manifest_sha256": manifest_hash, "files": files}


def validate(args, phase: Path, prefix: Path) -> None:
    validator_before = sha(Path(__file__))
    shared_helper_before = sha(Path(__file__).with_name("phase2_evidence.py"))
    run = phase / "artifacts" / args.label
    destination = phase / "validation" / args.label
    destination.mkdir()
    before_path = phase / "validation" / f"{args.label}-runtime-before.json"
    before = json.loads(before_path.read_text())
    after = runtime(prefix)
    save(destination / "runtime-after.json", after)
    if before != after:
        raise ValueError("Read-only shared runtime metadata or selected bytes changed")
    bound = json.loads((phase / "bound" / args.label / "receipt.json").read_text())
    if bound.get("status") != "passed" or bound.get("legacy_phase2_suite") or bound.get("suite") != "tex" or bound.get("test_summary", {}).get("passed") != 1 or not bound.get("fresh_test_compilation"):
        raise ValueError("A fresh successful real generation TeX suite is required")
    publication = dict(line.split("=", 1) for line in (run / "generation-publication.receipt").read_text().splitlines())
    hosts = {role: host_inputs(run, role) for role in ("control", "protected")}
    protected = run / "protected-generation"
    profile_path = protected / "output/isolation.profile.sb"
    profile = profile_path.read_bytes()
    if profile != (run / "protected-probe-evidence/isolation.profile.sb").read_bytes():
        raise ValueError("Compiler/probe profiles differ")
    compiler = nul_arguments(run / "protected-tex.argv.nul")
    probe = nul_arguments(run / "protected-probe.argv.nul")
    for argv in (compiler, probe):
        if argv[:2] != ["/usr/bin/sandbox-exec", "-p"] or argv[2].encode("utf-8") != profile:
            raise ValueError("Actual spawn argv does not bind the measured profile")
    if compiler[3:5] != ["/usr/bin/perl", str(prefix / "bin/universal-darwin/latexmk")] or compiler[-1] != str(protected / "input/main.tex"):
        raise ValueError("Actual compiler must consume the captured main input")
    if "-norc" not in compiler or "-latexoption=-no-shell-escape" not in compiler or f"-outdir={protected / 'output'}" not in compiler:
        raise ValueError("Required own offline compiler arguments missing")
    if nul_arguments(run / "protected-tex.cwd.nul") != [str(protected / "input")] or nul_arguments(run / "protected-probe.cwd.nul") != [str(protected / "input")]:
        raise ValueError("Actual child cwd is not the generation")
    env_values = nul_arguments(run / "protected-tex.env.nul")
    environment = dict(zip(env_values[::2], env_values[1::2], strict=True))
    probe_values = nul_arguments(run / "protected-probe.env.nul")
    if environment != dict(zip(probe_values[::2], probe_values[1::2], strict=True)):
        raise ValueError("Probe/compiler environments differ")
    expected_keys = {"PATH", "LANG", "TZ", "TMPDIR", "TEXMFSYSVAR", "TEXMFSYSCONFIG", "FONTCONFIG_FILE", "FONTCONFIG_PATH", "TEXMFHOME", "TEXMFVAR", "TEXMFCONFIG", "TEXMFCACHE", "VARTEXFONTS", "XDG_CACHE_HOME", "XDG_CONFIG_HOME", "MKTEXFMT"}
    if set(environment) != expected_keys or environment["FONTCONFIG_FILE"] != str(protected / "input/protected-fonts.conf") or environment["TMPDIR"] != str(protected / "temporary"):
        raise ValueError("Unexpected inherited child environment")
    expected_receipt = "inside=allowed\noriginal=denied\nread=denied\nwrite=denied\nnetwork=denied\nsynthetic_env=absent\n"
    for role in ("parent", "child"):
        if (run / f"protected-probe-evidence/{role}.receipt").read_text() != expected_receipt:
            raise ValueError("Own original-project/outside/loopback/env boundary failed")
    if publication.get("publication_host_locked") != "true" or publication.get("workspace") != hosts["protected"]["workspace"] or int(publication["generation"]) != hosts["protected"]["generation"] or publication.get("manifest_sha256") != hosts["protected"]["manifest_sha256"]:
        raise ValueError("Current host publication binding mismatch")
    if publication["source_main_sha256"] != hosts["protected"]["files"]["main.tex"]["compiler_sha256"] or publication["source_part_sha256"] != hosts["protected"]["files"]["részek/szöveg.tex"]["compiler_sha256"]:
        raise ValueError("Published source byte digest mismatch")
    result = {"schema": 1, "phase": 4, "label": args.label, "scope": "own host-generation compiler fixture, not production Runtime integration",
              "bound_receipt_sha256": sha(phase / "bound" / args.label / "receipt.json"), "compiler_argv": compiler, "probe_argv": probe,
              "profile_sha256": sha(profile_path), "child_environment": environment, "host_inputs": hosts, "publication": publication,
              "runtime_selected_bytes_and_metadata_unchanged": True, "runtime_before_sha256": sha(before_path),
              "runtime_after_sha256": sha(destination / "runtime-after.json"), "documents": {}}
    tool_environment = {"PATH": "/usr/bin:/bin", "LANG": "en_US.UTF-8", "TMPDIR": str((phase / "tmp").resolve(strict=True)), "FONTCONFIG_FILE": environment["FONTCONFIG_FILE"]}
    for role in ("control", "protected"):
        storage = run / f"{role}-generation"
        pdf = storage / "output/main.pdf"
        pdf_before = sha(pdf)
        info = command([str(args.poppler / "pdfinfo"), str(pdf)], destination / f"{role}-info", tool_environment)
        if not re.search(r"^Pages:\s+1\s*$", info["stdout"], re.MULTILINE) or (destination / f"{role}-info.stderr.log").read_bytes():
            raise ValueError("Actual PDF parser did not validate exactly one clean page")
        extracted = command([str(args.poppler / "pdftotext"), "-enc", "UTF-8", "-layout", str(pdf), "-"], destination / f"{role}-text", tool_environment)
        text = unicodedata.normalize("NFC", extracted["stdout"])
        for required in ("P18 Phase3", "Árvíztűrő tükörfúrógép", "Második fájl"):
            if required not in text:
                raise ValueError("Actual parsed PDF text missing")
        if not re.search(r"A képlet száma:\s*\(1\)", text):
            raise ValueError("Actual parsed cross-reference unresolved")
        command([str(args.poppler / "pdftoppm"), "-f", "1", "-singlefile", "-scale-to", "1200", "-png", str(pdf), str(destination / f"{role}-page")], destination / f"{role}-render", tool_environment)
        image = destination / f"{role}-page.png"
        if not image.is_file() or image.stat().st_size < 1000 or sha(pdf) != pdf_before:
            raise ValueError("Render missing or PDF changed during validation")
        if role == "protected" and pdf_before != publication["output_sha256"]:
            raise ValueError("Parsed PDF differs from the current host-published bytes")
        allowed = [storage, prefix, Path("/System/Library"), Path("/usr/lib"), Path("/usr/share/locale"), Path("/usr/share/zoneinfo")]
        inputs = []
        for line in (storage / "output/main.fls").read_text().splitlines():
            if line.startswith("INPUT "):
                path = Path(line[6:])
                if not path.is_absolute():
                    path = storage / "input" / path
                path = Path(os.path.normpath(path))
                if path != Path("/dev/null") and not any(path.is_relative_to(parent) for parent in allowed):
                    raise ValueError(f"Unapproved recorder input, not opened by verifier: {path}")
                inputs.append(str(path))
        if str(storage / "input/main.tex") not in inputs or str(storage / "input/részek/szöveg.tex") not in inputs:
            raise ValueError("Actual recorder did not consume both captured TeX files")
        result["documents"][role] = {"pdf": str(pdf), "pdf_sha256": pdf_before, "bytes": pdf.stat().st_size,
                                      "pages": 1, "text_verified": True, "cross_reference_verified": True,
                                      "render": str(image), "render_sha256": sha(image), "recorder_inputs": sorted(set(inputs))}
    result["artifact_sha256"] = {str(p.relative_to(run)): sha(p) for p in sorted(run.rglob("*")) if p.is_file() and not p.is_symlink()}
    if sha(Path(__file__)) != validator_before or sha(Path(__file__).with_name("phase2_evidence.py")) != shared_helper_before:
        raise ValueError("Validator/helper source changed during use")
    result["validator_source_sha256"] = validator_before
    result["shared_evidence_helper_sha256"] = shared_helper_before
    save(destination / "pdf-validation.json", result)
    print(json.dumps({"status": "PASS", "phase": 4, "label": args.label, "pages": {role: doc["pages"] for role, doc in result["documents"].items()},
                      "runtime_unchanged": True, "host_generation": hosts["protected"]["generation"], "receipt": str(destination / "pdf-validation.json")}, ensure_ascii=False))


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("operation", choices=("before", "validate"))
    parser.add_argument("--phase", type=Path, required=True)
    parser.add_argument("--tex-prefix", type=Path, required=True)
    parser.add_argument("--label", required=True)
    parser.add_argument("--poppler", type=Path)
    args = parser.parse_args()
    phase, prefix = args.phase.resolve(strict=True), args.tex_prefix.resolve(strict=True)
    if not phase.name.startswith("inherited-fd-phase4-") or phase.parent.name != "p18-security" or not re.fullmatch(r"[a-z0-9-]+", args.label):
        raise ValueError("Explicit own Phase4 directory and fresh safe label required")
    if not (phase / "tmp").resolve(strict=True).is_relative_to(phase):
        raise ValueError("Explicit own existing TMPDIR required")
    if args.operation == "before":
        save(phase / "validation" / f"{args.label}-runtime-before.json", runtime(prefix))
        print("Captured read-only shared runtime metadata and selected hashes for own Phase4 run")
    else:
        if args.poppler is None:
            raise ValueError("Existing Poppler path required; no install or fallback")
        validate(args, phase, prefix)


if __name__ == "__main__":
    main()
