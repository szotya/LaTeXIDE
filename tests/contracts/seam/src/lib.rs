//! Executable example of compiling an owned module without shared lib.rs edits.

#[path = "../../seam_module.rs"]
pub mod owned_module;

#[cfg(test)]
mod tests {
    use super::owned_module::inspect_snapshot;
    use latexide_contracts::{SnapshotLimits, WorkspaceSnapshot};

    #[test]
    fn a_worker_can_test_its_own_path_module_against_core_and_contracts() {
        let snapshot = WorkspaceSnapshot { id: "seam-fixture".into(), epoch: 1, files: vec![] };
        let limits = SnapshotLimits { max_files: 10, max_file_bytes: 1024 * 1024, max_total_bytes: 1024 * 1024 };
        assert_eq!(inspect_snapshot(&snapshot, &limits).unwrap(), (0, 1024 * 1024));
    }
}
