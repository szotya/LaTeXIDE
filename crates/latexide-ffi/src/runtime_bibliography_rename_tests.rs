//! Eight fresh actual Runtime controls, with independent physical file checks.
use super::RuntimeBibRenameError;
#[cfg(target_os = "macos")]
use super::RuntimeBibRenameApply;
use crate::runtime::{Runtime, bibliography::RuntimeCiteCompletion};
use latexide_core::{Workspace, Position, TextEdit, LineEnding};
#[cfg(target_os = "macos")]
use latexide_core::services::bibliography::key_rename::{KeyRenameError, RenameSiteKind};
#[cfg(target_os = "macos")]
use latexide_protocol::Request;
use std::{fs, path::PathBuf, sync::{Arc, Mutex}};

const MAIN: &str = "\\cite{old}\n";
const PART: &str = "\\citep[p.2]{old}\n";
const BIB: &str = "@book{old,title={A}}\n";
struct Fixture { root: PathBuf, files: Vec<String>, nonce: String }
impl Fixture {
    fn new(label: &str, files: &[(&str, &str)], physical: bool) -> Self {
        // Explicit producer/reviewer context keeps its original path and nonce.
        // A normal main libtest can create its own retained fixture context.
        let base = std::env::var_os("P12_ROOT").map(PathBuf::from)
            .unwrap_or_else(|| std::env::temp_dir().join("latexide-runtime-bib-apply"));
        let nonce = std::env::var("P12_NONCE").unwrap_or_else(|_| {
            let mut bytes = [0u8; 16];
            getrandom::fill(&mut bytes).expect("fresh test fixture nonce");
            bytes.iter().map(|byte| format!("{byte:02x}")).collect::<String>()
        });
        let root = base.join("fixtures").join(&nonce).join(label);
        fs::create_dir_all(root.parent().unwrap()).unwrap();
        fs::create_dir(&root).expect("fresh exclusive own fixture");
        fs::create_dir(root.join("project")).unwrap();
        for (name, text) in files {
            let path = root.join("project").join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            let mut raw = if physical { vec![0xef, 0xbb, 0xbf] } else { vec![] };
            raw.extend(if physical { text.replace('\n', "\r\n").into_bytes() }
                else { text.as_bytes().to_vec() });
            fs::write(path, raw).unwrap();
        }
        Self { root, files: files.iter().map(|(name, _)| name.to_string()).collect(), nonce }
    }
    fn standard(label: &str) -> Self {
        Self::new(label, &[("main.tex", MAIN), ("part.tex", PART), ("refs.bib", BIB)], false)
    }
    fn workspace(&self, all: bool) -> Workspace {
        let mut w = Workspace::open(self.root.join("project")).unwrap();
        for name in &self.files { if all || name != "part.tex" { w.open_document(name).unwrap(); } }
        w
    }
    fn owner(&self, all: bool) -> Arc<Mutex<Runtime>> {
        Arc::new(Mutex::new(Runtime::new(self.workspace(all))))
    }
    fn disk(&self) -> Vec<(String, Option<Vec<u8>>)> {
        self.files.iter().map(|name| {
            let bytes = match fs::read(self.root.join("project").join(name)) {
                Ok(raw) => Some(raw), Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                Err(e) => panic!("{e}"),
            };
            (name.clone(), bytes)
        }).collect()
    }
    fn record(&self, detail: String) {
        let row = format!("nonce={}\nproject={:?}\nfixture=actual Runtime::new and main553 Workspace::open\nSave_calls=0;Session_CABI_UI=0\n{detail}\n",
            self.nonce, self.root.join("project"));
        fs::write(self.root.join("observed.txt"), row.as_bytes()).unwrap(); println!("{row}");
    }
}
fn state(owner: &Arc<Mutex<Runtime>>) -> String {
    let r = owner.lock().unwrap();
    format!("snapshots={:?};capture={:?};undo={};redo={}", r.workspace.snapshots(),
        r.workspace.transaction_snapshot().unwrap(), r.workspace.can_undo(), r.workspace.can_redo())
}
fn unchanged(f: &Fixture, owner: &Arc<Mutex<Runtime>>, before: &str,
    disk: &[(String, Option<Vec<u8>>)])
{
    assert_eq!(state(owner), before); assert_eq!(f.disk(), disk);
    assert_eq!(Arc::strong_count(owner), 1);
}
fn error<T>(result: Result<T, RuntimeBibRenameError>) -> RuntimeBibRenameError {
    match result { Err(error) => error, Ok(_) => panic!("typed whole refusal required") }
}
#[cfg(target_os = "macos")]
fn code<T>(result: Result<T, RuntimeBibRenameError>, expected: &str) {
    match error(result) {
        RuntimeBibRenameError::Runtime(e) => assert_eq!(e.code, expected),
        other => panic!("expected Runtime/Core refusal, got {other:?}"),
    }
}
#[cfg(target_os = "macos")]
fn prepare(service: &RuntimeCiteCompletion) -> RuntimeBibRenameApply {
    service.prepare_bib_rename_apply(service.preview_bib_rename("old", "renamed").unwrap()).unwrap()
}
fn edit(owner: &Arc<Mutex<Runtime>>, id: &str, line: u32, start: u32, end: u32, text: &str) {
    let mut r = owner.lock().unwrap(); let revision = r.workspace.snapshot(id).unwrap().revision;
    r.workspace.apply_edits(id, revision, vec![TextEdit { start: Position { line, character: start },
        end: Position { line, character: end }, text: text.into() }], "fixture").unwrap();
}
#[cfg(target_os = "macos")]
fn docs(owner: &Arc<Mutex<Runtime>>, renamed: bool, revision: u64, origin: &str) {
    let r = owner.lock().unwrap(); let all = r.workspace.snapshots(); assert_eq!(all.len(), 3);
    let group = all[0].transaction_id; assert!(group > 0);
    for d in all {
        let original = match d.id.as_str() { "main.tex" => MAIN, "part.tex" => PART, "refs.bib" => BIB,
            _ => panic!("unexpected target") };
        assert_eq!(d.text, if renamed { original.replace("old", "renamed") } else { original.into() });
        assert_eq!(d.revision, revision); assert_eq!(d.transaction_id, group); assert_eq!(d.last_origin, origin);
        assert_eq!(d.dirty, renamed);
    }
}

