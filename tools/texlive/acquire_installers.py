#!/usr/bin/env python3
"""Acquire official installers, validating signed metadata and payload hashes.

Does not execute the installers or change the host system. Uses the captured
TeX Live 2026 checksum metadata under evidence/acquisition/compiler-review.
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import subprocess
import tarfile
import urllib.request


ROOT = Path(__file__).resolve().parents[2]
METADATA = ROOT / "evidence/acquisition/compiler-review/texlive-installer-signatures"
WORK = ROOT / ".local/texlive-acquisition"
ANCHOR = "C78B82D8C79512F79CC0D7C80D5E5D9106BAB6BC"
SOURCE = "https://tug.ctan.org/systems/texlive/tlnet/"


def acquire() -> None:
    downloads = WORK / "downloads"
    downloads.mkdir(parents=True, exist_ok=True)
    records = []
    for filename in ("install-tl-unx.tar.gz", "install-tl-windows.exe"):
        checksum_file = METADATA / (filename + ".sha512")
        signature = subprocess.run(
            ["gpgv", "--homedir", str(METADATA / "isolated-gnupg"),
             "--status-fd", "1", "--keyring", str(METADATA / "texlive-keyring.gpg"),
             str(METADATA / (filename + ".sha512.asc")), str(checksum_file)],
            capture_output=True, text=True, check=True,
        )
        valid = [line for line in signature.stdout.splitlines()
                 if line.startswith("[GNUPG:] VALIDSIG ")]
        if not valid or not any(line.endswith(ANCHOR) for line in valid):
            raise RuntimeError("Signature does not match the documented primary key")
        expected, signed_filename = checksum_file.read_text().split()
        if signed_filename != filename or len(expected) != 128:
            raise RuntimeError("Unexpected checksum metadata")
        target = downloads / filename
        if not target.exists():
            partial = target.with_suffix(target.suffix + ".partial")
            request = urllib.request.Request(SOURCE + filename,
                                             headers={"User-Agent": "LateX-IDE-acquisition/1.0"})
            with urllib.request.urlopen(request, timeout=60) as response, partial.open("wb") as output:
                total = 0
                while block := response.read(1024 * 1024):
                    total += len(block)
                    if total > 64 * 1024 * 1024:
                        raise RuntimeError("Installer exceeds the reviewed 64 MiB download limit")
                    output.write(block)
            partial.rename(target)
        observed = hashlib.file_digest(target.open("rb"), "sha512").hexdigest()
        if observed != expected:
            raise RuntimeError("Installer payload SHA-512 mismatch: " + filename)
        records.append({"filename": filename, "source_url": SOURCE + filename,
                        "bytes": target.stat().st_size, "sha512": observed,
                        "signed_sha512_matches": True, "gpg_status": signature.stdout,
                        "gpg_diagnostics": signature.stderr, "installer_executed": False})
        print(filename + ": GPG metadata and payload SHA-512 verified", flush=True)
    destination = WORK / "installer-unix"
    if not destination.exists():
        destination.mkdir()
        with tarfile.open(downloads / "install-tl-unx.tar.gz", "r:gz") as archive:
            archive.extractall(destination, filter="data")
    manifest = {"distribution": "TeX Live 2026", "scope": "Acquisition-stage record; installer execution is tracked separately in evidence/acquisition/compiler-review/texlive-install-command.json and texlive-install.log",
                "signing_key_fingerprint": ANCHOR,
                "key_anchor_limitation": "Key and documented fingerprint obtained via official TUG HTTPS",
                "installers": records, "malware_scan": "not_performed"}
    (WORK / "installer-manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print("Unix installer extracted without execution: " + str(destination), flush=True)


if __name__ == "__main__":
    acquire()
