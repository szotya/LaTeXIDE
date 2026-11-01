//! P18's compiler-resource policy. This module is not registered in the runtime yet.
//!
//! Construct one provider from host-owned policy per project. A v1 path grant is a
//! policy decision, not an open file, an OS sandbox, or permission to launch code.
//! The compiler must independently install its OS isolation before execution;
//! missing/failed isolation must never fall back to the existing unrestricted spawn.

pub mod execution;
pub mod generation;
mod identity;

use identity::PinnedPath;
use latexide_contracts::security::{
    ExecutionPolicy, ResourceGrant, ResourceRequest, SecurityError, SecurityPort,
};
use std::{
    fs,
    path::{Component, Path, PathBuf},
};

struct Root {
    configured: PathBuf,
    canonical: PathBuf,
    identity: PinnedPath,
}

impl Root {
    fn new(value: &str) -> Result<Self, SecurityError> {
        let configured = checked_path(value)?;
        if !configured.is_absolute() {
            return Err(SecurityError::OutsideRoot);
        }
        let canonical = configured
            .canonicalize()
            .map_err(|_| SecurityError::IdentityChanged)?;
        if !fs::metadata(&canonical).is_ok_and(|metadata| metadata.is_dir()) {
            return Err(SecurityError::IdentityChanged);
        }
        let identity = PinnedPath::new(&canonical)?;
        Ok(Self {
            configured: configured.into(),
            canonical,
            identity,
        })
    }

    fn revalidate(&self) -> Result<(), SecurityError> {
        if self
            .configured
            .canonicalize()
            .map_err(|_| SecurityError::IdentityChanged)?
            != self.canonical
            || !fs::metadata(&self.canonical).is_ok_and(|metadata| metadata.is_dir())
        {
            return Err(SecurityError::IdentityChanged);
        }
        self.identity.revalidate()
    }
}

/// Policy is bound at construction; the trait argument cannot escalate it later.
/// No file contents are read, outputs created, processes started, or sockets opened.
pub struct ProjectSecurity {
    policy: ExecutionPolicy,
    project: Root,
    writable: Vec<Root>,
}

/// A local identity witness, not a serializable grant or a write/execute handle.
/// Revalidation detects observed replacements, but another path open after it
/// still races. A final writer needs handle-relative I/O and OS confinement.
pub struct CheckedResource<'a> {
    provider: &'a ProjectSecurity,
    canonical_path: String,
    target: PathBuf,
    identities: Vec<PinnedPath>,
    absent_leaf: Option<PathBuf>,
}

impl CheckedResource<'_> {
    pub fn revalidate(&self) -> Result<ResourceGrant, SecurityError> {
        self.provider.revalidate_roots()?;
        for identity in &self.identities {
            identity.revalidate()?;
        }
        if let Some(path) = &self.absent_leaf {
            match fs::symlink_metadata(path) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
                _ => return Err(SecurityError::IdentityChanged),
            }
        }
        if canonical_target(&self.target, self.absent_leaf.is_some())?.to_str()
            != Some(&self.canonical_path)
        {
            return Err(SecurityError::IdentityChanged);
        }
        Ok(ResourceGrant {
            canonical_path: self.canonical_path.clone(),
        })
    }
}

impl ProjectSecurity {
    pub fn new(policy: &ExecutionPolicy) -> Result<Self, SecurityError> {
        if !policy.trusted {
            return Err(SecurityError::Untrusted);
        }
        let project = Root::new(&policy.project_root)?;
        let writable = policy
            .writable_roots
            .iter()
            .map(|path| Root::new(path))
            .collect::<Result<_, _>>()?;
        Ok(Self {
            policy: policy.clone(),
            project,
            writable,
        })
    }

    fn revalidate_roots(&self) -> Result<(), SecurityError> {
        self.project.revalidate()?;
        for root in &self.writable {
            root.revalidate()?;
        }
        Ok(())
    }

