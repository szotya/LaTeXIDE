# Evidence publication boundary

Raw execution logs, screenshots, generated outputs, copied historical snapshots and captured machine metadata stay local. See [local artifact hashes and original link targets](../docs/development/LOCAL_EVIDENCE.md). Historical documentation is not proof of a fresh clone or a native platform result.

Only these source/provenance classes are published here:

- `ide-audit-2026-10-31/{build_audit_artifacts,validate_audit,audit_probes}.py`: reproducible audit source. The historical validator requires the local evidence tree; use `scripts/validate_development_packages.py` for clean-clone plan checks.
- `acquisition/compiler-review/texlive-installer-signatures/{install-tl-unx.tar.gz.sha512,install-tl-unx.tar.gz.sha512.asc,install-tl-windows.exe.sha512,install-tl-windows.exe.sha512.asc,texlive.asc,texlive-keyring.gpg}`: upstream public checksums, signatures and public verification keys used by the installer helper.

No private signing key, credential, live session descriptor, installer payload or executable is published. New raw outputs remain ignored; an explicit publication decision is required to expand this boundary.