#[cfg(target_os = "macos")]
#[test]
fn ra01_multifile_atomic_single_use() {
    let f = Fixture::standard("ra01"); let owner = f.owner(true);
    let service = RuntimeCiteCompletion::new(&owner); let before = state(&owner); let disk = f.disk();
    let preview = service.preview_bib_rename("old", "renamed").unwrap(); assert_eq!(preview.edits().len(), 3);
    assert_eq!(preview.edits().iter().filter(|e| e.kind == RenameSiteKind::Declaration).count(), 1);
    let mut display = preview.edits().to_vec(); display[0].insertion = "FORGED".into();
    display[0].source.document_id = "absent.tex".into();
    let mut token = service.prepare_bib_rename_apply(preview).unwrap(); assert!(!token.is_consumed());
    unchanged(&f, &owner, &before, &disk);
    let applied = service.apply_bib_rename(&mut token, "agent").unwrap(); assert_eq!(applied.len(), 3);
    assert!(token.is_consumed()); docs(&owner, true, 2, "agent"); assert_eq!(f.disk(), disk);
    let after = state(&owner); code(service.apply_bib_rename(&mut token, "agent"), "runtime_bib_apply_consumed");
    unchanged(&f, &owner, &after, &disk);
    f.record(format!("atomic_targets=3;one_group=true;copied_display_not_authority={display:?}\nafter={after}"));
}
#[cfg(target_os = "macos")]
#[test]
fn ra02_dirty_unicode_bom_crlf() {
    let main = "\u{feff}😀\n\\cite{réx𝒜}\n";
    let bib = "\u{feff}😀\n@book{réf𝒜,title={U}}\n";
    let f = Fixture::new("ra02", &[("main.tex", main), ("refs.bib", bib)], true);
    let owner = f.owner(true); edit(&owner, "refs.bib", 1, 6, 11, "réx𝒜");
    let service = RuntimeCiteCompletion::new(&owner); let disk = f.disk(); let before = state(&owner);
    let preview = service.preview_bib_rename("réx𝒜", "új𝒜").unwrap(); assert_eq!(preview.edits().len(), 2);
    for e in preview.edits() {
        assert_eq!(e.source.range.start, 10); assert_eq!(e.source.range.length, 5);
        assert_eq!(e.source.line, 1); assert_eq!(e.source.character, 6); assert_eq!(e.expected_old_text, "réx𝒜");
        assert_eq!(e.source.revision, Some(if e.source.document_id == "refs.bib" { 2 } else { 1 }));
    }
    let mut token = service.prepare_bib_rename_apply(preview).unwrap(); unchanged(&f, &owner, &before, &disk);
    assert_eq!(service.apply_bib_rename(&mut token, "agent").unwrap().len(), 2);
    {
        let r = owner.lock().unwrap();
        for d in r.workspace.snapshots() {
            let expected = if d.id == "refs.bib" { bib.replace("réf𝒜", "új𝒜") } else { main.replace("réx𝒜", "új𝒜") };
            assert_eq!(d.text, expected); assert!(d.bom && d.text.starts_with('\u{feff}'));
            assert_eq!(d.line_ending, LineEnding::Crlf); assert!(d.dirty);
            assert_eq!(d.revision, if d.id == "refs.bib" { 3 } else { 2 });
        }
        let a = r.workspace.snapshot("main.tex").unwrap(); let b = r.workspace.snapshot("refs.bib").unwrap();
        assert_eq!(a.transaction_id, b.transaction_id); assert_eq!(a.last_origin, "agent");
    }
    assert_eq!(f.disk(), disk); assert_eq!(Arc::strong_count(&owner), 1);
    f.record(format!("dirty_overlay_real=true;UTF16_10_5=true;physical_BOM_CRLF_unchanged=true\nafter={}", state(&owner)));
}
#[cfg(target_os = "macos")]
#[test]
fn ra03_runtime_shared_undo_redo_disk_unchanged() {
    let f = Fixture::standard("ra03"); let owner = f.owner(true);
    let service = RuntimeCiteCompletion::new(&owner); let disk = f.disk(); let mut token = prepare(&service);
    service.apply_bib_rename(&mut token, "agent").unwrap(); docs(&owner, true, 2, "agent");
    assert!(owner.lock().unwrap().dispatch(Request::Undo, "user").ok);
    docs(&owner, false, 3, "undo:agent");
    { let r = owner.lock().unwrap(); assert!(!r.workspace.can_undo()); assert!(r.workspace.can_redo()); }
    assert_eq!(f.disk(), disk);
    assert!(owner.lock().unwrap().dispatch(Request::Redo, "user").ok);
    docs(&owner, true, 4, "redo:agent");
    { let r = owner.lock().unwrap(); assert!(r.workspace.can_undo()); assert!(!r.workspace.can_redo()); }
    assert_eq!(f.disk(), disk); assert_eq!(Arc::strong_count(&owner), 1);
    f.record(format!("actual_Runtime_dispatch_Undo_Redo=true;one_group_for_all3=true\nafter={}", state(&owner)));
}
#[cfg(target_os = "macos")]
#[test]
fn ra04_foreign_owner_consumes_without_edit() {
    let f = Fixture::standard("ra04"); let owner = f.owner(true); let foreign = f.owner(true);
    let service = RuntimeCiteCompletion::new(&owner); let other = RuntimeCiteCompletion::new(&foreign);
    assert_eq!(owner.lock().unwrap().workspace.transaction_snapshot().unwrap().files,
        foreign.lock().unwrap().workspace.transaction_snapshot().unwrap().files);
    let a = state(&owner); let b = state(&foreign); let disk = f.disk();
    code(other.prepare_bib_rename_apply(service.preview_bib_rename("old", "renamed").unwrap()), "runtime_bib_apply_owner_mismatch");
    let mut token = prepare(&service);
    code(other.apply_bib_rename(&mut token, "agent"), "runtime_bib_apply_owner_mismatch"); assert!(token.is_consumed());
    code(service.apply_bib_rename(&mut token, "agent"), "runtime_bib_apply_consumed");
    unchanged(&f, &owner, &a, &disk); unchanged(&f, &foreign, &b, &disk);
    f.record(format!("identical_files_and_revisions_foreign_allocation_refused=true\nowner={a}\nforeign={b}"));
}
#[cfg(target_os = "macos")]
#[test]
fn ra05_stale_aba_disk_generation_refuse() {
    for variant in ["edit", "aba", "disk", "generation", "context"] {
        let f = Fixture::standard(&format!("ra05-{variant}")); let owner = f.owner(true);
        let service = RuntimeCiteCompletion::new(&owner); let mut token = prepare(&service);
        let original = owner.lock().unwrap().workspace.snapshot("main.tex").unwrap();
        let expected = match variant {
            "edit" | "aba" => {
                edit(&owner, "main.tex", 0, 6, 9, "alt");
                if variant == "aba" {
                    assert!(owner.lock().unwrap().dispatch(Request::Undo, "user").ok);
                    let now = owner.lock().unwrap().workspace.snapshot("main.tex").unwrap();
                    assert_eq!(now.text, original.text); assert!(now.revision > original.revision);
                }
                "stale_snapshot"
            }
            "disk" => {
                let path = f.root.join("project/refs.bib"); let m = fs::metadata(&path).unwrap();
                let modified = m.modified().unwrap(); fs::write(&path, BIB.replace("{A}", "{B}")).unwrap();
                fs::OpenOptions::new().write(true).open(&path).unwrap()
                    .set_times(fs::FileTimes::new().set_modified(modified)).unwrap();
                let now = fs::metadata(&path).unwrap(); assert_eq!(now.len(), m.len()); assert_eq!(now.modified().unwrap(), modified);
                "stale_analysis_snapshot"
            }
            "generation" => { owner.lock().unwrap().workspace = f.workspace(true); "stale_snapshot" }
            "context" => { owner.lock().unwrap().session_descriptor = f.root.join("context-only.json"); "stale_runtime_bib_context" }
            _ => unreachable!(),
        };
        let before = state(&owner); let disk = f.disk();
        code(service.apply_bib_rename(&mut token, "agent"), expected); assert!(token.is_consumed());
        unchanged(&f, &owner, &before, &disk);
        f.record(format!("variant={variant};exact_refusal={expected};no_partial_edit=true\nbefore={before}"));
    }
}
#[cfg(target_os = "macos")]
#[test]
fn ra06_collision_duplicate_bad_target_whole_refusal() {
    for (label, bib, duplicate) in [("duplicate", "@book{old,title={A}}\n@book{old,title={B}}\n", true),
        ("collision", "@book{old,title={A}}\n@book{renamed,title={B}}\n", false)]
    {
        let f = Fixture::new(&format!("ra06-{label}"), &[("main.tex", MAIN), ("refs.bib", bib)], false);
        let owner = f.owner(true); let service = RuntimeCiteCompletion::new(&owner);
        let before = state(&owner); let disk = f.disk(); let e = error(service.preview_bib_rename("old", "renamed"));
        if duplicate { assert!(matches!(e, RuntimeBibRenameError::Preview(KeyRenameError::AmbiguousDeclaration { count: 2 }))); }
        else { assert!(matches!(e, RuntimeBibRenameError::Preview(KeyRenameError::NewKeyCollision))); }
        unchanged(&f, &owner, &before, &disk); f.record(format!("initial={label};typed_refusal={e:?};no_partial_edit=true"));
    }
    let f = Fixture::standard("ra06-closed"); let owner = f.owner(false); let service = RuntimeCiteCompletion::new(&owner);
    let before = state(&owner); let disk = f.disk(); let preview = service.preview_bib_rename("old", "renamed").unwrap();
    assert_eq!(preview.edits().len(), 3);
    code(service.prepare_bib_rename_apply(preview), "bib_rename_target_not_open");
    unchanged(&f, &owner, &before, &disk); assert_eq!(owner.lock().unwrap().workspace.snapshots().len(), 2);
    f.record("one_closed_target_whole_refusal=true;implicit_open=0".into());
    for variant in ["late-collision", "missing"] {
        let f = Fixture::standard(&format!("ra06-{variant}")); let owner = f.owner(true);
        let service = RuntimeCiteCompletion::new(&owner); let mut token = prepare(&service);
        let expected = if variant == "late-collision" {
            edit(&owner, "refs.bib", 1, 0, 0, "@book{renamed,title={B}}\n"); "stale_snapshot"
        } else { fs::remove_file(f.root.join("project/part.tex")).unwrap(); "stale_analysis_snapshot" };
        let before = state(&owner); let disk = f.disk(); code(service.apply_bib_rename(&mut token, "agent"), expected);
        assert!(token.is_consumed()); unchanged(&f, &owner, &before, &disk);
        if variant == "late-collision" {
            assert!(matches!(error(service.preview_bib_rename("old", "renamed")), RuntimeBibRenameError::Preview(KeyRenameError::NewKeyCollision)));
        }
        f.record(format!("variant={variant};whole_refusal={expected};unchanged_other_targets=true\nbefore={before}"));
    }
}
#[cfg(target_os = "macos")]
#[test]
fn ra07_contention_try_lock_spends_token() {
    let f = Fixture::standard("ra07"); let owner = f.owner(true); let service = RuntimeCiteCompletion::new(&owner);
    let preview = service.preview_bib_rename("old", "renamed").unwrap(); let mut token = prepare(&service);
    let before = state(&owner); let disk = f.disk();
    let guard = owner.lock().unwrap();
    code(service.prepare_bib_rename_apply(preview), "runtime_busy");
    code(service.apply_bib_rename(&mut token, "agent"), "runtime_busy");
    assert!(token.is_consumed()); drop(guard);
    code(service.apply_bib_rename(&mut token, "agent"), "runtime_bib_apply_consumed");
    unchanged(&f, &owner, &before, &disk);
    f.record(format!("both_calls_refused_while_actual_owner_still_locked=true;outer_watchdog_only=true\nbefore={before}"));
}
#[cfg(target_os = "macos")]
#[test]
fn ra08_teardown_and_atomic_edit_quota() {
    let f = Fixture::standard("ra08-teardown"); let owner = f.owner(true); let weak = Arc::downgrade(&owner);
    let service = RuntimeCiteCompletion::new(&owner); let mut token = prepare(&service); let disk = f.disk();
    assert_eq!(Arc::strong_count(&owner), 1); drop(owner); assert!(weak.upgrade().is_none());
    code(service.apply_bib_rename(&mut token, "agent"), "runtime_unavailable"); assert!(token.is_consumed());
    assert_eq!(f.disk(), disk); f.record("Weak_owner_gone=true;retained_strong_owner=0;no_disk_change=true".into());
    let source = MAIN.repeat(1001);
    let f = Fixture::new("ra08-quota", &[("main.tex", &source), ("refs.bib", BIB)], false);
    let owner = f.owner(true); let service = RuntimeCiteCompletion::new(&owner); let before = state(&owner); let disk = f.disk();
    let preview = service.preview_bib_rename("old", "renamed").unwrap(); assert_eq!(preview.edits().len(), 1002);
    code(service.prepare_bib_rename_apply(preview), "bib_rename_quota");
    unchanged(&f, &owner, &before, &disk); assert!(!owner.lock().unwrap().workspace.can_undo());
    f.record("actual_Core_1000_edit_budget=refusal;1002_sites;all_buffers_and_history_unchanged=true".into());
}

