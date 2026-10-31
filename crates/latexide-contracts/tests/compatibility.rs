use latexide_contracts::{
    ContractError, ContractVersion, IntegrationManifest, OwnedPath, OwnedRequirement,
    OwnershipRegistry, merge_manifests, validate_manifest,
};

fn registry() -> OwnershipRegistry {
    OwnershipRegistry {
        paths: vec![
            OwnedPath { package_id: "P00".into(), path: "Cargo.toml".into() },
            OwnedPath { package_id: "P07".into(), path: "apps/macos/Sources/LaTeXIDEKit/Features/Editor/".into() },
        ],
        requirements: vec![OwnedRequirement {
            package_id: "P07".into(), requirement_id: "EDT-02".into(), prerequisites: vec![],
        }],
    }
}

fn manifest() -> IntegrationManifest {
    IntegrationManifest {
        schema: 1, package_id: "P07".into(), contract_version: 1,
        requirement_ids: vec!["EDT-02".into()],
        source_paths: vec!["apps/macos/Sources/LaTeXIDEKit/Features/Editor/Wrap.swift".into()],
        hook_targets: vec!["Cargo.toml".into()],
        test_commands: vec!["swift test --filter EditorFeatureTests".into()],
        compatibility: "contract-v1; independent editor slice".into(),
    }
}

#[test]
fn contract_v1_first_slice() {
    let good = manifest();
    let json = serde_json::to_string(&good).unwrap();
    let decoded: IntegrationManifest = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, good);
    assert_eq!(validate_manifest(decoded, ContractVersion::V1, &registry()).unwrap().requirement_ids, vec!["EDT-02"]);
    let mut foreign = good;
    foreign.source_paths = vec!["apps/macos/Sources/LaTeXIDEKit/Assistant/AssistantSession.swift".into()];
    assert_eq!(validate_manifest(foreign, ContractVersion::V1, &registry()), Err(ContractError::ForeignPath));
}

#[test]
fn review_focus_r1_rejects_incompatible_manifest_major() {
    let mut request = manifest(); request.contract_version = 2;
    assert_eq!(validate_manifest(request, ContractVersion::V1, &registry()), Err(ContractError::IncompatibleVersion));
}

#[test]
fn manifest_paths_are_literal_relative_files_with_segment_boundaries() {
    for path in ["../Cargo.toml", "/etc/passwd", "C:/secret.txt", "apps\\evil.swift", "apps/macos/../secret", "apps//secret", "apps/./secret", "apps/macos/Sources/LaTeXIDEKit/Features/EditorEscape/F.swift"] {
        let mut request = manifest(); request.source_paths = vec![path.into()];
        assert_eq!(validate_manifest(request, ContractVersion::V1, &registry()), Err(ContractError::ForeignPath), "{path}");
    }
}

#[test]
fn a_known_requirement_cannot_be_claimed_by_another_package() {
    let mut request = manifest(); request.package_id = "P08".into();
    assert_eq!(validate_manifest(request, ContractVersion::V1, &registry()), Err(ContractError::ForeignRequirement));
    let mut request = manifest(); request.requirement_ids = vec!["UNKNOWN-99".into()];
    assert_eq!(validate_manifest(request, ContractVersion::V1, &registry()), Err(ContractError::UnknownRequirement));
}

#[test]
fn hooks_must_be_integrator_owned_and_commands_remain_inert_data() {
    let mut request = manifest(); request.hook_targets = vec!["secrets/token".into()];
    assert_eq!(validate_manifest(request, ContractVersion::V1, &registry()), Err(ContractError::ForeignPath));
    let mut request = manifest(); request.test_commands = vec!["not a shell invocation; $(never-run)".into()];
    assert!(validate_manifest(request, ContractVersion::V1, &registry()).is_ok());
}

#[test]
fn cycles_and_unknown_prerequisites_are_rejected_before_ordering() {
    let mut owners = registry(); owners.requirements[0].prerequisites = vec!["EDT-02".into()];
    assert_eq!(validate_manifest(manifest(), ContractVersion::V1, &owners), Err(ContractError::CyclicRequirement));
    owners.requirements[0].prerequisites = vec!["UNKNOWN-99".into()];
    assert_eq!(validate_manifest(manifest(), ContractVersion::V1, &owners), Err(ContractError::UnknownRequirement));
}

#[test]
fn review_focus_r2_shared_hooks_merge_deterministically_without_applying_patches() {
    let mut owners = registry();
    owners.paths.push(OwnedPath { package_id: "P08".into(), path: "crates/latexide-core/src/services/workspace_settings/".into() });
    owners.requirements.push(OwnedRequirement { package_id: "P08".into(), requirement_id: "WS-06".into(), prerequisites: vec!["EDT-02".into()] });
    let first = manifest();
    let mut second = manifest(); second.package_id = "P08".into(); second.requirement_ids = vec!["WS-06".into()];
    second.source_paths = vec!["crates/latexide-core/src/services/workspace_settings/mod.rs".into()];
    let forward = merge_manifests(&[first.clone(), second.clone()], ContractVersion::V1, &owners).unwrap();
    let backward = merge_manifests(&[second, first], ContractVersion::V1, &owners).unwrap();
    assert_eq!(forward, backward);
    assert_eq!(forward.order.requirement_ids, vec!["EDT-02", "WS-06"]);
    assert_eq!(forward.shared_hooks[0].path, "Cargo.toml");
    assert_eq!(forward.shared_hooks[0].package_ids, vec!["P07", "P08"]);
}

#[test]
fn ambiguous_registry_ownership_is_not_resolved_by_input_order() {
    let mut owners = registry(); owners.paths.push(OwnedPath { package_id: "P08".into(), path: "apps/macos/Sources/LaTeXIDEKit/Features/Editor/".into() });
    assert_eq!(validate_manifest(manifest(), ContractVersion::V1, &owners), Err(ContractError::InvalidRegistry));
}

#[test]
fn canonical_registry_validates_the_real_p00_manifest_and_all_226_requirement_edges() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let owners: OwnershipRegistry = serde_json::from_slice(&std::fs::read(root.join("integration/registry/owners.v1.json")).unwrap()).unwrap();
    let request: IntegrationManifest = serde_json::from_slice(&std::fs::read(root.join("integration/requests/P00.json")).unwrap()).unwrap();
    assert_eq!(owners.requirements.len(), 226);
    assert_eq!(validate_manifest(request, ContractVersion::V1, &owners).unwrap().requirement_ids, vec!["QA-08", "QA-14", "OS-10"]);
}
