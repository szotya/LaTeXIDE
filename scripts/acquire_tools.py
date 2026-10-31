#!/usr/bin/env python3
"""Acquire pinned upstream tools locally; hashes prove integrity, not malware absence."""
import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath, PureWindowsPath
import re
import shutil
import stat
import tarfile
import urllib.parse
import urllib.request
import zipfile

ROOT = Path(__file__).resolve().parents[1]
LOCAL = ROOT / ".local"
LOCK = ROOT / "tools/toolchain-lock.json"


def verify_file(path, expected):
    if not re.fullmatch(r"[a-f0-9]{64}", expected):
        raise ValueError("Invalid expected SHA-256")
    with Path(path).open("rb") as source:
        digest = hashlib.file_digest(source, "sha256").hexdigest()
    if digest != expected:
        raise ValueError(f"SHA-256 mismatch: {path.name}: expected {expected}, got {digest}")
    return digest


def safe_member(destination, name):
    normalized = name.replace("\\", "/")
    relative = PurePosixPath(normalized)
    if relative.is_absolute() or ".." in relative.parts or PureWindowsPath(name).drive:
        raise ValueError(f"Unsafe archive path: {name}")
    result = (destination / relative).resolve()
    if not result.is_relative_to(destination.resolve()):
        raise ValueError(f"Archive path escapes destination: {name}")
    return result


def safe_extract(archive, destination):
    archive, destination = Path(archive), Path(destination)
    destination.mkdir(parents=True, exist_ok=True)
    if archive.suffix == ".zip":
        with zipfile.ZipFile(archive) as source:
            members = source.infolist()
            if len(members) > 100_000 or sum(m.file_size for m in members) > 2 * 1024**3:
                raise ValueError("Archive exceeds extraction limits")
            for member in members:
                safe_member(destination, member.filename)
                if stat.S_ISLNK(member.external_attr >> 16):
                    raise ValueError("ZIP symlinks are not supported")
            for member in members:
                target = safe_member(destination, member.filename)
                if member.is_dir():
                    target.mkdir(parents=True, exist_ok=True)
                else:
                    target.parent.mkdir(parents=True, exist_ok=True)
                    with source.open(member) as incoming, target.open("wb") as outgoing:
                        shutil.copyfileobj(incoming, outgoing)
    else:
        with tarfile.open(archive) as source:
            members = source.getmembers()
            if len(members) > 100_000 or sum(m.size for m in members) > 2 * 1024**3:
                raise ValueError("Archive exceeds extraction limits")
            for member in members:
                target = safe_member(destination, member.name)
                if member.isdev() or member.isfifo():
                    raise ValueError("Special archive entries are not allowed")
                if member.issym() or member.islnk():
                    link = (target.parent if member.issym() else destination) / member.linkname
                    if not link.resolve().is_relative_to(destination.resolve()):
                        raise ValueError("Archive symlink escapes destination")
            source.extractall(destination, members=members, filter="data")


def download(asset):
    parsed = urllib.parse.urlparse(asset["url"])
    if parsed.scheme != "https" or parsed.hostname not in {"github.com", "static.rust-lang.org"}:
        raise ValueError("Tool URL is outside the pinned official origins")
    target = LOCAL / "downloads" / asset["filename"]
    target.parent.mkdir(parents=True, exist_ok=True)
    if target.exists():
        verify_file(target, asset["sha256"])
        return target
    temporary = target.with_suffix(target.suffix + ".part")
    request = urllib.request.Request(asset["url"], headers={"User-Agent": "LaTeX-IDE-development/0.1"})
    with urllib.request.urlopen(request, timeout=60) as incoming, temporary.open("wb") as outgoing:
        shutil.copyfileobj(incoming, outgoing, length=1024 * 1024)
    verify_file(temporary, asset["sha256"])
    temporary.replace(target)
    return target


def install_rust_component(extracted, destination):
    """Copy the verified component payload without the upstream shell installer's space parsing."""
    package = next(Path(extracted).glob("*/components")).parent
    destination = Path(destination)
    destination.mkdir(parents=True, exist_ok=True)
    for component in (package / "components").read_text().splitlines():
        source = safe_member(package, component)
        if not source.is_dir():
            raise ValueError(f"Missing Rust component payload: {component}")
        for entry in source.iterdir():
            if entry.name == "manifest.in":
                continue
            target = destination / entry.name
            if entry.is_dir():
                shutil.copytree(entry, target, dirs_exist_ok=True, symlinks=True)
            else:
                shutil.copy2(entry, target, follow_symlinks=False)


def acquire(asset):
    print(f"Acquiring {asset['id']}", flush=True)
    archive = download(asset)
    extracted = LOCAL / "extracted" / asset["id"]
    marker = extracted / ".verified-sha256"
    if not marker.exists() or marker.read_text().strip() != asset["sha256"]:
        if extracted.exists():
            shutil.rmtree(extracted)
        safe_extract(archive, extracted)
        marker.write_text(asset["sha256"] + "\n")
    if asset["kind"] == "rust-component":
        install_rust_component(extracted, LOCAL / "rust")
        installed = LOCAL / "rust"
    else:
        binary_name = "tectonic.exe" if asset["platform"] == "windows-x64" else "tectonic"
        source = next(extracted.rglob(binary_name))
        installed = LOCAL / "compilers" / asset["platform"] / binary_name
        installed.parent.mkdir(parents=True, exist_ok=True)
        staging = installed.with_suffix(installed.suffix + ".new")
        shutil.copy2(source, staging)
        staging.chmod(0o755)
        staging.replace(installed)
    return {"id": asset["id"], "url": asset["url"], "version": asset["version"],
            "upstream_sha256": asset["sha256"], "observed_sha256": verify_file(archive, asset["sha256"]),
            "installed_path": str(installed.relative_to(ROOT)), "integrity": "verified",
            "malware_scan": "not_performed", "signature_verification": "not_performed"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--metadata-only", action="store_true")
    parser.add_argument("--install", action="store_true")
    args = parser.parse_args()
    lock = json.loads(LOCK.read_text())
    if args.metadata_only or not args.install:
        print(json.dumps(lock, indent=2))
        return
    results = [acquire(asset) for asset in lock["assets"]]
    evidence = ROOT / "evidence/acquisition/verified-tools.json"
    evidence.parent.mkdir(parents=True, exist_ok=True)
    evidence.write_text(json.dumps({"schema": 1, "assets": results}, indent=2) + "\n")
    print(f"Integrity verified for {len(results)} pinned assets. Binary malware status remains unassessed.")


if __name__ == "__main__":
    main()
