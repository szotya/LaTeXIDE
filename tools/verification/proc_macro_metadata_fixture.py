"""Narrow malformed proc_macro metadata fixtures and diagnostic classification.

The basename and scope are supplied by a trusted caller. This module neither
finds toolchains nor runs commands, Rust, Cargo, or a diagnostic-provided path.
POSIX file-descriptor guards are required; unsupported backends fail closed.
Content classification is not authentication of a compiler process.
"""
from contextlib import ExitStack
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import re
import stat

MALFORMED_SIZE = 31
MALFORMED_SHA256 = "c11d5d8e8d6d8724bb35bd9b78fa2a93cfe3d5dff0b99f45aefb380a90a6a262"
MAX_DIAGNOSTIC_BYTES = 65536
_FD_BACKEND = (
    all(hasattr(os, name) for name in ("O_DIRECTORY", "O_NOFOLLOW", "O_NONBLOCK", "getuid"))
    and all(fn in os.supports_dir_fd for fn in (os.open, os.mkdir, os.stat))
)
_ROW_KEYS = {"$message_type", "message", "code", "level", "spans", "children", "rendered"}
_NOTE_KEYS = {"message", "code", "level", "spans", "children", "rendered"}


class FixtureError(ValueError):
    """The requested fixture is missing, unsafe, colliding, or not frozen."""


@dataclass(frozen=True)
class MalformedRmeta:
    path: Path
    basename: str
    size: int
    sha256: str


def _absolute(path):
    try:
        result = Path(path)
    except (TypeError, ValueError) as exc:
        raise FixtureError("path required") from exc
    if not result.is_absolute() or ".." in result.parts or "\x00" in str(result):
        raise FixtureError("absolute non-traversing path required")
    return result


def _checked_basename(name):
    if type(name) is not str or re.fullmatch(r"libproc_macro-[0-9a-f]{16}\.rmeta", name) is None:
        raise FixtureError("exact positive proc_macro metadata basename required")
    return name


def _private_directory(fd):
    row = os.fstat(fd)
    if not stat.S_ISDIR(row.st_mode) or row.st_uid != os.getuid() or stat.S_IMODE(row.st_mode) != 0o700:
        raise FixtureError("own private directory required")
    return row


def _open_scope(path):
    flags = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW
    fd = os.open("/", flags)
    try:
        for part in path.parts[1:]:
            following = os.open(part, flags, dir_fd=fd)
            os.close(fd)
            fd = following
        _private_directory(fd)
        return fd
    except BaseException:
        os.close(fd)
        raise


def _open_private_child(parent, name):
    fd = os.open(name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=parent)
    try:
        _private_directory(fd)
        return fd
    except BaseException:
        os.close(fd)
        raise


def _signature(row):
    return (
        row.st_dev, row.st_ino, row.st_mode, row.st_uid, row.st_nlink,
        row.st_size, row.st_mtime_ns, row.st_ctime_ns,
    )


def _regular_private(row):
    if (
        not stat.S_ISREG(row.st_mode) or row.st_uid != os.getuid()
        or stat.S_IMODE(row.st_mode) != 0o600 or row.st_nlink != 1
        or row.st_size != MALFORMED_SIZE
    ):
        raise FixtureError("one own private frozen 31-byte file required")


def _read_malformed(owner_fd):
    initial = os.stat("malformed.bytes", dir_fd=owner_fd, follow_symlinks=False)
    _regular_private(initial)
    fd = os.open(
        "malformed.bytes", os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=owner_fd,
    )
    try:
        before = os.fstat(fd)
        _regular_private(before)
        if _signature(before) != _signature(initial):
            raise FixtureError("malformed source changed before read")
        payload = os.read(fd, MALFORMED_SIZE + 1)
        after = os.fstat(fd)
        named_after = os.stat("malformed.bytes", dir_fd=owner_fd, follow_symlinks=False)
        if _signature(before) != _signature(after) or _signature(after) != _signature(named_after):
            raise FixtureError("malformed source changed during read")
        if len(payload) != MALFORMED_SIZE or hashlib.sha256(payload).hexdigest() != MALFORMED_SHA256:
            raise FixtureError("malformed source is not the frozen bytes")
        return payload
    finally:
        os.close(fd)


