//! Read-only P00 coordinator. Validation does not apply patches or qualify providers.
use crate::{ContractError, ContractVersion, IntegrationManifest, IntegrationOrder, MergePlan, OwnershipRegistry,
            integration::{is_package_id, is_relative_path, registry_order}, integration_port::IntegrationPort,
            merge_manifests, validate_manifest};
use serde::{Serialize, de::DeserializeOwned};
use std::{collections::BTreeSet, fs::{self, File}, io::Read, path::{Path, PathBuf}};

const MAX_JSON_BYTES: u64 = 1024 * 1024;

pub struct IntegrationCoordinator;
impl IntegrationPort for IntegrationCoordinator {
    fn validate_manifest(&self, manifest: IntegrationManifest, contract: ContractVersion, ownership: &OwnershipRegistry) -> Result<IntegrationOrder, ContractError> {
        validate_manifest(manifest, contract, ownership)
    }
}

#[derive(Debug, Serialize)]
pub struct RequirementPrerequisite {
    pub consumer: String,
    pub requirement_id: String,
    pub package_id: String,
    /// Queue presence is not accepted provider evidence.
    pub provider_in_queue: bool,
}

#[derive(Debug, Serialize)]
pub struct QueueInspection {
    pub schema: u32,
    pub contract_version: ContractVersion,
    pub packages: Vec<String>,
    pub plan: MergePlan,
    pub unqualified_prerequisites: Vec<RequirementPrerequisite>,
    pub execution_state: String,
}

/// Inspect fixed registry/request locations of a caller-selected repository.
/// Filesystem checks are observations, not authorization to write these paths.
pub fn inspect_queue(root: &Path) -> Result<QueueInspection, ContractError> {
    let root = root.canonicalize().map_err(|_| ContractError::ForeignPath)?;
    if !root.is_dir() { return Err(ContractError::ForeignPath); }
    let ownership: OwnershipRegistry = read_json(&confined(&root, "integration/registry/owners.v1.json", false)?)?;
    registry_order(&ownership)?;
    let directory = confined(&root, "integration/requests", false)?;
    let mut requests = Vec::new();
    for entry in fs::read_dir(directory).map_err(|_| ContractError::InvalidManifest)? {
        let entry = entry.map_err(|_| ContractError::InvalidManifest)?;
        if entry.path().extension().and_then(|v| v.to_str()) == Some("json") { requests.push(entry.path()); }
        if requests.len() > 44 { return Err(ContractError::Quota); }
    }
    requests.sort();
    let mut manifests = Vec::new(); let mut packages = BTreeSet::new();
    for path in requests {
        let id = path.file_stem().and_then(|v| v.to_str()).ok_or(ContractError::InvalidManifest)?;
        if !is_package_id(id) { return Err(ContractError::InvalidManifest); }
        let manifest: IntegrationManifest = read_json(&confined(&root, &format!("integration/requests/{id}.json"), false)?)?;
        if manifest.package_id != id || !packages.insert(id.to_owned()) { return Err(ContractError::InvalidManifest); }
        IntegrationCoordinator.validate_manifest(manifest.clone(), ContractVersion::V1, &ownership)?;
        for source in &manifest.source_paths {
            if !confined(&root, source, false)?.is_file() { return Err(ContractError::ForeignPath); }
        }
        for hook in &manifest.hook_targets {
            let path = confined(&root, hook, true)?;
            if path.exists() && !path.is_file() { return Err(ContractError::ForeignPath); }
        }
        manifests.push(manifest);
    }
    let plan = if manifests.is_empty() {
        MergePlan { order: IntegrationOrder { requirement_ids: vec![] }, shared_hooks: vec![], source_paths: vec![] }
    } else { merge_manifests(&manifests, ContractVersion::V1, &ownership)? };
    let mut prerequisites = Vec::new();
    for consumer in &plan.order.requirement_ids {
        let requirement = ownership.requirements.iter().find(|r| &r.requirement_id == consumer).ok_or(ContractError::UnknownRequirement)?;
        for id in requirement.prerequisites.iter().collect::<BTreeSet<_>>() {
            let provider = ownership.requirements.iter().find(|r| &r.requirement_id == id).ok_or(ContractError::UnknownRequirement)?;
            prerequisites.push(RequirementPrerequisite { consumer: consumer.clone(), requirement_id: id.clone(),
                package_id: provider.package_id.clone(), provider_in_queue: packages.contains(&provider.package_id) });
        }
    }
    Ok(QueueInspection { schema: 1, contract_version: ContractVersion::V1, packages: packages.into_iter().collect(),
        plan, unqualified_prerequisites: prerequisites, execution_state: "not_applied".into() })
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T, ContractError> {
    if !path.is_file() { return Err(ContractError::InvalidManifest); }
    let mut bytes = Vec::new();
    File::open(path).map_err(|_| ContractError::InvalidManifest)?.take(MAX_JSON_BYTES + 1)
        .read_to_end(&mut bytes).map_err(|_| ContractError::InvalidManifest)?;
    if bytes.len() as u64 > MAX_JSON_BYTES { return Err(ContractError::Quota); }
    serde_json::from_slice(&bytes).map_err(|_| ContractError::InvalidManifest)
}

fn confined(root: &Path, relative: &str, allow_new_leaf: bool) -> Result<PathBuf, ContractError> {
    if !is_relative_path(relative, false) { return Err(ContractError::ForeignPath); }
    let parts: Vec<_> = relative.split('/').collect(); let mut path = root.to_owned();
    for (index, part) in parts.iter().enumerate() {
        path.push(part);
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() => return Err(ContractError::ForeignPath),
            Ok(metadata) if index + 1 < parts.len() && !metadata.is_dir() => return Err(ContractError::ForeignPath),
            Ok(_) => {},
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && allow_new_leaf && index + 1 == parts.len() => return Ok(path),
            Err(_) => return Err(ContractError::ForeignPath),
        }
    }
    if !path.canonicalize().map_err(|_| ContractError::ForeignPath)?.starts_with(root) { return Err(ContractError::ForeignPath); }
    Ok(path)
}