// The production P04 filesystem producer is intentionally macOS-only. Other
// hosts must prove the whole refusal, never substitute a successful analysis
// model or present these as positive bibliography-apply acceptance.
#[cfg(not(target_os = "macos"))]
fn unsupported_preview(service: &RuntimeCiteCompletion) {
    match error(service.preview_bib_rename("old", "renamed")) {
        RuntimeBibRenameError::Runtime(e) => {
            assert_eq!(e.code, "analysis_outside_root");
            assert_eq!(e.message, "Complete P04 analysis observation refused: OutsideRoot");
        }
        other => panic!("expected unsupported production analysis refusal, got {other:?}"),
    }
}
#[cfg(not(target_os = "macos"))]
#[test]
fn ra09_unconfirmed_analysis_refuses_clean_workspace() {
    for all in [false, true] {
        let f = Fixture::standard(if all { "ra09-all" } else { "ra09-partial" });
        assert!(f.root.join("project").is_dir());
        let owner = f.owner(all); let service = RuntimeCiteCompletion::new(&owner);
        assert_eq!(owner.lock().unwrap().workspace.snapshots().len(), if all { 3 } else { 2 });
        let before = state(&owner); let disk = f.disk();
        unsupported_preview(&service);
        unchanged(&f, &owner, &before, &disk);
        let runtime = owner.lock().unwrap();
        assert!(!runtime.workspace.can_undo() && !runtime.workspace.can_redo());
        drop(runtime);
        f.record("unsupported_production_analysis=whole_refusal;open_and_closed_targets_unchanged=true;no_apply_token=true".into());
    }
}
#[cfg(not(target_os = "macos"))]
#[test]
fn ra10_unconfirmed_analysis_preserves_dirty_unicode_bom_crlf() {
    let main = "\u{feff}😀\n\\cite{old}\n";
    let bib = "\u{feff}😀\n@book{alt,title={U}}\n";
    let f = Fixture::new("ra10-dirty", &[("main.tex", main), ("refs.bib", bib)], true);
    let owner = f.owner(true); edit(&owner, "refs.bib", 1, 6, 9, "old");
    {
        let runtime = owner.lock().unwrap();
        let dirty = runtime.workspace.snapshot("refs.bib").unwrap();
        assert!(dirty.dirty && dirty.bom);
        assert_eq!(dirty.line_ending, LineEnding::Crlf);
        assert_eq!(dirty.revision, 2);
        assert!(runtime.workspace.can_undo() && !runtime.workspace.can_redo());
    }
    let service = RuntimeCiteCompletion::new(&owner); let before = state(&owner); let disk = f.disk();
    unsupported_preview(&service);
    unchanged(&f, &owner, &before, &disk);
    f.record("unsupported_production_analysis=whole_refusal;dirty_UTF16_revision_history_and_physical_BOM_CRLF_unchanged=true;no_apply_token=true".into());
}
