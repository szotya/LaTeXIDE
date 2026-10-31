use latexide_contracts::{ContractError, ContractVersion, IntegrationManifest, OwnedPath, OwnedRequirement, OwnershipRegistry};
use latexide_contracts::coordinator::{IntegrationCoordinator, inspect_queue};
use latexide_contracts::integration_port::IntegrationPort;
use std::{fs, path::{Path, PathBuf}, process::Command, sync::atomic::{AtomicU64, Ordering}};

static NEXT: AtomicU64 = AtomicU64::new(1);
struct Project(PathBuf);
impl Project {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("latexide-queue-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        for folder in ["integration/registry", "integration/requests", "editor", "workspace"] { fs::create_dir_all(root.join(folder)).unwrap(); }
        for file in ["Cargo.toml", "editor/feature.rs", "workspace/feature.rs"] { fs::write(root.join(file), "fixture").unwrap(); }
        let value = Self(root); value.write("integration/registry/owners.v1.json", &registry()); value
    }
    fn write(&self, path: &str, value: &impl serde::Serialize) { fs::write(self.0.join(path), serde_json::to_vec_pretty(value).unwrap()).unwrap(); }
    fn manifest(&self, package: &str) { self.write(&format!("integration/requests/{package}.json"), &manifest(package)); }
}
impl Drop for Project { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }

fn registry() -> OwnershipRegistry {
    OwnershipRegistry {
        paths: vec![OwnedPath { package_id: "P00".into(), path: "Cargo.toml".into() },
                    OwnedPath { package_id: "P07".into(), path: "editor/".into() },
                    OwnedPath { package_id: "P08".into(), path: "workspace/".into() }],
        requirements: vec![OwnedRequirement { package_id: "P07".into(), requirement_id: "EDT-02".into(), prerequisites: vec!["UI-01".into()] },
                           OwnedRequirement { package_id: "P08".into(), requirement_id: "UI-01".into(), prerequisites: vec![] }],
    }
}
fn manifest(package: &str) -> IntegrationManifest {
    IntegrationManifest { schema: 1, package_id: package.into(), contract_version: 1,
        requirement_ids: vec![if package == "P07" { "EDT-02" } else { "UI-01" }.into()],
        source_paths: vec![if package == "P07" { "editor/feature.rs" } else { "workspace/feature.rs" }.into()],
        hook_targets: vec!["Cargo.toml".into()], test_commands: vec!["never executed".into()], compatibility: "fixture-v1".into() }
}

#[test]
fn real_integration_port_rejects_a_foreign_requirement() {
    let mut value = manifest("P07"); value.requirement_ids = vec!["UI-01".into()];
    assert_eq!(IntegrationCoordinator.validate_manifest(value, ContractVersion::V1, &registry()), Err(ContractError::ForeignRequirement));
}

#[test]
fn queue_merges_shared_hooks_in_requirement_order_without_accepting_providers() {
    let project = Project::new(); project.manifest("P08"); project.manifest("P07");
    let queue = inspect_queue(&project.0).unwrap();
    assert_eq!(queue.packages, vec!["P07", "P08"]);
    assert_eq!(queue.plan.order.requirement_ids, vec!["UI-01", "EDT-02"]);
    assert_eq!(queue.plan.shared_hooks[0].package_ids, vec!["P07", "P08"]);
    assert_eq!(queue.execution_state, "not_applied");
    assert_eq!(queue.unqualified_prerequisites[0].requirement_id, "UI-01");
    assert!(queue.unqualified_prerequisites[0].provider_in_queue);
    assert_eq!(serde_json::to_vec(&queue).unwrap(), serde_json::to_vec(&inspect_queue(&project.0).unwrap()).unwrap());
}

#[test]
fn queue_rejects_foreign_or_missing_declared_sources() {
    let project = Project::new(); let mut value = manifest("P07"); value.source_paths = vec!["workspace/feature.rs".into()];
    project.write("integration/requests/P07.json", &value);
    assert_eq!(inspect_queue(&project.0).unwrap_err(), ContractError::ForeignPath);
    project.manifest("P07"); fs::remove_file(project.0.join("editor/feature.rs")).unwrap();
    assert_eq!(inspect_queue(&project.0).unwrap_err(), ContractError::ForeignPath);
}

#[test]
fn queue_binds_file_name_to_the_declared_package_and_bounds_json() {
    let project = Project::new(); project.write("integration/requests/P08.json", &manifest("P07"));
    assert_eq!(inspect_queue(&project.0).unwrap_err(), ContractError::InvalidManifest);
    fs::remove_file(project.0.join("integration/requests/P08.json")).unwrap(); project.manifest("P07");
    fs::write(project.0.join("integration/requests/P07.json"), vec![b' '; 1024 * 1024 + 1]).unwrap();
    assert_eq!(inspect_queue(&project.0).unwrap_err(), ContractError::Quota);
}

#[cfg(unix)]
#[test]
fn a_symlink_cannot_supply_a_declared_source_even_if_it_stays_inside_root() {
    let project = Project::new(); project.manifest("P07");
    fs::remove_file(project.0.join("editor/feature.rs")).unwrap();
    std::os::unix::fs::symlink(project.0.join("workspace/feature.rs"), project.0.join("editor/feature.rs")).unwrap();
    assert_eq!(inspect_queue(&project.0).unwrap_err(), ContractError::ForeignPath);
}

#[test]
fn cli_never_executes_manifest_commands_or_writes_hook_files() {
    let project = Project::new(); let mut value = manifest("P07");
    let sentinel = project.0.join("must-not-exist"); value.test_commands = vec![format!("touch {}", sentinel.display())];
    project.write("integration/requests/P07.json", &value);
    let before = fs::read(project.0.join("Cargo.toml")).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_latexide-integration")).args(["--root"]).arg(&project.0).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["execution_state"], "not_applied");
    assert!(!sentinel.exists()); assert_eq!(fs::read(project.0.join("Cargo.toml")).unwrap(), before);
}

#[test]
fn canonical_queue_keeps_mcp03_and_own_acceptance_dependencies_unqualified() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let queue = inspect_queue(&root).unwrap();
    assert!(queue.packages.contains(&"P00".into()));
    let dependency = queue.unqualified_prerequisites.iter().find(|p| p.consumer == "QA-08" && p.requirement_id == "MCP-03").unwrap();
    assert_eq!(dependency.package_id, "P03");
    assert_eq!(queue.execution_state, "not_applied");
}
