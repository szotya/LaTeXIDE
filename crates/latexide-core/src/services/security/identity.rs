//! File-identity witnesses, not an atomic filesystem mutation API.
use latexide_contracts::security::SecurityError;
use std::{
    fs::{self, File, Metadata},
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, PartialEq, Eq)]
struct Identity {
    device: u64,
    inode: u64,
}

fn identity(metadata: &Metadata) -> Result<Identity, SecurityError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.is_symlink()
            || !(metadata.is_dir() || metadata.is_file())
            || (metadata.is_file() && metadata.nlink() != 1)
        {
            return Err(SecurityError::IdentityChanged);
        }
        Ok(Identity {
            device: metadata.dev(),
            inode: metadata.ino(),
        })
    }
    // No path/mtime/length fallback on platforms lacking a qualified identity adapter.
    #[cfg(not(unix))]
    {
        let _ = metadata;
        Err(SecurityError::IdentityChanged)
    }
}

fn open_witness(path: &Path) -> Result<File, SecurityError> {
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        use std::fs::OpenOptions;
        use std::os::unix::fs::OpenOptionsExt;
        // Darwin SDK sys/fcntl.h: O_NOFOLLOW_ANY | O_NONBLOCK. An unsupported
        // kernel flag is an error, never a retry with weaker flags.
        #[cfg(target_os = "macos")]
        const FLAGS: i32 = 0x2000_0000 | 0x0000_0004;
        // Linux architecture-specific O_NOFOLLOW | O_NONBLOCK from libc.
        // Ancestors are individually witnessed;
        // this does not claim Linux openat2/RESOLVE_BENEATH isolation.
        #[cfg(target_os = "linux")]
        const FLAGS: i32 = libc::O_NOFOLLOW | libc::O_NONBLOCK;
        OpenOptions::new()
            .read(true)
            .custom_flags(FLAGS)
            .open(path)
            .map_err(|_| SecurityError::IdentityChanged)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = path;
        Err(SecurityError::IdentityChanged)
    }
}

/// Holding the file prevents an unlinked inode from being silently reused while
/// the witness is alive. No contents, writes, or raw handles are exposed.
pub(super) struct PinnedPath {
    path: PathBuf,
    file: File,
    identity: Identity,
}

impl PinnedPath {
    pub(super) fn new(path: &Path) -> Result<Self, SecurityError> {
        let observed = fs::symlink_metadata(path).map_err(|_| SecurityError::IdentityChanged)?;
        let expected = identity(&observed)?;
        let file = open_witness(path)?;
        if identity(
            &file
                .metadata()
                .map_err(|_| SecurityError::IdentityChanged)?,
        )? != expected
        {
            return Err(SecurityError::IdentityChanged);
        }
        let witness = Self {
            path: path.into(),
            file,
            identity: expected,
        };
        witness.revalidate()?;
        Ok(witness)
    }

    pub(super) fn revalidate(&self) -> Result<(), SecurityError> {
        let held = self
            .file
            .metadata()
            .map_err(|_| SecurityError::IdentityChanged)?;
        let current =
            fs::symlink_metadata(&self.path).map_err(|_| SecurityError::IdentityChanged)?;
        if identity(&held)? != self.identity || identity(&current)? != self.identity {
            return Err(SecurityError::IdentityChanged);
        }
        Ok(())
    }
}
