//! Darwin descriptor-relative capture. No weak-flag or path-open fallback.
use super::Digest;
use std::{
    ffi::CString,
    fs::{File, Metadata, OpenOptions, Permissions},
    io::{self, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::fs::{FileExt, MetadataExt, OpenOptionsExt, PermissionsExt},
    },
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

// Checked against the current Darwin SDK fcntl.h / stat.h / sys/stdio.h.
const NOFOLLOW: i32 = 0x2000_0000 | 0x0100_0000 | 0x0000_0004;
const DIRECTORY: i32 = 0x0010_0000;
unsafe extern "C" {
    fn openat(fd: i32, name: *const std::ffi::c_char, flags: i32, ...) -> i32;
    fn mkdirat(fd: i32, name: *const std::ffi::c_char, mode: u16) -> i32;
    fn renameatx_np(
        from: i32,
        old: *const std::ffi::c_char,
        to: i32,
        new: *const std::ffi::c_char,
        flags: u32,
    ) -> i32;
    fn geteuid() -> u32;
}
fn invalid() -> io::Error {
    io::Error::other("input identity/content changed")
}
fn name(text: &std::ffi::OsStr) -> io::Result<CString> {
    use std::os::unix::ffi::OsStrExt;
    CString::new(text.as_bytes()).map_err(|_| invalid())
}
fn node(metadata: &Metadata, directory: bool) -> io::Result<(u64, u64)> {
    if if directory {
        !metadata.is_dir()
    } else {
        !metadata.is_file() || metadata.nlink() != 1
    } {
        return Err(invalid());
    }
    Ok((metadata.dev(), metadata.ino()))
}
fn at(parent: &File, part: &std::ffi::OsStr, directory: bool, create: bool) -> io::Result<File> {
    let part = name(part)?;
    let flags = NOFOLLOW
        | if directory { DIRECTORY } else { 0 }
        | if create { 1 | 0x0200 | 0x0800 } else { 0 };
    let fd = unsafe { openat(parent.as_raw_fd(), part.as_ptr(), flags, 0o600u32) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    let file = unsafe { File::from_raw_fd(fd) };
    node(&file.metadata()?, directory)?;
    Ok(file)
}
fn mkdir(parent: &File, part: &std::ffi::OsStr) -> io::Result<()> {
    let part = name(part)?;
    if unsafe { mkdirat(parent.as_raw_fd(), part.as_ptr(), 0o700) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
fn parts(path: &Path) -> io::Result<Vec<&std::ffi::OsStr>> {
    path.components()
        .map(|c| match c {
            Component::Normal(s) => Ok(s),
            _ => Err(invalid()),
        })
        .collect()
}
fn change(metadata: &Metadata) -> (i64, i64) {
    (metadata.ctime(), metadata.ctime_nsec())
}

pub(super) struct Directory {
    pub(super) path: PathBuf,
    file: File,
    identity: (u64, u64),
    sealed_change: Option<(i64, i64)>,
    private: bool,
}
impl Directory {
    pub(super) fn open(path: &Path, private: bool) -> io::Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(NOFOLLOW | DIRECTORY)
            .open(path)?;
        let metadata = file.metadata()?;
        let identity = node(&metadata, true)?;
        if private && (metadata.uid() != unsafe { geteuid() } || metadata.mode() & 0o077 != 0) {
            return Err(invalid());
        }
        let root = Self {
            path: path.into(),
            file,
            identity,
            sealed_change: None,
            private,
        };
        root.revalidate()?;
        Ok(root)
    }
    fn at_or_below(&self, other: &Self) -> io::Result<bool> {
        self.revalidate()?;
        other.revalidate()?;
        let mut current = self.file.try_clone()?;
        for _ in 0..128 {
            let identity = node(&current.metadata()?, true)?;
            if identity == other.identity {
                return Ok(true);
            }
            // Trusted literal only, for metadata ancestry inspection. User
            // input paths still reject all parent traversal. No data is read.
            let parent = at(&current, std::ffi::OsStr::new(".."), true, false)?;
            if node(&parent.metadata()?, true)? == identity {
                return Ok(false);
            }
            current = parent;
        }
        Err(invalid())
    }
    pub(super) fn create_private(path: &Path, excluded: &Self) -> io::Result<Self> {
        let parent = Self::open(path.parent().ok_or_else(invalid)?, true)?;
        if parent.at_or_below(excluded)? {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "generation parent is inside the original project",
            ));
        }
        let leaf = path.file_name().ok_or_else(invalid)?;
        mkdir(&parent.file, leaf)?; // exclusive mkdir; a reused generation is rejected
        parent.revalidate()?;
        let created = Self::open(path, true)?;
        if excluded.at_or_below(&created)? {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "generation contains the original project",
            ));
        }
        Ok(created)
    }
    pub(super) fn create_child(&self, part: &str) -> io::Result<Self> {
        if parts(Path::new(part))?.len() != 1 {
            return Err(invalid());
        }
        self.revalidate()?;
        mkdir(&self.file, std::ffi::OsStr::new(part))?;
        Self::open(&self.path.join(part), true)
    }
    pub(super) fn revalidate(&self) -> io::Result<()> {
        let held = self.file.metadata()?;
        let now = OpenOptions::new()
            .read(true)
            .custom_flags(NOFOLLOW | DIRECTORY)
            .open(&self.path)?
            .metadata()?;
        if node(&held, true)? != self.identity
            || node(&now, true)? != self.identity
            || self.private && (now.uid() != unsafe { geteuid() } || now.mode() & 0o077 != 0)
            || self
                .sealed_change
                .is_some_and(|c| c != change(&held) || c != change(&now))
        {
            return Err(invalid());
        }
        Ok(())
    }
    pub(super) fn hold_sealed_change(&mut self) -> io::Result<()> {
        if self.file.metadata()?.mode() & 0o222 != 0 {
            return Err(invalid());
        }
        self.sealed_change = Some(change(&self.file.metadata()?));
        Ok(())
    }
    fn open_file(&self, relative: &Path, sealed: bool) -> io::Result<(File, Vec<Route>)> {
        self.revalidate()?;
        let parts = parts(relative)?;
        if parts.is_empty() {
            return Err(invalid());
        }
        let mut parent = self.file.try_clone()?;
        let mut route = Vec::new();
        for component in &parts[..parts.len() - 1] {
            let directory = at(&parent, component, true, false)?;
            let metadata = directory.metadata()?;
            route.push(Route {
                file: directory.try_clone()?,
                identity: node(&metadata, true)?,
                change: sealed.then(|| change(&metadata)),
            });
            parent = directory;
        }
        let file = at(&parent, parts[parts.len() - 1], false, false)?;
        Ok((file, route))
    }
    pub(super) fn publish_input(&self) -> io::Result<()> {
        self.revalidate()?;
        let from = CString::new("input.pending").unwrap();
        let to = CString::new("input").unwrap();
        if unsafe {
            renameatx_np(
                self.file.as_raw_fd(),
                from.as_ptr(),
                self.file.as_raw_fd(),
                to.as_ptr(),
                4,
            )
        } != 0
        {
            return Err(io::Error::last_os_error());
        }
        self.file.sync_all()
    }
    pub(super) fn reserve_output(&self, relative: &Path) -> io::Result<OutputSlot> {
        self.revalidate()?;
        let components = parts(relative)?;
        if components.len() != 1 {
            return Err(invalid());
        }
        let file = at(&self.file, components[0], false, true)?;
        let identity = node(&file.metadata()?, false)?;
        Ok(OutputSlot { file, identity })
    }
}
/// Created exclusively by the host in the actual pre-spawn gate. Retaining the
/// inode closes the "inject an old file before first output observation" case.
/// This cannot attest which same-UID process wrote bytes into the reserved inode.
pub(super) struct OutputSlot {
    file: File,
    identity: (u64, u64),
}
impl OutputSlot {
    pub(super) fn completed(
        &self,
        root: &Directory,
        relative: &Path,
        limit: usize,
    ) -> io::Result<FileWitness> {
        let (file, _) = root.open_file(relative, false)?;
        if node(&self.file.metadata()?, false)? != self.identity
            || node(&file.metadata()?, false)? != self.identity
            || file.metadata()?.len() == 0
        {
            return Err(invalid());
        }
        let witness = FileWitness::capture(root, relative, limit, false)?;
        if witness.identity() != self.identity {
            return Err(invalid());
        }
        Ok(witness)
    }
}
struct Route {
    file: File,
    identity: (u64, u64),
    change: Option<(i64, i64)>,
}
#[derive(PartialEq, Eq)]
struct Stamp {
    node: (u64, u64),
    len: u64,
    mtime: (i64, i64),
    ctime: (i64, i64),
    mode: u32,
}
impl Stamp {
    fn of(m: &Metadata) -> io::Result<Self> {
        Ok(Self {
            node: node(m, false)?,
            len: m.len(),
            mtime: (m.mtime(), m.mtime_nsec()),
            ctime: change(m),
            mode: m.mode(),
        })
    }
}
fn read_stable(file: &File, limit: usize) -> io::Result<(Vec<u8>, Stamp)> {
    let before = Stamp::of(&file.metadata()?)?;
    if before.len > limit as u64 {
        return Err(invalid());
    }
    let mut bytes = Vec::with_capacity(before.len as usize);
    let mut block = [0; 64 * 1024]; // no single source read exceeds the existing 64KiB boundary
    loop {
        let count = file.read_at(&mut block, bytes.len() as u64)?;
        if count == 0 {
            break;
        }
        if bytes.len().checked_add(count).is_none_or(|n| n > limit) {
            return Err(invalid());
        }
        bytes.extend_from_slice(&block[..count]);
    }
    let after = Stamp::of(&file.metadata()?)?;
    if before != after || after.len != bytes.len() as u64 {
        return Err(invalid());
    }
    Ok((bytes, after))
}
pub(super) struct FileWitness {
    relative: PathBuf,
    file: File,
    route: Vec<Route>,
    stamp: Stamp,
    pub(super) digest: Digest,
    sealed: bool,
}
impl FileWitness {
    pub(super) fn capture(
        root: &Directory,
        relative: &Path,
        limit: usize,
        sealed: bool,
    ) -> io::Result<Self> {
        let (file, route) = root.open_file(relative, sealed)?;
        let metadata = file.metadata()?;
        if sealed && (metadata.uid() != unsafe { geteuid() } || metadata.mode() & 0o277 != 0) {
            return Err(invalid());
        }
        let (bytes, stamp) = read_stable(&file, limit)?;
        let digest = Digest::of(&bytes).map_err(|_| invalid())?;
        let result = Self {
            relative: relative.into(),
            file,
            route,
            stamp,
            digest,
            sealed,
        };
        result.revalidate(root, limit)?;
        Ok(result)
    }
    pub(super) fn identity(&self) -> (u64, u64) {
        self.stamp.node
    }
    pub(super) fn revalidate(&self, root: &Directory, limit: usize) -> io::Result<Vec<u8>> {
        let (current, route) = root.open_file(&self.relative, self.sealed)?;
        if route.len() != self.route.len() {
            return Err(invalid());
        }
        for (held, now) in self.route.iter().zip(&route) {
            let metadata = held.file.metadata()?;
            if node(&metadata, true)? != held.identity
                || now.identity != held.identity
                || held
                    .change
                    .is_some_and(|c| c != change(&metadata) || Some(c) != now.change)
            {
                return Err(invalid());
            }
        }
        if Stamp::of(&self.file.metadata()?)? != self.stamp
            || Stamp::of(&current.metadata()?)? != self.stamp
        {
            return Err(invalid());
        }
        let (bytes, held_stamp) = read_stable(&self.file, limit)?;
        let (current_bytes, current_stamp) = read_stable(&current, limit)?;
        if held_stamp != self.stamp
            || current_stamp != self.stamp
            || Digest::of(&bytes).map_err(|_| invalid())? != self.digest
            || Digest::of(&current_bytes).map_err(|_| invalid())? != self.digest
        {
            return Err(invalid());
        }
        root.revalidate()?;
        Ok(bytes)
    }
}
pub(super) struct Builder {
    directory: Directory,
    directories: Vec<File>,
}
impl Builder {
    pub(super) fn new(root: &Directory) -> io::Result<Self> {
        Ok(Self {
            directory: root.create_child("input.pending")?,
            directories: Vec::new(),
        })
    }
    pub(super) fn write(
        &mut self,
        relative: &Path,
        bytes: &[u8],
        cancel: &AtomicBool,
    ) -> io::Result<()> {
        let parts = parts(relative)?;
        if parts.is_empty() {
            return Err(invalid());
        }
        let mut parent = self.directory.file.try_clone()?;
        for component in &parts[..parts.len() - 1] {
            if let Err(error) = mkdir(&parent, component) {
                if error.kind() != io::ErrorKind::AlreadyExists {
                    return Err(error);
                }
            }
            parent = at(&parent, component, true, false)?;
            self.directories.push(parent.try_clone()?);
        }
        let mut file = at(&parent, parts[parts.len() - 1], false, true)?;
        for chunk in bytes.chunks(64 * 1024) {
            if cancel.load(Ordering::Acquire) {
                return Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"));
            }
            file.write_all(chunk)?;
        }
        file.sync_all()?;
        file.set_permissions(Permissions::from_mode(0o400))?;
        file.sync_all()
    }
    pub(super) fn seal(self) -> io::Result<()> {
        for directory in self.directories.iter().rev() {
            directory.set_permissions(Permissions::from_mode(0o500))?;
            directory.sync_all()?;
        }
        self.directory
            .file
            .set_permissions(Permissions::from_mode(0o500))?;
        self.directory.file.sync_all()
    }
}
