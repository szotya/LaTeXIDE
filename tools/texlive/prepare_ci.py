#!/usr/bin/env python3
"""Install the signed, dated TeX Live fixture into a fresh private CI prefix."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import tarfile
import importlib.util

ROOT = Path(__file__).resolve().parents[2]
META = ROOT / "evidence/acquisition/compiler-review/texlive-installer-signatures"
REPOSITORY = "https://texlive.info/tlnet-archive/2026/10/30/tlnet"
ANCHOR = "C78B82D8C79512F79CC0D7C80D5E5D9106BAB6BC"


def verify_installer(payload, checksum):
    fields = Path(checksum).read_text(encoding="utf-8").split()
    if len(fields) != 2 or fields[1] != "install-tl-unx.tar.gz":
        raise ValueError("Unexpected signed installer filename")
    with Path(payload).open("rb") as stream:
        if hashlib.file_digest(stream, "sha512").hexdigest() != fields[0]:
            raise ValueError("Installer SHA-512 mismatch")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--destination", type=Path, required=True)
    args = parser.parse_args()
    if platform.system() != "Darwin" or platform.machine().lower() != "arm64":
        raise ValueError("P18 CI provisioning requires macOS arm64")
    destination = args.destination.resolve()
    if not args.destination.is_absolute() or any(c.isspace() for c in str(destination)):
        raise ValueError("An explicit space-free private CI prefix is required")
    destination.mkdir(mode=0o700, parents=False, exist_ok=False)
    gpg = shutil.which("gpg")
    gpgv = shutil.which("gpgv")
    if not gpg or not gpgv: raise ValueError("GPG verification tools are required")
    key_home = destination / "gnupg"; key_home.mkdir(mode=0o700)
    checksum = META / "install-tl-unx.tar.gz.sha512"
    signature = subprocess.run([gpgv, "--homedir", str(key_home), "--status-fd", "1", "--keyring", str(META / "texlive-keyring.gpg"),
                                str(checksum) + ".asc", str(checksum)], check=True, capture_output=True, text=True)
    if not any(line.startswith("[GNUPG:] VALIDSIG ") and (line.endswith(ANCHOR) or line.split()[2] == ANCHOR)
               for line in signature.stdout.splitlines()):
        raise ValueError("Unexpected installer signing key")
    payload = destination / "install-tl-unx.tar.gz"
    subprocess.run(["/usr/bin/curl", "--fail", "--location", "--proto", "=https", "--max-time", "120",
                    "--output", str(payload), REPOSITORY + "/install-tl-unx.tar.gz"], check=True)
    verify_installer(payload, checksum)
    extracted = destination / "installer"; extracted.mkdir(mode=0o700)
    with tarfile.open(payload) as archive: archive.extractall(extracted, filter="data")
    installers = list(extracted.glob("install-tl-*/install-tl"))
    if len(installers) != 1: raise ValueError("Expected one verified installer")
    prefix = destination / "texlive"
    profile = destination / "macos.profile"
    profile.write_text((ROOT / "tools/texlive/macos.profile").read_text().replace("@TEXLIVE_PREFIX@", str(prefix)), encoding="utf-8")
    temporary = destination / "tmp"; temporary.mkdir(mode=0o700)
    env = {k: v for k, v in os.environ.items() if not k.startswith(("TEX", "MKTEX", "PERL5"))}
    env.update(TMPDIR=str(temporary), TEXLIVE_GNUPG=gpg, TEXLIVE_DOWNLOADER="curl", TEXLIVE_INSTALL_NO_RESUME="1", TEXLIVE_INSTALL_NO_WELCOME="1",
               TEXMFHOME=str(prefix / "texmf-local"), TEXMFCONFIG=str(prefix / "texmf-config"),
               TEXMFVAR=str(prefix / "texmf-var"), TEXMFCACHE=str(prefix / "texmf-var"))
    subprocess.run(["/usr/bin/perl", str(installers[0]), "--profile", str(profile), "--no-interaction", "--no-gui", "--portable",
                    "--repository", REPOSITORY, "--verify-downloads", "--warn-checksums", "--strict"], env=env, check=True)
    packages = json.loads((ROOT / "tools/texlive/package-roots.json").read_text())["packages"]
    subprocess.run(["/usr/bin/perl", str(prefix / "bin/universal-darwin/tlmgr"), "--verify-repo=all", "--repository", REPOSITORY,
                    "install", *packages], env=env, check=True)
    spec = importlib.util.spec_from_file_location("p18_ci_tools", ROOT / "integration/registry/p18_ci.py")
    gate = importlib.util.module_from_spec(spec); spec.loader.exec_module(gate)
    receipt = {"schema": 1, "toolchain": gate.toolchain_state(prefix), "repository": REPOSITORY, "installer_sha512": checksum.read_text().split()[0],
               "signature_status": signature.stdout, "packages": packages, "prefix": str(prefix)}
    (destination / "provision.json").write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__": main()