def create_malformed_rmeta(scope_root, owner_root, positive_basename, malformed_source):
    """Create owner_root/bad/<positive basename> without reusing any destination.

    owner_root must be a private direct child named by a 32-hex fixture nonce.
    malformed_source must be its own regular, private malformed.bytes file.
    The positive/toolchain file is never passed, opened, or written.
    Failures preserve any newly created partial fixture for the caller to inspect.
    """
    if not _FD_BACKEND:
        raise FixtureError("required POSIX descriptor backend unavailable")
    scope = _absolute(scope_root)
    owner = _absolute(owner_root)
    source = _absolute(malformed_source)
    name = _checked_basename(positive_basename)
    if Path(name).name != name or name in ("", ".", ".."):
        raise FixtureError("single destination component required")
    if owner.parent != scope or re.fullmatch(r"[0-9a-f]{32}", owner.name) is None:
        raise FixtureError("owner outside the supplied fixture scope")
    if source != owner / "malformed.bytes":
        raise FixtureError("malformed source outside its exact owner slot")
    try:
        with ExitStack() as stack:
            scope_fd = _open_scope(scope)
            stack.callback(os.close, scope_fd)
            owner_fd = _open_private_child(scope_fd, owner.name)
            stack.callback(os.close, owner_fd)
            payload = _read_malformed(owner_fd)
            os.mkdir("bad", mode=0o700, dir_fd=owner_fd)
            bad_fd = _open_private_child(owner_fd, "bad")
            stack.callback(os.close, bad_fd)
            fd = os.open(
                name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
                0o600, dir_fd=bad_fd,
            )
            try:
                written = 0
                while written < len(payload):
                    count = os.write(fd, payload[written:])
                    if count <= 0:
                        raise FixtureError("short fixture write")
                    written += count
                os.fsync(fd)
                _regular_private(os.fstat(fd))
            finally:
                os.close(fd)
            fd = os.open(name, os.O_RDONLY | os.O_NOFOLLOW, dir_fd=bad_fd)
            try:
                row = os.fstat(fd)
                _regular_private(row)
                observed = os.read(fd, MALFORMED_SIZE + 1)
                named = os.stat(name, dir_fd=bad_fd, follow_symlinks=False)
                if _signature(row) != _signature(named):
                    raise FixtureError("destination identity changed")
            finally:
                os.close(fd)
            if observed != payload or hashlib.sha256(observed).hexdigest() != MALFORMED_SHA256:
                raise FixtureError("destination readback differs")
            return MalformedRmeta(owner / "bad" / name, name, MALFORMED_SIZE, MALFORMED_SHA256)
    except OSError as exc:
        raise FixtureError("fixture operation refused: " + str(exc)) from exc


def _unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("duplicate diagnostic key")
        result[key] = value
    return result


def _nonfinite(value):
    raise ValueError("nonfinite diagnostic constant: " + value)


def _row_shape(row):
    if type(row) is not dict or set(row) != _ROW_KEYS:
        return False
    if row["$message_type"] != "diagnostic" or type(row["message"]) is not str:
        return False
    if type(row["spans"]) is not list or type(row["children"]) is not list:
        return False
    if row["rendered"] is not None and type(row["rendered"]) is not str:
        return False
    code = row["code"]
    return code is None or (
        type(code) is dict and set(code) == {"code", "explanation"}
        and type(code["code"]) is str
        and (code["explanation"] is None or type(code["explanation"]) is str)
    )


def _metadata_error_matches(row, expected_path):
    if (
        row["level"] != "error"
        or row["message"] != "found invalid metadata files for crate `proc_macro`"
        or type(row["code"]) is not dict or row["code"]["code"] != "E0786"
        or len(row["children"]) != 1
    ):
        return False
    note = row["children"][0]
    return (
        type(note) is dict and set(note) == _NOTE_KEYS
        and note["message"] == "corrupt metadata encountered in " + str(expected_path)
        and note["level"] == "note" and note["code"] is None
        and note["spans"] == [] and note["children"] == [] and note["rendered"] is None
    )


def is_malformed_proc_macro_refusal(returncode, stdout, stderr, *, expected_metadata_path):
    """Classify the narrow pinned E0786 protocol; never execute diagnostic text.

    A matching record is caller-supplied metadata, not evidence of process origin.
    expected_metadata_path is compared as a locator string and is never opened.
    """
    if type(returncode) is not int or returncode != 1 or type(stdout) is not bytes or stdout:
        return False
    if type(stderr) is not bytes or not stderr or len(stderr) > MAX_DIAGNOSTIC_BYTES:
        return False
    try:
        expected = _absolute(expected_metadata_path)
        _checked_basename(expected.name)
        if expected.parent.name != "bad":
            return False
        rows = [
            json.loads(line, object_pairs_hook=_unique_object, parse_constant=_nonfinite)
            for line in stderr.decode("utf-8").splitlines() if line.strip()
        ]
        if len(rows) != 3 or not all(_row_shape(row) for row in rows):
            return False
        error, abort, note = rows
        return (
            _metadata_error_matches(error, expected)
            and abort["level"] == "error"
            and abort["message"] == "aborting due to 1 previous error"
            and abort["code"] is None and abort["children"] == [] and abort["spans"] == []
            and note["level"] == "failure-note"
            and note["message"] == "For more information about this error, try `rustc --explain E0786`."
            and note["code"] is None and note["children"] == [] and note["spans"] == []
        )
    except (FixtureError, ValueError, UnicodeError, TypeError, RecursionError):
        return False
