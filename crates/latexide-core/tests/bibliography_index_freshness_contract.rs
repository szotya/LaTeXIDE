//! Actual disk fixtures and public Workspace consumers; fixtures remain for readback.
use latexide_core::{completion::CompletionSet, Position, TextEdit, Workspace};
use std::{alloc::{GlobalAlloc, Layout, System}, fs::{self, File, FileTimes}, path::PathBuf,
    sync::{Mutex, atomic::{AtomicUsize, Ordering}}, time::Duration};

// Observe parse-sized allocations without adding instrumentation to production.
// A padded input makes the canonical parser's UTF-16 coordinate table observable;
// bounded file buffers are smaller than the independently chosen threshold.
struct ObserveAlloc;
static THRESHOLD: AtomicUsize = AtomicUsize::new(0);
static LARGE: AtomicUsize = AtomicUsize::new(0);
static NEXT_PROJECT: AtomicUsize = AtomicUsize::new(0);
// The allocation observer is process-wide; keep these cases isolated even
// when the CI invokes this target with libtest's default parallelism.
static CASE_SERIAL: Mutex<()> = Mutex::new(());
fn observe(size: usize) {
    let threshold = THRESHOLD.load(Ordering::Relaxed);
    if threshold > 0 && size >= threshold { LARGE.fetch_add(1, Ordering::Relaxed); }
}
unsafe impl GlobalAlloc for ObserveAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 { observe(layout.size()); unsafe { System.alloc(layout) } }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 { observe(layout.size()); unsafe { System.alloc_zeroed(layout) } }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) { unsafe { System.dealloc(ptr, layout) } }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 { observe(size); unsafe { System.realloc(ptr, layout, size) } }
}
#[global_allocator]
static ALLOCATOR: ObserveAlloc = ObserveAlloc;

struct Project(PathBuf);
impl Project {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("p12-freshness-{}-{}", std::process::id(), NEXT_PROJECT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&root).unwrap();
        println!("OWN_FIXTURE={}", root.display());
        Self(root)
    }
    fn write(&self, id: &str, text: &str) { fs::write(self.0.join(id), text).unwrap(); }
    fn rewrite_same_metadata(&self, text: &str) {
        let path = self.0.join("refs.bib");
        let before = fs::metadata(&path).unwrap();
        assert_eq!(before.len(), text.len() as u64, "FIXTURE_EQUAL_LENGTH");
        let old = fs::read(&path).unwrap();
        assert_ne!(old, text.as_bytes(), "FIXTURE_DIFFERENT_CONTENT");
        self.write("refs.bib", text);
        File::options().write(true).open(&path).unwrap()
            .set_times(FileTimes::new().set_modified(before.modified().unwrap())).unwrap();
        let after = fs::metadata(path).unwrap();
        assert_eq!(before.len(), after.len());
        assert_eq!(before.modified().unwrap(), after.modified().unwrap(), "FIXTURE_EQUAL_MTIME");
    }
    fn workspace(&self, key: &str) -> (Workspace, u32) {
        let source = format!("\\cite{{{key}}}\n\\cite{{}}");
        let caret = source[..source.rfind('}').unwrap()].encode_utf16().count() as u32;
        self.write("main.tex", &source);
        let mut ws = Workspace::open(self.0.clone()).unwrap();
        ws.open_document("main.tex").unwrap();
        (ws, caret)
    }
}
fn complete(ws: &mut Workspace, caret: u32) -> CompletionSet { ws.completions("main.tex", 1, caret).unwrap() }
fn labels(set: &CompletionSet) -> Vec<&str> { set.items.iter().map(|i| i.label.as_str()).collect() }
fn expire() { std::thread::sleep(Duration::from_millis(1100)); }
fn replace(ws: &mut Workspace, id: &str, text: &str) -> u64 {
    let snapshot = ws.snapshot(id).unwrap();
    let mut end = Position { line: 0, character: 0 };
    for ch in snapshot.text.chars() {
        if ch == '\n' { end.line += 1; end.character = 0; } else { end.character += ch.len_utf16() as u32; }
    }
    ws.apply_edits(id, snapshot.revision, vec![TextEdit { start: Position { line: 0, character: 0 }, end, text: text.into() }], "agent").unwrap().revision
}

