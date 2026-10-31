use latexide_core::{Position, TextEdit, Workspace};
use std::{fs, path::PathBuf, sync::atomic::{AtomicU64, Ordering}};

static NEXT: AtomicU64 = AtomicU64::new(1);
struct Project(PathBuf);
impl Project {
    fn new(bytes: &[u8]) -> Self {
        let base = std::env::temp_dir().join(format!("latexide-core-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir_all(base.join("project")).unwrap();
        fs::write(base.join("project/main.tex"), bytes).unwrap();
        Self(base)
    }
    fn root(&self) -> PathBuf { self.0.join("project") }
    fn workspace(&self) -> Workspace { Workspace::open(self.root()).unwrap() }
}
impl Drop for Project { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }

fn edit(line: u32, start: u32, end: u32, text: &str) -> TextEdit {
    TextEdit { start: Position { line, character: start }, end: Position { line, character: end }, text: text.into() }
}

#[test]
fn utf16_replacement_preserves_unicode_and_other_lines() {
    let project = Project::new("ab😀cd\nmásodik".as_bytes());
    let mut workspace = project.workspace();
    let doc = workspace.open_document("main.tex").unwrap();
    let changed = workspace.apply_edits(&doc.id, doc.revision, vec![edit(0, 2, 4, "X")], "agent").unwrap();
    assert_eq!(workspace.snapshot(&changed.id).unwrap().text, "abXcd\nmásodik");
    assert!(changed.dirty);
    assert_eq!(workspace.undo().unwrap().text, "ab😀cd\nmásodik");
}

#[test]
fn edit_cannot_split_a_utf16_surrogate_pair() {
    let project = Project::new("a😀b".as_bytes());
    let mut workspace = project.workspace();
    let doc = workspace.open_document("main.tex").unwrap();
    let error = workspace.apply_edits(&doc.id, doc.revision, vec![edit(0, 2, 3, "x")], "agent").unwrap_err();
    assert_eq!(error.code, "invalid_position");
    assert_eq!(workspace.snapshot(&doc.id).unwrap().text, "a😀b");
}

#[test]
fn stale_agent_edit_does_not_overwrite_new_user_input() {
    let project = Project::new(b"a");
    let mut workspace = project.workspace();
    let original = workspace.open_document("main.tex").unwrap();
    workspace.apply_edits(&original.id, original.revision, vec![edit(0, 1, 1, " user")], "user").unwrap();
    let error = workspace.apply_edits(&original.id, original.revision, vec![edit(0, 0, 1, "agent")], "agent").unwrap_err();
    assert_eq!(error.code, "revision_conflict");
    assert_eq!(workspace.snapshot(&original.id).unwrap().text, "a user");
}

#[test]
fn user_and_agent_changes_undo_in_order_and_redo() {
    let project = Project::new(b"a");
    let mut workspace = project.workspace();
    let original = workspace.open_document("main.tex").unwrap();
    let agent = workspace.apply_edits(&original.id, original.revision, vec![edit(0, 1, 1, "A")], "agent").unwrap();
    workspace.apply_edits(&agent.id, agent.revision, vec![edit(0, 2, 2, "U")], "user").unwrap();
    assert_eq!(workspace.undo().unwrap().text, "aA");
    assert_eq!(workspace.undo().unwrap().text, "a");
    assert_eq!(workspace.redo().unwrap().text, "aA");
    assert_eq!(workspace.redo().unwrap().text, "aAU");
}

#[test]
fn saved_agent_edit_is_still_undoable_and_resavable() {
    let project = Project::new(b"original");
    let mut workspace = project.workspace();
    let original = workspace.open_document("main.tex").unwrap();
    let agent = workspace.apply_edits(&original.id, original.revision, vec![edit(0, 0, 8, "agent")], "agent").unwrap();
    workspace.save_document(&agent.id, agent.revision).unwrap();
    assert_eq!(fs::read(project.root().join("main.tex")).unwrap(), b"agent");
    let undone = workspace.undo().unwrap();
    assert_eq!(undone.text, "original");
    assert!(undone.dirty);
    workspace.save_document(&undone.id, undone.revision).unwrap();
    assert_eq!(fs::read(project.root().join("main.tex")).unwrap(), b"original");
}

#[test]
fn save_preserves_utf8_bom_and_crlf() {
    let project = Project::new("\u{feff}Első\r\nMásodik\r\n".as_bytes());
    let mut workspace = project.workspace();
    let original = workspace.open_document("main.tex").unwrap();
    assert_eq!(original.text, "Első\nMásodik\n");
    let changed = workspace.apply_edits(&original.id, original.revision, vec![edit(0, 0, 4, "Új")], "agent").unwrap();
    workspace.save_document(&changed.id, changed.revision).unwrap();
    assert_eq!(fs::read(project.root().join("main.tex")).unwrap(), "\u{feff}Új\r\nMásodik\r\n".as_bytes());
}

#[test]
fn a_batch_is_reversible_even_when_replacements_change_lengths() {
    let project = Project::new(b"abcdef");
    let mut workspace = project.workspace();
    let original = workspace.open_document("main.tex").unwrap();
    let changed = workspace.apply_edits(&original.id, original.revision,
        vec![edit(0, 1, 2, "XX"), edit(0, 4, 5, "YY")], "agent").unwrap();
    assert_eq!(workspace.snapshot(&changed.id).unwrap().text, "aXXcdYYf");
    assert_eq!(workspace.undo().unwrap().text, "abcdef");
    assert_eq!(workspace.redo().unwrap().text, "aXXcdYYf");
}

#[test]
fn overlapping_batch_is_rejected_without_partial_change() {
    let project = Project::new(b"abcdef");
    let mut workspace = project.workspace();
    let original = workspace.open_document("main.tex").unwrap();
    assert!(workspace.apply_edits(&original.id, original.revision,
        vec![edit(0, 1, 3, "X"), edit(0, 2, 4, "Y")], "agent").is_err());
    assert_eq!(workspace.snapshot(&original.id).unwrap().text, "abcdef");
}

#[test]
fn external_file_change_is_not_silently_overwritten() {
    let project = Project::new(b"original");
    let mut workspace = project.workspace();
    let original = workspace.open_document("main.tex").unwrap();
    let agent = workspace.apply_edits(&original.id, original.revision, vec![edit(0, 0, 8, "agent")], "agent").unwrap();
    fs::write(project.root().join("main.tex"), b"external").unwrap();
    let error = workspace.save_document(&agent.id, agent.revision).unwrap_err();
    assert_eq!(error.code, "external_change");
    assert_eq!(fs::read(project.root().join("main.tex")).unwrap(), b"external");
    assert!(workspace.snapshot(&agent.id).unwrap().dirty);
}

#[test]
fn parent_traversal_and_invalid_utf8_are_rejected() {
    let project = Project::new(&[0xff, 0xfe, 0xfd]);
    fs::write(project.0.join("outside.tex"), b"private").unwrap();
    let mut workspace = project.workspace();
    assert_eq!(workspace.open_document("../outside.tex").unwrap_err().code, "path_escape");
    assert_eq!(workspace.open_document("main.tex").unwrap_err().code, "invalid_utf8");
    assert_eq!(fs::read(project.root().join("main.tex")).unwrap(), [0xff, 0xfe, 0xfd]);
}

#[cfg(unix)]
#[test]
fn a_symlink_cannot_escape_the_project() {
    let project = Project::new(b"ok");
    fs::write(project.0.join("outside.tex"), b"private").unwrap();
    std::os::unix::fs::symlink(project.0.join("outside.tex"), project.root().join("link.tex")).unwrap();
    assert_eq!(project.workspace().open_document("link.tex").unwrap_err().code, "path_escape");
}

#[cfg(unix)]
#[test]
fn save_rejects_a_file_retargeted_to_another_project_file() {
    let project = Project::new(b"original");
    let mut workspace = project.workspace();
    let original = workspace.open_document("main.tex").unwrap();
    let changed = workspace.apply_edits(&original.id, original.revision, vec![edit(0, 0, 8, "agent")], "agent").unwrap();
    fs::write(project.root().join("other.tex"), b"original").unwrap();
    fs::rename(project.root().join("main.tex"), project.root().join("old.tex")).unwrap();
    std::os::unix::fs::symlink("other.tex", project.root().join("main.tex")).unwrap();
    let result = workspace.save_document(&changed.id, changed.revision);
    assert!(result.is_err(), "A redirected target must not be written");
    assert_eq!(fs::read(project.root().join("other.tex")).unwrap(), b"original");
}

#[test]
fn oversized_file_is_rejected_without_truncation() {
    let bytes = vec![b'x'; 1024 * 1024 + 1];
    let project = Project::new(&bytes);
    assert_eq!(project.workspace().open_document("main.tex").unwrap_err().code, "file_too_large");
    assert_eq!(fs::metadata(project.root().join("main.tex")).unwrap().len(), 1024 * 1024 + 1);
}
