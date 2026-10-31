#!/usr/bin/env python3
"""Install the narrow shared package roots into the project-local TeX Live."""

from __future__ import annotations

import json
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
PREFIX = ROOT / ".local/texlive-macos"
BIN = PREFIX / "bin/universal-darwin"
EVIDENCE = ROOT / "evidence/acquisition/compiler-review"
REPO = "https://tug.ctan.org/systems/texlive/tlnet"


def local_environment() -> dict[str, str]:
    # Keep HOME unchanged, but do not inherit Perl startup/module injection or
    # external TeX trees. Native engine and scripts only need these host values.
    env = {key: os.environ[key] for key in ("HOME", "USER", "LOGNAME", "LANG", "LC_CTYPE")
           if key in os.environ}
    # Upstream tlmgr constructs its fmtutil --status-file shell argument
    # without quoting. A space-free temporary directory avoids that bug
    # while leaving the actual portable installation at the project path.
    temporary = Path(tempfile.mkdtemp(prefix="latexide-texlive-", dir="/private/tmp"))
    env.update({"PATH": str(BIN) + os.pathsep + "/usr/bin:/bin:/usr/sbin:/sbin",
                "TMPDIR": str(temporary),
                "TEXLIVE_GNUPG": "/usr/local/bin/gpg",
                "TEXMFHOME": str(PREFIX / "texmf-local"),
                "TEXMFCONFIG": str(PREFIX / "texmf-config"),
                "TEXMFVAR": str(PREFIX / "texmf-var"),
                "TEXMFCACHE": str(PREFIX / "texmf-var"),
                "VARTEXFONTS": str(PREFIX / "texmf-var/fonts"),
                "XDG_CACHE_HOME": str(PREFIX / "cache"),
                "XDG_CONFIG_HOME": str(PREFIX / "config")})
    return env


def main() -> int:
    config = PREFIX / "texmf-config/tlmgr/config"
    config.parent.mkdir(parents=True, exist_ok=True)
    config.write_text("# Project-local; require all repositories to be signed.\nverify-repo = all\n")
    packages = json.loads((ROOT / "tools/texlive/package-roots.json").read_text())["packages"]
    command = ["/usr/bin/perl", str(BIN / "tlmgr"), "--verify-repo=all",
               "--repository", REPO, "install", *packages]
    env = local_environment()
    record = {"command": command, "HOME_unchanged": env.get("HOME") == os.environ.get("HOME"),
              "PATH_scope": "Child process only; persistent shell and system PATH unchanged",
              "isolated_paths": {key: env[key] for key in
                                 ("TMPDIR", "TEXMFHOME", "TEXMFCONFIG", "TEXMFVAR",
                                  "TEXMFCACHE", "VARTEXFONTS", "XDG_CACHE_HOME", "XDG_CONFIG_HOME")}}
    (EVIDENCE / "texlive-packages-command.json").write_text(json.dumps(record, indent=2) + "\n")
    with (EVIDENCE / "texlive-packages.log").open("w") as log:
        process = subprocess.Popen(command, cwd=ROOT / ".local/texlive-acquisition", env=env,
                                   stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
        assert process.stdout is not None
        for line in process.stdout:
            log.write(line)
            log.flush()
            print(line, end="", flush=True)
        return process.wait()


if __name__ == "__main__":
    raise SystemExit(main())
