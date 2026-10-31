#!/usr/bin/env python3
"""Run the verified upstream installer with an isolated portable project prefix.

Defaults to a repository-verifying dry run; pass --install for the authorized
project-local installation. HOME and shell startup files are never changed.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]
WORK = ROOT / ".local/texlive-acquisition"
PREFIX = ROOT / ".local/texlive-macos"
EVIDENCE = ROOT / "evidence/acquisition/compiler-review"
META = EVIDENCE / "texlive-installer-signatures"
ANCHOR = "C78B82D8C79512F79CC0D7C80D5E5D9106BAB6BC"
REPO = "https://tug.ctan.org/systems/texlive/tlnet"


def render_profile(template: str, prefix: Path) -> str:
    value = str(prefix)
    if not prefix.is_absolute() or "\n" in value or "\r" in value:
        raise ValueError("TeX Live prefix must be an absolute single-line path")
    if "@TEXLIVE_PREFIX@" not in template:
        raise ValueError("TeX Live profile template has no prefix placeholder")
    return template.replace("@TEXLIVE_PREFIX@", value)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--install", action="store_true")
    args = parser.parse_args()
    payload = WORK / "downloads/install-tl-unx.tar.gz"
    digest, filename = (META / "install-tl-unx.tar.gz.sha512").read_text().split()
    if filename != payload.name:
        raise RuntimeError("Unexpected signed installer filename")
    with payload.open("rb") as source:
        if hashlib.file_digest(source, "sha512").hexdigest() != digest:
            raise RuntimeError("Installer SHA-512 mismatch")
    signature = subprocess.run(
        ["gpgv", "--homedir", str(META / "isolated-gnupg"), "--status-fd", "1",
         "--keyring", str(META / "texlive-keyring.gpg"),
         str(META / "install-tl-unx.tar.gz.sha512.asc"),
         str(META / "install-tl-unx.tar.gz.sha512")],
        capture_output=True, text=True, check=True,
    )
    if not any(line.startswith("[GNUPG:] VALIDSIG ") and line.endswith(ANCHOR)
               for line in signature.stdout.splitlines()):
        raise RuntimeError("Unexpected signing key")
    installers = list((WORK / "installer-unix").glob("install-tl-*/install-tl"))
    if len(installers) != 1:
        raise RuntimeError("Expected exactly one extracted official installer")
    temporary = WORK / "tmp"
    temporary.mkdir(parents=True, exist_ok=True)
    profile = WORK / "profiles" / f"macos-{os.getpid()}.profile"
    profile.parent.mkdir(parents=True, exist_ok=True)
    profile.write_text(render_profile((ROOT / "tools/texlive/macos.profile").read_text(), PREFIX))
    gpg = shutil.which("gpg")
    if gpg is None:
        raise RuntimeError("A working GPG verifier is required")
    env = os.environ.copy()
    env.update({"TMPDIR": str(temporary), "TEXLIVE_GNUPG": gpg,
                "TEXLIVE_INSTALL_NO_RESUME": "1", "TEXLIVE_INSTALL_NO_WELCOME": "1",
                "TEXMFHOME": str(PREFIX / "texmf-local"),
                "TEXMFCONFIG": str(PREFIX / "texmf-config"),
                "TEXMFVAR": str(PREFIX / "texmf-var"),
                "TEXMFCACHE": str(PREFIX / "texmf-var")})
    command = ["/usr/bin/perl", str(installers[0]), "--profile",
               str(profile), "--no-interaction", "--no-gui",
               "--portable", "--repository", REPO, "--verify-downloads",
               "--warn-checksums", "--strict"]
    if not args.install:
        command.append("--no-installation")
    label = "install" if args.install else "plan"
    record = {"command": command, "HOME_unchanged": env.get("HOME") == os.environ.get("HOME"),
              "persistent_PATH_changes": False, "prefix": str(PREFIX),
              "resolved_profile": profile.read_text(),
              "installer_payload_sha512": digest, "signature_status": signature.stdout,
              "note": "Upstream installer prepends its local binary directory to its own child-process PATH only."}
    (EVIDENCE / ("texlive-" + label + "-command.json")).write_text(json.dumps(record, indent=2) + "\n")
    with (EVIDENCE / ("texlive-" + label + ".log")).open("w") as log:
        process = subprocess.Popen(command, cwd=WORK, env=env, stdout=subprocess.PIPE,
                                   stderr=subprocess.STDOUT, text=True)
        assert process.stdout is not None
        for line in process.stdout:
            log.write(line)
            log.flush()
            print(line, end="", flush=True)
        return process.wait()


if __name__ == "__main__":
    raise SystemExit(main())
