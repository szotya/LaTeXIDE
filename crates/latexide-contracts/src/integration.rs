use crate::{ContractError, ContractVersion};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntegrationManifest {
    pub schema: u32,
    pub package_id: String,
    pub contract_version: u32,
    pub requirement_ids: Vec<String>,
    pub source_paths: Vec<String>,
    pub hook_targets: Vec<String>,
    /// Descriptive data only; never passed to a shell by this crate.
    pub test_commands: Vec<String>,
    pub compatibility: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnedPath { pub package_id: String, pub path: String }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnedRequirement {
    pub package_id: String,
    pub requirement_id: String,
    #[serde(default)]
    pub prerequisites: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnershipRegistry { pub paths: Vec<OwnedPath>, pub requirements: Vec<OwnedRequirement> }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntegrationOrder { pub requirement_ids: Vec<String> }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SharedHook { pub path: String, pub package_ids: Vec<String> }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MergePlan {
    pub order: IntegrationOrder,
    pub shared_hooks: Vec<SharedHook>,
    pub source_paths: Vec<String>,
}

pub fn validate_manifest(
    manifest: IntegrationManifest, contract: ContractVersion, ownership: &OwnershipRegistry,
) -> Result<IntegrationOrder, ContractError> {
    if !contract.supported() || manifest.contract_version != contract.major {
        return Err(ContractError::IncompatibleVersion);
    }
    let ordered = registry_order(ownership)?;
    if manifest.schema != 1 || !is_package_id(&manifest.package_id)
        || manifest.requirement_ids.is_empty() || manifest.requirement_ids.len() > 4096
        || manifest.source_paths.is_empty() || manifest.source_paths.len() > 16384
        || manifest.hook_targets.len() > 16384 || manifest.test_commands.is_empty()
        || manifest.test_commands.len() > 256
        || manifest.test_commands.iter().any(|s| s.trim().is_empty() || s.len() > 16384)
        || manifest.compatibility.trim().is_empty() || manifest.compatibility.len() > 8192
    {
        return Err(ContractError::InvalidManifest);
    }
    for id in &manifest.requirement_ids {
        let owned = ownership.requirements.iter().find(|r| &r.requirement_id == id)
            .ok_or(ContractError::UnknownRequirement)?;
        if owned.package_id != manifest.package_id { return Err(ContractError::ForeignRequirement); }
    }
    for path in &manifest.source_paths {
        if !is_relative_path(path, false) || !ownership.paths.iter().any(|owned|
            owned.package_id == manifest.package_id && covers(&owned.path, path))
        {
            return Err(ContractError::ForeignPath);
        }
    }
    for path in &manifest.hook_targets {
        if !is_relative_path(path, false) || !ownership.paths.iter().any(|owned|
            owned.package_id == "P00" && covers(&owned.path, path))
        {
            return Err(ContractError::ForeignPath);
        }
    }
    let requested: BTreeSet<_> = manifest.requirement_ids.into_iter().collect();
    Ok(IntegrationOrder { requirement_ids: ordered.into_iter().filter(|id| requested.contains(id)).collect() })
}

pub fn merge_manifests(
    manifests: &[IntegrationManifest], contract: ContractVersion, ownership: &OwnershipRegistry,
) -> Result<MergePlan, ContractError> {
    if manifests.is_empty() || manifests.len() > 256 { return Err(ContractError::InvalidManifest); }
    let mut requirements = BTreeSet::new();
    let mut sources = BTreeSet::new();
    let mut hooks: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for manifest in manifests {
        let order = validate_manifest(manifest.clone(), contract, ownership)?;
        requirements.extend(order.requirement_ids);
        sources.extend(manifest.source_paths.iter().cloned());
        for path in &manifest.hook_targets {
            hooks.entry(path.clone()).or_default().insert(manifest.package_id.clone());
        }
    }
    Ok(MergePlan {
        order: IntegrationOrder { requirement_ids: registry_order(ownership)?.into_iter()
            .filter(|id| requirements.contains(id)).collect() },
        shared_hooks: hooks.into_iter().map(|(path, packages)| SharedHook {
            path, package_ids: packages.into_iter().collect(),
        }).collect(),
        source_paths: sources.into_iter().collect(),
    })
}

pub fn is_package_id(id: &str) -> bool {
    let b = id.as_bytes();
    if b.len() != 3 || !b[1].is_ascii_digit() || !b[2].is_ascii_digit() { return false; }
    let n = (b[1] - b'0') * 10 + b[2] - b'0';
    matches!((b[0], n), (b'P', 0..=23) | (b'U', 1..=20))
}

/// Lexical, portable repository paths. This is not filesystem authorization:
/// an integrator must separately resolve symlinks and validate file identities.
pub fn is_relative_path(path: &str, allow_directory_prefix: bool) -> bool {
    if path.is_empty() || path.len() > 4096 || path.starts_with('/')
        || path.chars().any(|c| c.is_control() || matches!(c, '\\' | ':' | '*' | '?' | '{' | '}' | '<' | '>' | '|' | '"'))
    { return false; }
    let value = if allow_directory_prefix { path.strip_suffix('/').unwrap_or(path) } else { path };
    value.split('/').all(|part| !part.is_empty() && part != "." && part != ".."
        && !part.ends_with(' ') && !part.ends_with('.'))
}

fn covers(pattern: &str, path: &str) -> bool {
    if pattern.ends_with('/') { path.starts_with(pattern) } else { pattern == path }
}

fn registry_order(ownership: &OwnershipRegistry) -> Result<Vec<String>, ContractError> {
    if ownership.paths.is_empty() || ownership.paths.len() > 16384
        || ownership.requirements.is_empty() || ownership.requirements.len() > 4096
    { return Err(ContractError::InvalidRegistry); }
    for (index, left) in ownership.paths.iter().enumerate() {
        if !is_package_id(&left.package_id) || !is_relative_path(&left.path, true) {
            return Err(ContractError::InvalidRegistry);
        }
        for right in &ownership.paths[index + 1..] {
            if left.package_id == right.package_id { continue; }
            let a = left.path.to_ascii_lowercase(); let b = right.path.to_ascii_lowercase();
            if a.trim_end_matches('/') == b.trim_end_matches('/') || covers(&a, &b) || covers(&b, &a) {
                return Err(ContractError::InvalidRegistry);
            }
        }
    }
    let mut pending: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for requirement in &ownership.requirements {
        if !is_package_id(&requirement.package_id) || requirement.requirement_id.is_empty()
            || requirement.requirement_id.len() > 64
            || !requirement.requirement_id.bytes().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'-')
            || requirement.prerequisites.len() > 4096
            || pending.insert(requirement.requirement_id.clone(), requirement.prerequisites.iter().cloned().collect()).is_some()
        { return Err(ContractError::InvalidRegistry); }
    }
    if pending.values().any(|deps| deps.iter().any(|id| !pending.contains_key(id))) {
        return Err(ContractError::UnknownRequirement);
    }
    let mut ordered = Vec::new();
    while !pending.is_empty() {
        let next = pending.iter().find(|(_, deps)| deps.is_empty()).map(|(id, _)| id.clone())
            .ok_or(ContractError::CyclicRequirement)?;
        pending.remove(&next);
        for deps in pending.values_mut() { deps.remove(&next); }
        ordered.push(next);
    }
    Ok(ordered)
}
