#!/usr/bin/env python3
"""Compile a controlled Unicode multifile example, then deny all networking."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import subprocess
import sys

from install_packages import BIN, EVIDENCE, PREFIX, ROOT, local_environment


def run(command: list[str], cwd: Path, env: dict[str, str], label: str,
        timeout: int = 120) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(command, cwd=cwd, env=env, capture_output=True,
                            text=True, timeout=timeout)
    (EVIDENCE / (label + ".log")).write_text(result.stdout + result.stderr)
    (EVIDENCE / (label + "-command.json")).write_text(
        json.dumps({"command": command, "cwd": str(cwd), "returncode": result.returncode},
                   indent=2) + "\n")
    print(label + ": exit " + str(result.returncode), flush=True)
    if result.returncode:
        print((result.stdout + result.stderr)[-6000:], flush=True)
        result.check_returncode()
    return result


def main() -> None:
    env = local_environment()
    sample = EVIDENCE / "texlive-smoke/Árvíztűrő tükörfúrógép"
    sample.mkdir(parents=True, exist_ok=True)
    (sample / "main.tex").write_text(r"""\documentclass{article}
\usepackage{fontspec}
\setmainfont{lmroman10-regular.otf}[
  BoldFont=lmroman10-bold.otf,
  ItalicFont=lmroman10-italic.otf,
  BoldItalicFont=lmroman10-bolditalic.otf]
\usepackage{amsmath}
\title{Helyi Unicode fordítás}
\author{LaTeX IDE}
\begin{document}
\maketitle
Árvíztűrő tükörfúrógép. Őrült író: áéíóöőúüű — ÁÉÍÓÖŐÚÜŰ.
\input{szöveg}
A képlet száma: \eqref{eq:eredmeny}.
\end{document}
""", encoding="utf-8")
    (sample / "szöveg.tex").write_text(r"""\section{Második fájl}
Több fájlból álló dokumentum, magyar elérési úton.
\begin{equation}\label{eq:eredmeny}
\int_0^1 x^2\,\mathrm{d}x=\frac{1}{3},\qquad
E=mc^2,\quad \alpha+\beta=\gamma.
\end{equation}
""", encoding="utf-8")
    versions = {}
    for name, command in {
        "xelatex": [str(BIN / "xelatex"), "--version"],
        "latexmk": ["/usr/bin/perl", str(BIN / "latexmk"), "-v"],
        "perl": ["/usr/bin/perl", "-e", 'printf "Perl %vd (%s)\\n", $^V, $^X'],
        "platform_arches": ["/usr/bin/lipo", "-archs", str(BIN / "xetex")],
    }.items():
        versions[name] = run(command, sample, env, "texlive-version-" + name).stdout
    paths = {}
    for variable in ("TEXMFROOT", "TEXMFDIST", "TEXMFLOCAL", "TEXMFSYSCONFIG",
                     "TEXMFSYSVAR", "TEXMFHOME", "TEXMFCONFIG", "TEXMFVAR",
                     "TEXMFCACHE", "VARTEXFONTS"):
        value = subprocess.check_output([str(BIN / "kpsewhich"), "-var-value=" + variable],
                                        cwd=sample, env=env, text=True).strip()
        paths[variable] = value
        if value and not value.startswith(str(PREFIX)):
            raise RuntimeError("Unexpected external TeX tree: " + variable + "=" + value)
    base_command = ["/usr/bin/perl", str(BIN / "latexmk"), "-norc", "-xelatex",
                    "-interaction=nonstopmode", "-halt-on-error", "-file-line-error",
                    "-latexoption=-no-shell-escape", "-synctex=1", "-g"]
    results = {}
    for label, prefix in (("online", []), ("offline", ["/usr/bin/sandbox-exec", "-p",
                                                     "(version 1)(allow default)(deny network*)"])):
        output = sample / label
        output.mkdir(exist_ok=True)
        if prefix:
            probe = ("import socket, errno\n"
                     "s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)\n"
                     "try:\n    s.connect(('127.0.0.1', 9))\n"
                     "except OSError as e:\n"
                     "    assert e.errno in (errno.EPERM, errno.EACCES), repr(e)\n"
                     "    print('OS sandbox denied outbound network: ' + str(e))\n"
                     "else:\n    raise SystemExit('Network unexpectedly allowed')\n")
            run(prefix + [sys.executable, "-c", probe], sample, env,
                "texlive-offline-network-enforcement", timeout=10)
        command = prefix + base_command + ["-outdir=" + str(output), "main.tex"]
        run(command, sample, env, "texlive-smoke-" + label)
        pdf = output / "main.pdf"
        if not pdf.exists() or pdf.read_bytes()[:5] != b"%PDF-":
            raise RuntimeError("No PDF output for " + label)
        log = (output / "main.log").read_text(errors="replace")
        if "undefined references" in log or "LaTeX Warning: Reference" in log:
            raise RuntimeError("Unresolved references for " + label)
        results[label] = {"pdf": str(pdf.relative_to(ROOT)), "bytes": pdf.stat().st_size,
                          "sha256": hashlib.sha256(pdf.read_bytes()).hexdigest(),
                          "network": "OS sandbox denies network*" if prefix else "network available; no downloads required",
                          "resolved_cross_reference": True}
    manifest = {"distribution": "TeX Live 2026", "prefix": str(PREFIX),
                "versions": versions, "texmf_paths": paths, "TMPDIR": env["TMPDIR"],
                "HOME_changed": False, "persistent_PATH_changes": False,
                "sample": str(sample.relative_to(ROOT)), "compilations": results,
                "malware_scan": "not_performed", "full_dependency_audit": "not_performed",
                "windows_runtime_test": "not_performed"}
    (EVIDENCE / "texlive-runtime-manifest.json").write_text(json.dumps(manifest, indent=2,
                                                                      ensure_ascii=False) + "\n")


if __name__ == "__main__":
    main()