    pub fn check_resource(
        &self,
        policy: &ExecutionPolicy,
        request: ResourceRequest,
    ) -> Result<CheckedResource<'_>, SecurityError> {
        if !policy.trusted || policy != &self.policy {
            return Err(SecurityError::Untrusted);
        }
        if request.network && !self.policy.network {
            return Err(SecurityError::NetworkDenied);
        }
        let path = checked_path(&request.path)?;
        let candidate = if path.is_absolute() {
            path.to_owned()
        } else {
            self.project.canonical.join(path)
        };
        self.revalidate_roots()?;
        let roots = self
            .writable
            .iter()
            .chain((!request.write).then_some(&self.project));
        // Resolve only each proposed root prefix through the OS, preserving the
        // requested spelling for the no-symlink walk. Case/NFC/NFD equivalence is
        // a filesystem observation, never lowercasing or string normalization.
        let (root, requested_root) = roots
            .filter_map(|root| {
                let count = root.canonical.components().count();
                if candidate.components().count() < count {
                    return None;
                }
                let prefix: PathBuf = candidate.components().take(count).collect();
                (prefix.canonicalize().ok()?.as_path() == root.canonical.as_path())
                    .then_some((root, prefix))
            })
            .max_by_key(|(root, _)| root.canonical.components().count())
            .ok_or(SecurityError::OutsideRoot)?;
        directory_route_without_symlinks(&requested_root)?;
        let relative = candidate
            .strip_prefix(&requested_root)
            .map_err(|_| SecurityError::OutsideRoot)?;
        let components: Vec<_> = relative
            .components()
            .filter(|component| !matches!(component, Component::CurDir))
            .collect();
        let mut current = requested_root;
        let mut identities = Vec::with_capacity(components.len() + 1);
        identities.push(PinnedPath::new(&current)?);
        let mut absent_leaf = None;
        for (index, component) in components.iter().enumerate() {
            current.push(component.as_os_str());
            let final_component = index + 1 == components.len();
            match fs::symlink_metadata(&current) {
                Ok(metadata) if metadata.is_symlink() => {
                    return Err(SecurityError::IdentityChanged);
                }
                Ok(metadata) if metadata.is_dir() || (final_component && metadata.is_file()) => {
                    identities.push(PinnedPath::new(&current)?);
                }
                Ok(_) => return Err(SecurityError::IdentityChanged),
                Err(error)
                    if error.kind() == std::io::ErrorKind::NotFound
                        && final_component
                        && request.write =>
                {
                    absent_leaf = Some(current.clone());
                }
                Err(_) => return Err(SecurityError::IdentityChanged),
            }
        }
        let canonical = canonical_target(&current, absent_leaf.is_some())?;
        if !canonical.starts_with(&root.canonical) {
            return Err(SecurityError::IdentityChanged);
        }
        let canonical_path = canonical
            .to_str()
            .ok_or(SecurityError::IdentityChanged)?
            .into();
        let checked = CheckedResource {
            provider: self,
            canonical_path,
            target: current,
            identities,
            absent_leaf,
        };
        checked.revalidate()?;
        Ok(checked)
    }
}

fn directory_route_without_symlinks(path: &Path) -> Result<(), SecurityError> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        let metadata =
            fs::symlink_metadata(&current).map_err(|_| SecurityError::IdentityChanged)?;
        if metadata.is_symlink() || !metadata.is_dir() {
            return Err(SecurityError::IdentityChanged);
        }
    }
    Ok(())
}

impl SecurityPort for ProjectSecurity {
    fn authorize_resource(
        &self,
        policy: &ExecutionPolicy,
        request: ResourceRequest,
    ) -> Result<ResourceGrant, SecurityError> {
        self.check_resource(policy, request)?.revalidate()
    }
}

/// Convenience for a single decision. Retain `ProjectSecurity` for project-lifetime
/// checks; reconstructing it establishes a new policy/root baseline every time.
pub fn authorize_resource(
    policy: &ExecutionPolicy,
    request: ResourceRequest,
) -> Result<ResourceGrant, SecurityError> {
    ProjectSecurity::new(policy)?.authorize_resource(policy, request)
}

fn canonical_target(path: &Path, absent_leaf: bool) -> Result<PathBuf, SecurityError> {
    if absent_leaf {
        let parent = path.parent().ok_or(SecurityError::IdentityChanged)?;
        let name = path.file_name().ok_or(SecurityError::IdentityChanged)?;
        Ok(parent
            .canonicalize()
            .map_err(|_| SecurityError::IdentityChanged)?
            .join(name))
    } else {
        path.canonicalize()
            .map_err(|_| SecurityError::IdentityChanged)
    }
}

fn checked_path(value: &str) -> Result<&Path, SecurityError> {
    // Reject foreign-platform paths/URLs rather than accidentally treating them as
    // relative POSIX filenames. Spaces and non-ASCII names remain ordinary data.
    if value.is_empty()
        || value
            .chars()
            .any(|c| c.is_ascii_control() || matches!(c, '\\' | ':'))
    {
        return Err(SecurityError::OutsideRoot);
    }
    let path = Path::new(value);
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
    {
        return Err(SecurityError::OutsideRoot);
    }
    Ok(path)
}