#[test]
fn freshness_01_same_size_mtime_key_normal_refresh() {
    let _serial = CASE_SERIAL.lock().unwrap();
    let p = Project::new(); p.write("refs.bib", "@book{oldKey,title={A}}\n");
    let (mut ws, caret) = p.workspace("newKey");
    assert_eq!(labels(&complete(&mut ws, caret)), vec!["oldKey"]);
    p.rewrite_same_metadata("@book{newKey,title={B}}\n"); expire();
    let set = complete(&mut ws, caret);
    let target = ws.definition("main.tex", 1, 6).unwrap().target.unwrap();
    println!("NORMAL_KEYS={:?}; FORCED_DEFINITION={:?}", labels(&set), target);
    assert_eq!(labels(&set), vec!["newKey"], "SAME_METADATA_CONTENT_FRESH");
    assert_eq!((target.document_id.as_str(), target.start_utf16, target.length_utf16, target.revision), ("refs.bib", 6, 6, None));
}

#[test]
fn freshness_02_same_size_mtime_value_and_range() {
    let _serial = CASE_SERIAL.lock().unwrap();
    let p = Project::new(); p.write("refs.bib", "@book{same,title={AB}}\n");
    let (mut ws, caret) = p.workspace("same");
    assert!(complete(&mut ws, caret).items[0].detail.contains("refs.bib:1"));
    p.rewrite_same_metadata("\n@book{same,title={C}}\n"); expire();
    let set = complete(&mut ws, caret);
    let target = ws.definition("main.tex", 1, 6).unwrap().target.unwrap();
    println!("NORMAL_DETAIL={:?}; FORCED_DEFINITION={:?}", set.items[0].detail, target);
    assert!(set.items[0].detail.contains("refs.bib:2"), "SAME_METADATA_RANGE_FRESH");
    assert_eq!((target.start_utf16, target.length_utf16, target.line), (7, 4, 2));
}

#[test]
fn freshness_03_disk_delete_and_recreate() {
    let _serial = CASE_SERIAL.lock().unwrap();
    let p = Project::new(); p.write("refs.bib", "@book{oldKey,title={T}}\n");
    let (mut ws, caret) = p.workspace("newKey");
    assert_eq!(labels(&complete(&mut ws, caret)), vec!["oldKey"]);
    fs::remove_file(p.0.join("refs.bib")).unwrap(); expire();
    assert!(complete(&mut ws, caret).items.is_empty());
    p.write("refs.bib", "@book{newKey,title={T}}\n"); expire();
    assert_eq!(labels(&complete(&mut ws, caret)), vec!["newKey"]);
    assert_eq!(ws.definition("main.tex", 1, 6).unwrap().target.unwrap().length_utf16, 6);
}

#[test]
fn freshness_04_dirty_open_buffer_shadows_changed_disk() {
    let _serial = CASE_SERIAL.lock().unwrap();
    let p = Project::new(); p.write("refs.bib", "@book{diskA,title={T}}\n");
    let (mut ws, caret) = p.workspace("dirty"); ws.open_document("refs.bib").unwrap();
    assert_eq!(replace(&mut ws, "refs.bib", "@book{dirty,title={T}}\n"), 2);
    p.rewrite_same_metadata("@book{diskB,title={T}}\n"); ws.refresh_completion_index();
    assert_eq!(labels(&complete(&mut ws, caret)), vec!["dirty"]);
    let target = ws.definition("main.tex", 1, 6).unwrap().target.unwrap();
    assert_eq!(target.revision, Some(2));
    assert!(ws.snapshot("refs.bib").unwrap().dirty);
    assert_eq!(fs::read_to_string(p.0.join("refs.bib")).unwrap(), "@book{diskB,title={T}}\n");
}

#[test]
fn freshness_05_explicit_refresh_before_ttl_expiry() {
    let _serial = CASE_SERIAL.lock().unwrap();
    let p = Project::new(); p.write("refs.bib", "@book{oldKey,title={T}}\n");
    let (mut ws, caret) = p.workspace("newKey");
    assert_eq!(labels(&complete(&mut ws, caret)), vec!["oldKey"]);
    p.rewrite_same_metadata("@book{newKey,title={T}}\n"); ws.refresh_completion_index();
    assert_eq!(labels(&complete(&mut ws, caret)), vec!["newKey"]);
    assert_eq!(ws.definition("main.tex", 1, 6).unwrap().target.unwrap().start_utf16, 6);
}

