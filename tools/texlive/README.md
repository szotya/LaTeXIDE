# Project-local TeX Live 2026

This is a narrow portable installation for the local IDE. No shell startup,
global PATH, `/usr/local/texlive`, `/Library/TeX`, or system font changes are used.
The Mac install prefix contains spaces and remains under this project.

The official 2026-10-30 Unix and Windows installer payloads are retained in
`.local/texlive-acquisition/downloads/`. Both were checked against SHA-512 metadata
whose detached GPG signatures were verified before execution. The signing primary
key fingerprint is `C78B82D8C79512F79CC0D7C80D5E5D9106BAB6BC`, as documented by
[TUG](https://tug.org/texlive/verify.html). The key and fingerprint were obtained
through official TUG HTTPS; there was no independent out-of-band key validation.

## Acquisition and installation

From the project root:

```sh
python3 tools/texlive/acquire_installers.py
python3 tools/texlive/install_local.py
python3 tools/texlive/install_local.py --install
python3 tools/texlive/install_packages.py
python3 tools/texlive/smoke_compile.py
```

The first command validates captured signed metadata and payload hashes. The
second command verifies the repository/profile without installing. The third
installs only the infrastructure into `.local/texlive-macos`. The checked-in Mac profile is a template: the helper resolves every TeX path from the actual checkout into an operation-local profile and records that effective profile in its command evidence. The package step
adds the roots in [package-roots.json](package-roots.json), with dependencies.
The final step checks a controlled Unicode, multifile, math/font example in
normal mode and with an OS sandbox denying networking. These commands do not
run a Tectonic source build.

Installer URLs are moving daily endpoints. The captured 2026-10-30 checksums
intentionally make later changed payloads fail validation rather than silently
updating the toolchain. This folder does not yet contain a frozen complete
package repository. Installed package revisions and upstream container checksums
are captured in `evidence/acquisition/compiler-review/texlive-installed-packages.json`.

## Runtime environment

Let `P` be the absolute project prefix `.local/texlive-macos`, and create one
private, space-free temporary directory under `/private/tmp` for each operation.
The tested native binary directory is `P/bin/universal-darwin`.

| Setting | Tested value |
|---|---|
| Perl | `/usr/bin/perl`, 5.34.1 |
| Child PATH | `P/bin/universal-darwin:/usr/bin:/bin:/usr/sbin:/sbin` |
| `TEXMFHOME` | `P/texmf-local` |
| `TEXMFCONFIG` | `P/texmf-config` |
| `TEXMFVAR`, `TEXMFCACHE` | `P/texmf-var` |
| `VARTEXFONTS` | `P/texmf-var/fonts` |
| `XDG_CACHE_HOME` | `P/cache` |
| `XDG_CONFIG_HOME` | `P/config` |
| `TMPDIR` | `/private/tmp/latexide-texlive-<unique>` |
| `HOME` | Original value, never redirected |

Do not inherit `PERL5LIB`, `PERL5OPT`, `TEXMFCNF`, or external TeX search trees.
The helper keeps HOME/USER/LOGNAME/language values and supplies the local paths
above. The portable config also resolves TEXMFSYSVAR and TEXMFSYSCONFIG under P.
No environment change persists beyond the child process.

The space-free TMPDIR matters: upstream tlmgr constructs a fmtutil
`--status-file` shell argument without quoting it. A TMPDIR inside this
space-containing project failed during format generation; using `/private/tmp`
fixed it while leaving the installation prefix unchanged. The retained error
and recovery are documented in `texlive-local-issues.json` and the format logs.

Use `latexmk -norc -xelatex` with `-latexoption=-no-shell-escape` and a timeout.
The tested package set qualifies the XeLaTeX path; other engine modes have not
been qualified by this task. Latin Modern is available by TeX font filename:

```tex
\setmainfont{lmroman10-regular.otf}[
  BoldFont=lmroman10-bold.otf,
  ItalicFont=lmroman10-italic.otf,
  BoldItalicFont=lmroman10-bolditalic.otf]
```

This avoids requiring global macOS font registration. `amsfonts` is required
alongside `amsmath` for small Computer Modern extension metrics such as cmex7.

For package management, keep `--verify-repo=all`; the project-local
`texmf-config/tlmgr/config` enforces it too. The installer used
`--verify-downloads --warn-checksums`. Package repository loading reported
`(verified)`, with ordinary per-container checksum checking enabled.

## Windows handoff

The official `install-tl-windows.exe` is acquired and GPG/SHA-512 verified, but
has not been executed. Its installer bootstrap architecture is distinct from
the selected TeX binary platform: [windows-x64.profile](windows-x64.profile)
explicitly selects `binary_windows 1`, which is TeX Live's Intel/AMD x64 target.

Replace all `C:/LateX_IDE` paths in that profile with the actual Windows project
root. The shared Windows prefix is `.local/texlive-windows-x64`, matching `scripts/build_windows.ps1`. Use the official installer's **Unpack only** action, then run its
`install-tl-windows.bat` with the profile, `--portable --no-interaction`, the
official HTTPS repository, `--verify-downloads --warn-checksums`, and a working
GPG verifier. After the infrastructure bootstrap, install the identical roots
from package-roots.json with `tlmgr --verify-repo=all`. Keep its TeX/cache trees
under the Windows project and use a space-free temporary directory.

This is a precise acquisition/profile handoff, not a Windows runtime result.

## Evidence and remaining qualification

The runtime/version/path manifest is
`evidence/acquisition/compiler-review/texlive-runtime-manifest.json`; package
revisions, installer hashes/signatures, online/offline command logs, a kernel
network-denial probe, and the unchanged application fixture result are adjacent.
The offline PDF succeeded with all networking denied. `-no-shell-escape` and
network isolation do not establish a filesystem sandbox for arbitrary projects.

No binary malware scan or complete Rust/native dependency audit was performed.
Official provenance and hash/signature checks are not a malware-free guarantee.
Before redistribution, retain applicable license notices and sources, freeze
the full package snapshot, and qualify real Windows 11 x64 execution and a
broader document corpus.
