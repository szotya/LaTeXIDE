//! Executable example of compiling an owned module without shared lib.rs edits.

#[path = "../../seam_module.rs"]
pub mod owned_module;

#[cfg(test)]
mod tests {
    use super::owned_module::inspect_snapshot;
    use latexide_contracts::{RevisionedEdit, SnapshotFile, SnapshotLimits, WorkspaceSnapshot, validate_edits};
    use latexide_core::{DocumentSnapshot, Position, TextEdit, Workspace};
    use std::{fs, path::PathBuf, sync::atomic::{AtomicU64, Ordering}};

    static NEXT: AtomicU64 = AtomicU64::new(1);
    struct Project(PathBuf);
    impl Project {
        fn new(text: &str) -> Self {
            let root = std::env::temp_dir().join(format!("latexide-contract-feff-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
            fs::create_dir_all(&root).unwrap(); fs::write(root.join("main.tex"), text.as_bytes()).unwrap(); Self(root)
        }
    }
    impl Drop for Project { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }
    fn contract(doc: &DocumentSnapshot) -> WorkspaceSnapshot {
        WorkspaceSnapshot { id: "real-core".into(), epoch: 1, files: vec![SnapshotFile {
            path: doc.id.clone(), revision: Some(doc.revision), content_sha256: "a".repeat(64),
            utf8: doc.text.as_bytes().to_vec(), dirty: doc.dirty,
        }] }
    }
    fn limits() -> SnapshotLimits { SnapshotLimits { max_files: 10, max_file_bytes: 1024 * 1024, max_total_bytes: 1024 * 1024 } }

    #[test]
    fn logical_feff_inserted_by_the_real_core_remains_valid_contract_content() {
        let project = Project::new("text"); let mut core = Workspace::open(project.0.clone()).unwrap();
        let before = core.open_document("main.tex").unwrap();
        let edit = RevisionedEdit { document_id: before.id.clone(), expected_revision: before.revision,
            start_utf16: 0, length_utf16: 0, replacement: "\u{feff}".into() };
        validate_edits(&contract(&before), &[edit]).unwrap();
        core.apply_edits(&before.id, before.revision, vec![TextEdit {
            start: Position { line: 0, character: 0 }, end: Position { line: 0, character: 0 }, text: "\u{feff}".into(),
        }], "agent").unwrap();
        let after = core.snapshot(&before.id).unwrap();
        assert_eq!(after.text, "\u{feff}text"); assert!(!after.bom);
        contract(&after).validate(&limits()).unwrap();
    }

    #[test]
    fn physical_bom_and_logical_feff_are_separate_when_loading_core_content() {
        let project = Project::new("\u{feff}\u{feff}text"); let mut core = Workspace::open(project.0.clone()).unwrap();
        let doc = core.open_document("main.tex").unwrap();
        assert!(doc.bom); assert_eq!(doc.text, "\u{feff}text");
        contract(&doc).validate(&limits()).unwrap();
    }

    #[test]
    fn a_worker_can_test_its_own_path_module_against_core_and_contracts() {
        let snapshot = WorkspaceSnapshot { id: "seam-fixture".into(), epoch: 1, files: vec![] };
        let limits = SnapshotLimits { max_files: 10, max_file_bytes: 1024 * 1024, max_total_bytes: 1024 * 1024 };
        assert_eq!(inspect_snapshot(&snapshot, &limits).unwrap(), (0, 1024 * 1024));
    }
}