fn measured(ws: &mut Workspace, caret: u32, threshold: usize) -> (CompletionSet, usize) {
    LARGE.store(0, Ordering::Relaxed); THRESHOLD.store(threshold, Ordering::Relaxed);
    let set = complete(ws, caret);
    THRESHOLD.store(0, Ordering::Relaxed);
    (set, LARGE.load(Ordering::Relaxed))
}
#[test]
fn freshness_06_unchanged_file_reuses_parsed_symbols() {
    let _serial = CASE_SERIAL.lock().unwrap();
    let p = Project::new();
    let old = format!("%{}\n@book{{oldKey,title={{T}}}}\n", "x".repeat(32768));
    let new = old.replace("oldKey", "newKey");
    p.write("refs.bib", &old); let (mut ws, caret) = p.workspace("newKey");
    let (cold_set, cold) = measured(&mut ws, caret, old.len() * 3);
    let (_, warm) = measured(&mut ws, caret, old.len() * 3);
    ws.refresh_completion_index(); let (unchanged_set, unchanged) = measured(&mut ws, caret, old.len() * 3);
    p.rewrite_same_metadata(&new); ws.refresh_completion_index();
    let (changed_set, changed) = measured(&mut ws, caret, old.len() * 3);
    println!("PARSE_SIZED_ALLOCS cold={cold} warm={warm} unchanged={unchanged} changed={changed}");
    assert_eq!(labels(&cold_set), vec!["oldKey"]); assert_eq!(labels(&unchanged_set), vec!["oldKey"]);
    assert_eq!(labels(&changed_set), vec!["newKey"]);
    assert!(cold > 0 && changed > 0, "ALLOCATION_OBSERVER_POSITIVE_CONTROL");
    assert_eq!(warm, 0);
    assert_eq!(unchanged, 0, "UNCHANGED_DISK_REUSES_PARSED_SYMBOLS");
}

#[test]
fn freshness_07_over_quota_file_remains_partial_after_change() {
    let _serial = CASE_SERIAL.lock().unwrap();
    let p = Project::new(); p.write("refs.bib", "@book{oldKey,title={T}}\n");
    let (mut ws, caret) = p.workspace("newKey"); complete(&mut ws, caret);
    p.write("oversized.bib", &"x".repeat(latexide_core::MAX_FILE_BYTES + 1));
    p.rewrite_same_metadata("@book{newKey,title={T}}\n"); ws.refresh_completion_index();
    let set = complete(&mut ws, caret); assert_eq!(labels(&set), vec!["newKey"]);
    let status = set.index_status.unwrap();
    assert!(status.truncated); assert_eq!(status.skipped_files, 1); assert_eq!(status.indexed_files, 2);
}

#[test]
fn freshness_08_save_then_undo_keeps_authoritative_dirty_revision() {
    let _serial = CASE_SERIAL.lock().unwrap();
    let p = Project::new(); p.write("refs.bib", "@book{oldKey,title={T}}\n");
    let (mut ws, caret) = p.workspace("oldKey"); ws.open_document("refs.bib").unwrap();
    assert_eq!(replace(&mut ws, "refs.bib", "@book{newKey,title={T}}\n"), 2);
    assert_eq!(labels(&complete(&mut ws, caret)), vec!["newKey"]);
    let saved = ws.save_document("refs.bib", 2).unwrap(); assert!(!saved.dirty);
    let undone = ws.undo().unwrap(); assert_eq!(undone.revision, 3); assert!(undone.dirty);
    ws.refresh_completion_index(); assert_eq!(labels(&complete(&mut ws, caret)), vec!["oldKey"]);
    let target = ws.definition("main.tex", 1, 6).unwrap().target.unwrap(); assert_eq!(target.revision, Some(3));
    assert_eq!(fs::read_to_string(p.0.join("refs.bib")).unwrap(), "@book{newKey,title={T}}\n");
}
