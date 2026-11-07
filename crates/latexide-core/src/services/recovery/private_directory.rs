//! macOS recovery-directory preparation. No chmod, symlink following or cleanup.
//! The held parent constrains mkdirat; this is not a global rename/confinement
//! guarantee against a same-UID actor relocating an already held directory.
use super::{DurableError, Phase, Result, io, private_dir, uid};
use std::{
    ffi::{CString, OsStr, c_char},
    fs::{self, File, Metadata, OpenOptions},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{
            ffi::OsStrExt,
            fs::{MetadataExt, OpenOptionsExt},
        },
    },
    path::{Component, Path},
};

// macOS SDK sys/fcntl.h. ANY already includes the final component: do not OR
// O_NOFOLLOW with it (Darwin rejects that combination with EINVAL).
const OPEN_DIRECTORY: i32 = 0x00100000 | 0x01000000 | 0x20000000;
unsafe extern "C" {
    fn mkdirat(fd: i32, path: *const c_char, mode: u16) -> i32;
    fn openat(fd: i32, path: *const c_char, flags: i32, ...) -> i32;
}

fn absolute(path: &Path) -> Result<()> {
    if !path.is_absolute()
        || path
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
    {
        return Err(DurableError::UnsafePath);
    }
    Ok(())
}
fn open_error(error: std::io::Error) -> DurableError {
    // An unavailable kernel/filesystem primitive is an explicit unsupported
    // result, never permission to retry without no-follow protection.
    if matches!(error.raw_os_error(), Some(22 | 45 | 78)) {
        DurableError::UnconfirmedPlatform
    } else {
        DurableError::Io {
            phase: Phase::CheckpointWrite,
            kind: error.kind(),
        }
    }
}
pub(crate) fn checked_open(path: &Path, before: &Metadata) -> Result<File> {
    absolute(path)?;
    if !before.is_dir() || before.file_type().is_symlink() {
        return Err(DurableError::UnsafePath);
    }
    // Kernel enforcement covers every existing ancestor, including a late
    // ancestor link swap after symlink_metadata. Opening is read-only.
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(OPEN_DIRECTORY)
        .open(path)
        .map_err(open_error)?;
    let after = io(file.metadata(), Phase::CheckpointWrite)?;
    if !after.is_dir()
        || after.dev() != before.dev()
        || after.ino() != before.ino()
        || after.uid() != before.uid()
        || after.mode() != before.mode()
    {
        return Err(DurableError::UnsafePath);
    }
    Ok(file)
}
fn private(file: &File) -> Result<()> {
    let m = io(file.metadata(), Phase::CheckpointWrite)?;
    if !m.is_dir() || m.uid() != uid() || m.mode() & 0o777 != 0o700 {
        return Err(DurableError::UnsafePath);
    }
    Ok(())
}
pub(crate) fn create_child(parent: &File, name: &OsStr) -> Result<File> {
    if name.is_empty() || name == "." || name == ".." || name.as_bytes().contains(&b'/') {
        return Err(DurableError::UnsafePath);
    }
    let name = CString::new(name.as_bytes()).map_err(|_| DurableError::UnsafePath)?;
    // A path swap cannot redirect this operation through an ancestor symlink.
    // Existing entries (including a just-inserted leaf link) fail, no repair.
    if unsafe { mkdirat(parent.as_raw_fd(), name.as_ptr(), 0o700) } != 0 {
        return Err(open_error(std::io::Error::last_os_error()));
    }
    let fd = unsafe { openat(parent.as_raw_fd(), name.as_ptr(), OPEN_DIRECTORY) };
    if fd < 0 {
        return Err(open_error(std::io::Error::last_os_error()));
    }
    let child = unsafe { File::from_raw_fd(fd) };
    private(&child)?;
    io(parent.sync_all(), Phase::CheckpointParentSync)?;
    Ok(child)
}
pub(super) fn create(path: &Path) -> Result<()> {
    absolute(path)?;
    let parent_path = path.parent().ok_or(DurableError::UnsafePath)?;
    let before = fs::symlink_metadata(parent_path).map_err(open_error)?;
    // Validate/open before the first side effect; ordinary project ancestors
    // may be 0755. Only recovery-owned directories require private 0700.
    let parent = checked_open(parent_path, &before)?;
    let child = create_child(&parent, path.file_name().ok_or(DurableError::UnsafePath)?)?;
    let expected = io(child.metadata(), Phase::CheckpointWrite)?;
    let rebound = checked_open(path, &expected)?;
    private(&rebound)
}
pub(super) fn ensure(path: &Path) -> Result<()> {
    absolute(path)?;
    match fs::symlink_metadata(path) {
        Ok(before) => {
            let file = checked_open(path, &before)?;
            private(&file)?;
            private_dir(path)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let parent = path.parent().ok_or(DurableError::UnsafePath)?;
            match fs::symlink_metadata(parent) {
                Ok(before) => {
                    checked_open(parent, &before)?;
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => ensure(parent)?,
                Err(e) => return Err(open_error(e)),
            }
            create(path)
        }
        Err(e) => Err(open_error(e)),
    }
}
