#!/usr/bin/env python3
"""Required P18 execution coverage and fresh compiler PDF validation."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import unicodedata

ROOT = Path(__file__).resolve().parents[2]
REQUIRED = ["os_subprocess_boundary_probe", "review_focus_r2",
            "security_service::execution::tests::compiler_adapter_real_tex_offline",
            "security_service::generation::tex_tests::generation_real_tex_offline"]


def validate_workspace_log(text):
    outcomes = []
    selected_target = False
    for line in text.splitlines():
        if re.match(r"^\s*(Running |Doc-tests )", line):
            selected_target = bool(re.match(r"^\s*Running tests/security_policy_contract\.rs \(", line))
        match = re.match(r"^test (\S+) \.\.\. (ok|FAILED|ignored)(?:,.*)?$", line)
        if match and selected_target: outcomes.append(match.groups())
    if not outcomes or any(state != "ok" for _, state in outcomes):
        raise ValueError("P18 workspace contains skipped/failed or no named outcomes")
    ids = [name for name, _ in outcomes]
    if "p18_os_probe_child" in ids or any(ids.count(name) != 1 for name in REQUIRED):
        raise ValueError("Required P18 case missing, duplicated, or private helper counted as test")
    return REQUIRED.copy()


def validate_pdf_record(data, phase):
    if type(data.get("pages")) is not int or data["pages"] != 1 or not isinstance(data.get("text"), str):
        raise ValueError("Expected exactly one parsed PDF page")
    text = unicodedata.normalize("NFC", data["text"])
    if any(s not in text for s in (f"P18 Phase{phase}", "Árvíztűrő tükörfúrógép", "Második fájl")) or not re.search(r"A képlet száma:\s*\(1\)", text):
        raise ValueError("PDF text or resolved reference differs")


def validate_render_pair(a, b):
    first, second = Path(a).read_bytes(), Path(b).read_bytes()
    if not first or first != second: raise ValueError("Control/protected PDF render differs or is empty")


def sha(path):
    with Path(path).open("rb") as stream: return hashlib.file_digest(stream, "sha256").hexdigest()


def toolchain_state(prefix):
    prefix = Path(prefix).resolve(strict=True)
    files, links = {}, {}
    for path in sorted(prefix.rglob("*")):
        name = path.relative_to(prefix).as_posix()
        if path.is_symlink():
            if not path.resolve(strict=False).is_relative_to(prefix): raise ValueError("TeX symlink escaped its private prefix")
            links[name] = os.readlink(path)
        elif path.is_file(): files[name] = {"sha256": sha(path), "mode": path.stat().st_mode & 0o7777}
    if not files: raise ValueError("Empty TeX fixture")
    return {"files": files, "links": links}


def validate_toolchain(prefix):
    prefix = Path(prefix).resolve(strict=True)
    receipt = prefix.parent / "provision.json"
    data = json.loads(receipt.read_text(encoding="utf-8"))
    if data.get("schema") != 1 or data.get("prefix") != str(prefix) or data.get("toolchain") != toolchain_state(prefix):
        raise ValueError("Provisioned TeX fixture changed or belongs to another prefix")
    return sha(receipt)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--rust-log", type=Path, required=True)
    parser.add_argument("--result", type=Path, required=True)
    args = parser.parse_args()
    toolchain_receipt = validate_toolchain(os.environ["LATEXIDE_P18_TEX_PREFIX"])
    cases = validate_workspace_log(args.rust_log.read_text(encoding="utf-8", errors="replace"))
    label = os.environ["LATEXIDE_P18_TEX_RUN"]
    if not re.fullmatch(r"[a-zA-Z0-9-]+", label): raise ValueError("Invalid P18 run label")
    directory = args.result.parent / "p18-pdfs"
    directory.mkdir(mode=0o700, exist_ok=False)
    binary = directory / "pdf-check"
    subprocess.run(["xcrun", "swiftc", "-module-cache-path", str(directory / "module-cache"),
                    str(ROOT / "tests/contracts/P18PDFCheck.swift"), "-o", str(binary)], check=True)
    artifacts = [{"path": str(binary), "kind": "p18_pdf_checker"}]
    records = []
    for phase in (2, 3):
        root = Path(os.environ[f"LATEXIDE_P18_PHASE{phase}_ROOT"]).resolve(strict=True)
        run = root / "artifacts" / label
        renders = []
        for role in ("control", "protected"):
            pdf = run / (role if phase == 2 else role + "-generation") / "output/main.pdf"
            if pdf.is_symlink() or not pdf.is_file(): raise ValueError("Fresh P18 compiler PDF missing")
            before = sha(pdf)
            png = directory / f"phase{phase}-{role}.png"
            result = subprocess.run([str(binary), str(pdf), str(png)], check=True, capture_output=True, text=True)
            data = json.loads(result.stdout)
            validate_pdf_record(data, phase)
            if sha(pdf) != before: raise ValueError("PDF changed during parser/render validation")
            records.append({"phase": phase, "role": role, "pdf_sha256": before, "render_sha256": sha(png), "pages": data["pages"]})
            # PDFs live in the caller's private fixture root. Bind copies within
            # the declared verification output root for ordinary report recheck.
            copy = directory / f"phase{phase}-{role}.pdf"
            with copy.open("xb") as output: output.write(pdf.read_bytes())
            if sha(copy) != before: raise ValueError("PDF changed during evidence capture")
            artifacts.extend([{"path": str(copy), "kind": "p18_compiler_pdf"}, {"path": str(png), "kind": "p18_pdf_render"}])
            renders.append(png)
        validate_render_pair(*renders)
    receipt = directory / "validation.json"
    receipt.write_text(json.dumps({"cases": cases, "toolchain_receipt_sha256": toolchain_receipt, "rust_log_sha256": sha(args.rust_log), "pdfs": records}, indent=2) + "\n", encoding="utf-8")
    artifacts.append({"path": str(receipt), "kind": "p18_pdf_validation"})
    summary = {"schema": 1, "verification_run_id": os.environ["P00_VERIFICATION_RUN_ID"], "suite": "p18-pdfs",
               "tests": 4, "passed": 4, "skipped": 0, "failures": 0, "errors": 0, "skipped_ids": [], "artifacts": artifacts}
    args.result.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"status": "passed", "required_cases": cases, "pdfs": records}))


if __name__ == "__main__": main()
