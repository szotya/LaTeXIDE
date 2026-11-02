#!/usr/bin/env python3
"""Local test evidence only. Never executes a command from a manifest/report."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import unicodedata

SELECTED = (
    "bin/universal-darwin/xetex", "bin/universal-darwin/xdvipdfmx",
    "bin/universal-darwin/kpsewhich", "texmf-dist/scripts/latexmk/latexmk.pl",
    "texmf-var/web2c/xetex/xelatex.fmt", "texmf-dist/tex/latex/fontspec/fontspec.sty",
    "texmf-dist/tex/latex/amsmath/amsmath.sty",
    "texmf-dist/fonts/opentype/public/lm/lmroman10-regular.otf",
    "texmf-dist/fonts/opentype/public/lm/lmroman10-bold.otf",
    "texmf-dist/fonts/opentype/public/lm/lmroman10-italic.otf",
    "texmf-dist/fonts/opentype/public/lm/lmroman10-bolditalic.otf",
)
SYSTEM = ("/usr/bin/perl", "/bin/sh", "/bin/bash", "/usr/bin/sandbox-exec",
          "/System/Library/Sandbox/Profiles/dyld-support.sb")


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def save(path: Path, value: object) -> None:
    with path.open("x", encoding="utf-8") as output:
        json.dump(value, output, ensure_ascii=False, indent=2)
        output.write("\n")


def runtime(prefix: Path) -> dict:
    # Census is metadata only; no recursive arbitrary config/user-content reads.
    census = {}
    for path in sorted(prefix.rglob("*")):
        info = path.lstat()
        census[str(path.relative_to(prefix))] = {
            "mode": info.st_mode, "size": info.st_size, "mtime_ns": info.st_mtime_ns,
            "device": info.st_dev, "inode": info.st_ino,
            "link": os.readlink(path) if path.is_symlink() else None,
        }
    selected = {}
    for name in SELECTED:
        path = prefix / name
        if not path.resolve(strict=True).is_relative_to(prefix):
            raise ValueError("Selected compiler resource escaped the declared prefix")
        selected[name] = sha(path)
    return {"prefix": str(prefix), "metadata_census": census, "selected_sha256": selected,
            "system_sha256": {name: sha(Path(name)) for name in SYSTEM},
            "shell_selector": os.readlink("/private/var/select/sh")}


def command(argv: list[str], destination: Path, environment: dict[str, str]) -> dict:
    program_hash = sha(Path(argv[0]))
    result = subprocess.run(argv, env=environment, capture_output=True, timeout=30)
    stdout = destination.with_suffix(".stdout.log")
    stderr = destination.with_suffix(".stderr.log")
    with stdout.open("xb") as output:
        output.write(result.stdout)
    with stderr.open("xb") as output:
        output.write(result.stderr)
    program_unchanged = sha(Path(argv[0])) == program_hash
    record = {"argv": argv, "exit": result.returncode, "program_sha256": program_hash,
              "program_unchanged": program_unchanged,
              "stdout_sha256": sha(stdout), "stderr_sha256": sha(stderr)}
    save(destination.with_suffix(".command.json"), record)
    if result.returncode or not program_unchanged:
        raise RuntimeError(f"Validation command failed ({destination.name}); logs preserved")
    return {**record, "stdout": result.stdout.decode("utf-8")}


def nul_arguments(path: Path) -> list[str]:
    body = path.read_bytes()
    if not body.endswith(b"\0"):
        raise ValueError("Unterminated argv evidence")
    return [arg.decode("utf-8") for arg in body[:-1].split(b"\0")]


def validate(args, phase: Path, prefix: Path) -> None:
    validator_hash = sha(Path(__file__))
    run = phase / "artifacts" / args.label
    target = phase / "validation" / args.label
    target.mkdir()
    environment = {"PATH": "/usr/bin:/bin", "LANG": "en_US.UTF-8", "TMPDIR": str(phase / "tmp"),
                   "FONTCONFIG_FILE": str(run / "Árvíztűrő projekt/protected-fonts.conf")}
    before = json.loads((phase / "validation" / f"{args.label}-runtime-before.json").read_text())
    after = runtime(prefix)
    save(target / "runtime-after.json", after)
    if before != after:
        raise ValueError("Shared compiler metadata or selected bytes changed; not PASS")
    profile = run / "protected/output/isolation.profile.sb"
    probe_profile = run / "protected-probe-evidence/isolation.profile.sb"
    if profile.read_bytes() != probe_profile.read_bytes():
        raise ValueError("Compiler and malicious-child probe profiles differ")
    compiler_argv = nul_arguments(run / "protected-tex.argv.nul")
    probe_argv = nul_arguments(run / "protected-probe.argv.nul")
    for argv in (compiler_argv, probe_argv):
        if argv[:2] != ["/usr/bin/sandbox-exec", "-p"] or argv[2].encode() != profile.read_bytes():
            raise ValueError("argv/profile binding is not the tested sandbox")
    env = nul_arguments(run / "protected.env.nul")
    env = dict(zip(env[::2], env[1::2], strict=True))
    if any(name in env for name in ("HOME", "PERL5LIB", "PERL5OPT", "P18_SYNTHETIC_SECRET")):
        raise ValueError("Unexpected inherited environment")
    expected = "inside=allowed\nread=denied\nwrite=denied\nnetwork=denied\nsynthetic_env=absent\n"
    for role in ("parent", "child"):
        if (run / f"protected-probe-evidence/{role}.receipt").read_text() != expected:
            raise ValueError("Boundary probe failed")
    result = {"schema": 1, "label": args.label, "scope": "own standalone adapter, not application integration",
              "compiler_argv": compiler_argv, "probe_argv": probe_argv, "child_environment": env,
              "profile_sha256": sha(profile), "runtime_selected_bytes_and_metadata_unchanged": True,
              "runtime_before_sha256": sha(phase / "validation" / f"{args.label}-runtime-before.json"),
              "runtime_after_sha256": sha(target / "runtime-after.json"), "documents": {}}
    for role in ("control", "protected"):
        pdf = run / role / "output/main.pdf"
        info = command([str(args.poppler / "pdfinfo"), str(pdf)], target / f"{role}-info", environment)
        if not re.search(r"^Pages:\s+1\s*$", info["stdout"], re.MULTILINE):
            raise ValueError("The actual PDF parser did not find the expected page")
        if (target / f"{role}-info.stderr.log").read_bytes():
            raise ValueError("PDF parser emitted diagnostics")
        text = command([str(args.poppler / "pdftotext"), "-enc", "UTF-8", "-layout", str(pdf), "-"], target / f"{role}-text", environment)
        normalized = unicodedata.normalize("NFC", text["stdout"])
        for expected_text in ("P18 Phase2", "Árvíztűrő tükörfúrógép", "Második fájl"):
            if expected_text not in normalized:
                raise ValueError(f"Expected own fixture text missing: {expected_text}")
        if not re.search(r"A képlet száma:\s*\(1\)", normalized):
            raise ValueError("Cross-reference text not resolved in parsed PDF")
        image = target / f"{role}-page"
        command([str(args.poppler / "pdftoppm"), "-f", "1", "-singlefile", "-scale-to", "1200", "-png", str(pdf), str(image)], target / f"{role}-render", environment)
        png = image.with_suffix(".png")
        if not png.is_file() or png.stat().st_size < 1000:
            raise ValueError("PDF render missing or empty")
        allowed = [prefix, run, Path("/System/Library"), Path("/usr/lib"), Path("/usr/share/locale"), Path("/usr/share/zoneinfo")]
        inputs = []
        for line in (run / role / "output/main.fls").read_text().splitlines():
            if line.startswith("INPUT "):
                path = Path(line[6:])
                if not path.is_absolute():
                    path = run / "Árvíztűrő projekt" / path
                # Lexical normalization only; do not open unexpected user paths.
                path = Path(os.path.normpath(path))
                if path != Path("/dev/null") and not any(path.is_relative_to(root) for root in allowed):
                    raise ValueError(f"Unapproved recorder input (not opened by verifier): {path}")
                inputs.append(str(path))
        result["documents"][role] = {"pdf": str(pdf), "pdf_sha256": sha(pdf), "bytes": pdf.stat().st_size,
                                      "pages": 1, "text_verified": True, "cross_reference_verified": True,
                                      "render": str(png), "render_sha256": sha(png),
                                      "recorder_inputs": sorted(set(inputs))}
    result["artifact_sha256"] = {str(p.relative_to(run)): sha(p) for p in sorted(run.rglob("*")) if p.is_file()}
    if sha(Path(__file__)) != validator_hash:
        raise ValueError("Validator source changed during use")
    result["validator_source_sha256"] = validator_hash
    save(target / "pdf-validation.json", result)
    print(json.dumps({"result": "PASS", "label": args.label, "pages": {k:v["pages"] for k,v in result["documents"].items()},
                      "profile_sha256": result["profile_sha256"], "runtime_unchanged": True,
                      "receipt": str(target / "pdf-validation.json")}, ensure_ascii=False))


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("operation", choices=("before", "validate"))
    parser.add_argument("--phase", required=True, type=Path)
    parser.add_argument("--tex-prefix", required=True, type=Path)
    parser.add_argument("--label", required=True)
    parser.add_argument("--poppler", type=Path)
    args = parser.parse_args()
    if not re.fullmatch(r"[a-zA-Z0-9-]+", args.label):
        raise ValueError("Invalid own run label")
    phase, prefix = args.phase.resolve(strict=True), args.tex_prefix.resolve(strict=True)
    if args.operation == "before":
        save(phase / "validation" / f"{args.label}-runtime-before.json", runtime(prefix))
        print("Captured shared runtime metadata and selected compiler/resource bytes; no toolchain write")
    else:
        if args.poppler is None:
            raise ValueError("Existing Poppler tools required; installation is not supported")
        validate(args, phase, prefix)


if __name__ == "__main__":
    main()
