#!/usr/bin/env python3
"""Fixed local P18 suites: rebuild own crate, bind/archive before test, recheck."""
from __future__ import annotations
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess

SUITES = {
    "baseline": ["--exact", "security_service::execution::tests::phase2_adapter_alone_does_not_attest_content_freshness", "--nocapture"],
    "generation": ["security_service::generation::tests::", "--nocapture"],
    "all": ["--include-ignored", "--skip", "p18_os_probe_child", "--skip", "compiler_adapter_real_tex_offline", "--skip", "generation_real_tex_offline", "--nocapture"],
    "tex": ["--ignored", "--exact", "security_service::generation::tex_tests::generation_real_tex_offline", "--nocapture"],
}


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def save(path: Path, value) -> None:
    with path.open("x", encoding="utf-8") as output:
        json.dump(value, output, ensure_ascii=False, indent=2)
        output.write("\n")


def main(*, legacy_phase2: bool = False) -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--phase", type=Path, required=True)
    parser.add_argument("--rust-bin", type=Path, required=True)
    parser.add_argument("--tex-prefix", type=Path, required=True)
    parser.add_argument("--label", required=True)
    parser.add_argument("--suite", choices=SUITES, required=True)
    args = parser.parse_args()
    if not re.fullmatch(r"[a-z0-9-]+", args.label):
        raise ValueError("Invalid own fresh label")
    root = args.root.resolve(strict=True)
    phase = args.phase.resolve(strict=True)
    toolchain = args.rust_bin.resolve(strict=True)
    tmp = (phase / "tmp").resolve(strict=True)
    if not tmp.is_dir() or not tmp.is_relative_to(phase) or not phase.name.startswith("phase3-") or phase.parent.name != "p18-security":
        raise ValueError("Explicit existing own external Phase3 TMPDIR required; no fallback")
    destination = phase / "bound" / args.label
    destination.mkdir(parents=False, exist_ok=False)

    def fingerprint():
        paths = sorted(p for p in (root / "crates").rglob("*") if p.is_file() and (p.suffix == ".rs" or p.name in ("Cargo.toml", "Cargo.lock")))
        paths += [root / "Cargo.toml", root / "Cargo.lock"]
        paths += sorted(Path(__file__).parent.glob("*.py"))
        if any(p.is_symlink() for p in paths):
            raise ValueError("Symlink source is not a qualified input")
        return {str(p.relative_to(root)): sha(p) for p in paths}

    before = fingerprint()
    for relative, digest in before.items():
        archive = destination / "source" / relative
        archive.parent.mkdir(parents=True, exist_ok=True)
        with archive.open("xb") as output:
            output.write((root / relative).read_bytes())
        if sha(archive) != digest:
            raise ValueError("Source changed during archival")
    environment = {
        "PATH": str(toolchain) + ":/usr/bin:/bin:/usr/sbin:/sbin",
        "CARGO_HOME": str((phase / "cargo-home").resolve(strict=True)),
        "RUSTC": str(toolchain / "rustc"), "RUSTDOC": str(toolchain / "rustdoc"),
        "TMPDIR": str(tmp), "CARGO_INCREMENTAL": "0", "LANG": "en_US.UTF-8",
        "CLANG_MODULE_CACHE_PATH": str(tmp / "clang-modules"),
        "LATEXIDE_P18_PHASE3_ROOT": str(phase), "LATEXIDE_P18_PHASE2_ROOT": str(phase),
        "LATEXIDE_P18_TEX_RUN": args.label, "LATEXIDE_P18_TEX_PREFIX": str(args.tex_prefix.resolve(strict=True)),
        "P18_SYNTHETIC_SECRET": "OWN_P18_PHASE3_TEST_ONLY",
    }
    programs = [toolchain / name for name in ("cargo", "rustc", "rustdoc")]
    programs += [Path(name) for name in ("/usr/bin/sandbox-exec", "/usr/bin/perl", "/bin/sh", "/bin/bash", "/System/Library/Sandbox/Profiles/dyld-support.sb")]
    tool_before = {str(p): sha(p) for p in programs}
    # Never delete previous artifacts or bypass Cargo's clean safety tag. Each
    # run gets a previously nonexistent target, so its binary is freshly built.
    own_target = phase / "target" / args.label
    if own_target.exists():
        raise ValueError("A fresh per-label target is required")
    build = [str(toolchain / "cargo"), "test", "-p", "latexide-core", "--test", "security_policy_contract", "--no-run", "--message-format=json", "--locked", "--offline", "--target-dir", str(own_target)]
    steps = []
    build_result = None
    for name, argv in (("build", build),):
        result = subprocess.run(argv, cwd=root, env=environment, capture_output=True, timeout=120)
        for suffix, body in (("stdout", result.stdout), ("stderr", result.stderr)):
            with (destination / f"{name}.{suffix}.log").open("xb") as output:
                output.write(body)
        steps.append({"name": name, "argv": argv, "exit": result.returncode})
        if result.returncode:
            save(destination / "failed-build.json", {"steps": steps, "source_files": before, "source_unchanged": before == fingerprint(), "test_state": "unrun"})
            print(result.stderr.decode(errors="replace")[-6000:])
            raise SystemExit(result.returncode)
        build_result = result
    candidates = []
    for line in build_result.stdout.splitlines():
        message = json.loads(line)
        if message.get("reason") == "compiler-artifact" and message.get("target", {}).get("name") == "security_policy_contract" and message.get("executable"):
            if message.get("fresh") is not False:
                raise ValueError("Expected an actual fresh core/test compilation, not an old binary")
            candidates.append(Path(message["executable"]).resolve(strict=True))
    if len(candidates) != 1 or not candidates[0].is_relative_to(own_target.resolve(strict=True)):
        raise ValueError("Exactly the fixed own test binary must be built")
    binary = candidates[0]
    binary_hash = sha(binary)
    archived = destination / "security_policy_contract.binary"
    with archived.open("xb") as output:
        output.write(binary.read_bytes())
    if sha(archived) != binary_hash or fingerprint() != before:
        raise ValueError("Source/binary changed before test")
    selected_args = SUITES[args.suite]
    if legacy_phase2 and args.suite == "tex":
        selected_args = ["--ignored", "--exact", "security_service::execution::tests::compiler_adapter_real_tex_offline", "--nocapture"]
    list_argv = [str(binary), *[arg for arg in selected_args if arg != "--nocapture"], "--list", "--format", "terse"]
    listed = subprocess.run(list_argv, cwd=root, env=environment, capture_output=True, timeout=30)
    for suffix, body in (("stdout", listed.stdout), ("stderr", listed.stderr)):
        with (destination / f"selection.{suffix}.log").open("xb") as output:
            output.write(body)
    identities = [line[:-6] for line in listed.stdout.decode("utf-8").splitlines() if line.endswith(": test")]
    if args.suite in ("tex", "baseline"):
        required = {selected_args[selected_args.index("--exact") + 1]}
    elif args.suite == "generation":
        required = {"security_service::generation::tests::source_digest_not_only_inode_size_or_mtime_controls_capture", "security_service::generation::tests::compiler_reads_only_its_captured_bytes_and_current_result_is_host_locked"}
    else:
        required = {"contract_v1_first_slice", "os_subprocess_boundary_probe", "review_focus_r2"}
        if not legacy_phase2:
            required.add("security_service::generation::tests::compiler_reads_only_its_captured_bytes_and_current_result_is_host_locked")
    selection_valid = listed.returncode == 0 and len(identities) == len(set(identities)) and bool(identities) and required <= set(identities)
    if args.suite in ("tex", "baseline"):
        selection_valid = selection_valid and set(identities) == required
    if not selection_valid:
        save(destination / "failed-selection.json", {"status": "failed", "source_files": before, "source_unchanged": fingerprint() == before,
             "build_steps": steps, "fresh_test_compilation": True, "binary_sha256_before": binary_hash,
             "archived_binary": str(archived), "list_argv": list_argv, "list_exit": listed.returncode,
             "selected_test_ids": identities, "required_test_ids": sorted(required), "test_state": "unrun_required_identity_missing",
             "log_sha256": {p.name: sha(p) for p in destination.glob("*.log")}})
        print(json.dumps({"status": "failed", "reason": "required_test_identity_missing", "selected": len(identities), "receipt": str(destination / "failed-selection.json")}))
        raise SystemExit(2)
    command = [str(binary), *selected_args]
    with (destination / "test.log").open("xb") as output:
        tested = subprocess.run(command, cwd=root, env=environment, stdout=output, stderr=subprocess.STDOUT, timeout=120)
    text = (destination / "test.log").read_text()
    rows = re.findall(r"test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; \d+ measured; (\d+) filtered out;", text)
    summary = dict(zip(("passed", "failed", "ignored", "filtered"), map(int, rows[-1]))) if rows else None
    unchanged = before == fingerprint()
    binary_unchanged = sha(binary) == binary_hash
    tools_unchanged = tool_before == {str(p): sha(p) for p in programs}
    valid_summary = bool(summary) and summary["passed"] == len(identities) and summary["failed"] == summary["ignored"] == 0
    passed = tested.returncode == 0 and unchanged and binary_unchanged and tools_unchanged and valid_summary
    receipt = {"schema": 1, "phase": 3, "label": args.label, "suite": args.suite, "legacy_phase2_suite": legacy_phase2,
               "status": "passed" if passed else "failed", "selection_argv": list_argv, "selected_test_ids": identities, "required_test_ids": sorted(required),
               "source_files": before, "source_unchanged": unchanged, "build_steps": steps,
               "fresh_test_compilation": True, "test_argv": command, "test_exit": tested.returncode,
               "test_summary": summary, "binary": str(binary), "binary_sha256_before": binary_hash,
               "archived_binary": str(archived), "binary_unchanged": binary_unchanged,
               "tool_sha256_before": tool_before, "tools_unchanged": tools_unchanged,
               "environment": environment, "log_sha256": {p.name: sha(p) for p in destination.glob("*.log")},
               "scope": "own standalone host-generation/OS tests, not production Runtime or OS immutability"}
    save(destination / "receipt.json", receipt)
    print(json.dumps({"receipt": str(destination / "receipt.json"), "test_exit": tested.returncode,
                      "summary": summary, "source_unchanged": unchanged, "binary_unchanged": binary_unchanged,
                      "tools_unchanged": tools_unchanged, "fresh_test_compilation": True}))
    if tested.returncode:
        print(text[-6000:])
    if not passed:
        raise SystemExit(tested.returncode or 1)


if __name__ == "__main__":
    main()
