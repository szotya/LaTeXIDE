//! Own real files, host Mutex lease, and processes. Fixtures are retained.
use super::*;
use std::{
    fs::{self, FileTimes},
    os::unix::fs::{MetadataExt, PermissionsExt, symlink},
    sync::{
        Mutex, MutexGuard,
        atomic::{AtomicU64, Ordering},
    },
};

static NEXT: AtomicU64 = AtomicU64::new(1);
struct Authority(Mutex<HostSnapshot>);
impl HostInputAuthority for Authority {
    type Lease<'a> = MutexGuard<'a, HostSnapshot>;
    fn try_lease_current(&self) -> Result<Self::Lease<'_>, GenerationError> {
        self.0
            .try_lock()
            .map_err(|_| GenerationError::HostUnavailable)
    }
}

struct Fixture {
    base: PathBuf,
    project: PathBuf,
    generation: PathBuf,
    authority: Authority,
}
impl Fixture {
    fn new() -> Self {
        let phase = PathBuf::from(
            std::env::var_os("LATEXIDE_P18_PHASE3_ROOT").expect("explicit own Phase3 root"),
        )
        .canonicalize()
        .unwrap();
        let base = phase.join("fixtures").join(format!(
            "generation-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        private_dir(&base);
        let project = base.join("Árvíztűrő projekt");
        private_dir(&project);
        fs::write(project.join("main.tex"), b"own project").unwrap();
        let authority = Authority(Mutex::new(snapshot(
            &project,
            1,
            vec![HostInput::disk("main.tex", 7, b"own project".to_vec()).unwrap()],
        )));
        Self {
            generation: base.join("run"),
            base,
            project,
            authority,
        }
    }
    fn binding(&self) -> InputBinding {
        self.authority
            .try_lease_current()
            .unwrap()
            .binding()
            .clone()
    }
    fn capture(&self) -> Result<CapturedInputs, GenerationError> {
        CapturedInputs::capture(
            &self.authority,
            &self.binding(),
            &self.generation,
            &AtomicBool::new(false),
        )
    }
}
fn private_dir(path: &Path) {
    fs::create_dir(path).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}
fn snapshot(project: &Path, generation: u64, files: Vec<HostInput>) -> HostSnapshot {
    HostSnapshot::new(
        [19; 16],
        generation,
        project.into(),
        "main.tex".into(),
        files,
    )
    .unwrap()
}
fn overwrite_same_metadata(path: &Path, bytes: &[u8]) {
    let before = fs::metadata(path).unwrap();
    fs::write(path, bytes).unwrap();
    fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_times(FileTimes::new().set_modified(before.modified().unwrap()))
        .unwrap();
    let after = fs::metadata(path).unwrap();
    assert_eq!(before.ino(), after.ino());
    assert_eq!(before.len(), after.len());
    assert_eq!(before.modified().unwrap(), after.modified().unwrap());
}

#[test]
fn digest_binds_actual_bytes_against_independent_sha256_vectors() {
    for (bytes, expected) in [
        (
            Vec::new(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        ),
        (
            b"abc".to_vec(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        ),
        (
            vec![b'a'; 1_000_000],
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0",
        ),
    ] {
        assert_eq!(Digest::of(&bytes).unwrap().hex(), expected);
    }
}

#[test]
fn rename_window_keeps_generation_private_and_host_locked_until_ready() {
    let f = Fixture::new();
    let mut observed = false;
    let captured = CapturedInputs::capture_with_hook(
        &f.authority,
        &f.binding(),
        &f.generation,
        &AtomicBool::new(false),
        &mut |point| {
            if matches!(point, CapturePoint::AfterRenameBeforeSeal) {
                observed = true;
                assert!(f.authority.try_lease_current().is_err());
                assert_eq!(fs::metadata(&f.generation).unwrap().mode() & 0o777, 0o700);
                assert_eq!(
                    fs::metadata(f.generation.join("input")).unwrap().mode() & 0o777,
                    0o700
                );
                assert_eq!(
                    fs::metadata(f.generation.join("input/main.tex"))
                        .unwrap()
                        .mode()
                        & 0o777,
                    0o400
                );
                assert!(!f.generation.join("output").exists());
            }
        },
    )
    .unwrap();
    assert!(
        observed,
        "rename/seal transaction boundary was not observed"
    );
    assert_eq!(
        fs::metadata(captured.input_root()).unwrap().mode() & 0o777,
        0o500
    );
    assert_eq!(
        fs::read(captured.input_root().join("main.tex")).unwrap(),
        b"own project"
    );
}

#[test]
fn changed_snapshot_during_private_rename_window_never_returns_ready() {
    let f = Fixture::new();
    let mut observed = false;
    let result = CapturedInputs::capture_with_hook(
        &f.authority,
        &f.binding(),
        &f.generation,
        &AtomicBool::new(false),
        &mut |point| {
            if matches!(point, CapturePoint::AfterRenameBeforeSeal) {
                observed = true;
                let path = f.generation.join("input/main.tex");
                fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
                overwrite_same_metadata(&path, b"own proJect");
                fs::set_permissions(&path, fs::Permissions::from_mode(0o400)).unwrap();
            }
        },
    );
    assert!(
        observed,
        "rename/seal transaction boundary was not observed"
    );
    assert!(matches!(result, Err(GenerationError::SnapshotChanged)));
    assert!(!f.generation.join("output").exists());
    assert_eq!(
        fs::read(f.project.join("main.tex")).unwrap(),
        b"own project"
    );
}

#[test]
fn cancellation_during_private_rename_window_never_returns_ready() {
    let f = Fixture::new();
    let cancel = AtomicBool::new(false);
    let mut observed = false;
    let result = CapturedInputs::capture_with_hook(
        &f.authority,
        &f.binding(),
        &f.generation,
        &cancel,
        &mut |point| {
            if matches!(point, CapturePoint::AfterRenameBeforeSeal) {
                observed = true;
                cancel.store(true, Ordering::Release);
            }
        },
    );
    assert!(
        observed,
        "rename/seal transaction boundary was not observed"
    );
    assert!(matches!(result, Err(GenerationError::Cancelled)));
    assert_eq!(
        fs::metadata(f.generation.join("input")).unwrap().mode() & 0o777,
        0o500
    );
    assert!(!f.generation.join("output").exists());
}

#[test]
fn capture_preserves_raw_unicode_bom_crlf_and_nested_paths() {
    let f = Fixture::new();
    let bytes = "\u{feff}Árvíztűrő\r\nMásodik sor\r\n".as_bytes().to_vec();
    private_dir(&f.project.join("szöveg"));
    fs::write(f.project.join("szöveg/ő.tex"), &bytes).unwrap();
    *f.authority.0.lock().unwrap() = snapshot(
        &f.project,
        2,
        vec![
            HostInput::disk("main.tex", 7, b"own project".to_vec()).unwrap(),
            HostInput::disk("szöveg/ő.tex", 8, bytes.clone()).unwrap(),
        ],
    );
    let captured = f.capture().unwrap();
    assert_eq!(
        fs::read(captured.input_root().join("szöveg/ő.tex")).unwrap(),
        bytes
    );
    assert_eq!(
        fs::read(captured.input_root().join("main.tex")).unwrap(),
        b"own project"
    );
    assert!(!f.generation.join("input.pending").exists());
    assert_eq!(
        fs::metadata(captured.input_root()).unwrap().mode() & 0o777,
        0o500
    );
    assert_eq!(
        fs::metadata(captured.input_root().join("main.tex"))
            .unwrap()
            .mode()
            & 0o777,
        0o400
    );
}

#[test]
fn dirty_host_buffer_compiles_its_bytes_without_saving_the_original_disk() {
    let f = Fixture::new();
    *f.authority.0.lock().unwrap() = snapshot(
        &f.project,
        2,
        vec![
            HostInput::buffer(
                "main.tex",
                8,
                b"dirty host buffer\r\n".to_vec(),
                Digest::of(b"own project").unwrap(),
            )
            .unwrap(),
        ],
    );
    let captured = f.capture().unwrap();
    assert_eq!(
        fs::read(captured.input_root().join("main.tex")).unwrap(),
        b"dirty host buffer\r\n"
    );
    assert_eq!(
        fs::read(f.project.join("main.tex")).unwrap(),
        b"own project"
    );
}

#[test]
fn stale_host_generation_cannot_create_an_input_generation() {
    let f = Fixture::new();
    let old = f.binding();
    *f.authority.0.lock().unwrap() = snapshot(
        &f.project,
        2,
        vec![HostInput::disk("main.tex", 7, b"own project".to_vec()).unwrap()],
    );
    assert!(matches!(
        CapturedInputs::capture(&f.authority, &old, &f.generation, &AtomicBool::new(false)),
        Err(GenerationError::StaleHost)
    ));
    assert!(!f.generation.exists());
}

#[test]
fn source_digest_not_only_inode_size_or_mtime_controls_capture() {
    let f = Fixture::new();
    overwrite_same_metadata(&f.project.join("main.tex"), b"bad project");
    assert!(matches!(f.capture(), Err(GenerationError::SourceChanged)));
    assert!(!f.generation.join("input").exists());
}

#[test]
fn incorrect_disk_baseline_digest_cannot_be_used_with_fresh_host_buffer_bytes() {
    let f = Fixture::new();
    *f.authority.0.lock().unwrap() = snapshot(
        &f.project,
        2,
        vec![
            HostInput::buffer(
                "main.tex",
                8,
                b"new buffer".to_vec(),
                Digest::of(b"wrong disk").unwrap(),
            )
            .unwrap(),
        ],
    );
    assert!(matches!(f.capture(), Err(GenerationError::SourceChanged)));
}

#[test]
fn symlink_and_hardlink_sources_are_refused_before_publication() {
    for link in [false, true] {
        let f = Fixture::new();
        let outside = f.base.join("outside");
        fs::write(&outside, b"own project").unwrap();
        if link {
            fs::hard_link(f.project.join("main.tex"), f.base.join("alias")).unwrap();
        } else {
            fs::remove_file(f.project.join("main.tex")).unwrap();
            symlink(&outside, f.project.join("main.tex")).unwrap();
        }
        assert!(matches!(f.capture(), Err(GenerationError::SourceChanged)));
        assert!(!f.generation.join("input").exists());
        assert_eq!(fs::read(outside).unwrap(), b"own project");
    }
}

#[test]
fn capture_cannot_escape_or_alias_the_explicit_relative_input_set() {
    for path in [
        "",
        "/main.tex",
        "../main.tex",
        "a/../main.tex",
        "./main.tex",
        "a//b",
        "a/",
        "C:\\main.tex",
        "https://x",
        "a\n.tex",
    ] {
        assert!(
            matches!(
                HostInput::disk(path, 1, vec![1]),
                Err(GenerationError::InvalidInput)
            ),
            "{path:?}"
        );
    }
    let f = Fixture::new();
    *f.authority.0.lock().unwrap() = snapshot(
        &f.project,
        2,
        vec![
            HostInput::disk("main.tex", 1, b"own project".to_vec()).unwrap(),
            HostInput::disk("MAIN.tex", 2, b"own project".to_vec()).unwrap(),
        ],
    );
    assert!(f.capture().is_err()); // Case alias shares the real inode on this volume.
    assert!(!f.generation.join("input").exists());
}

#[test]
fn host_generation_and_input_quotas_are_enforced_without_opening_editor_buffers() {
    assert!(matches!(
        HostInput::disk("x", 1, vec![0; 1_048_577]),
        Err(GenerationError::Quota)
    ));
    assert!(matches!(
        HostInput::disk("x", 0, vec![0]),
        Err(GenerationError::InvalidInput)
    ));
    let f = Fixture::new();
    let eleven = (0..11)
        .map(|i| HostInput::disk(&format!("{i}.tex"), 1, vec![0]).unwrap())
        .collect();
    assert!(matches!(
        HostSnapshot::new([19; 16], 1, f.project.clone(), "0.tex".into(), eleven),
        Err(GenerationError::Quota)
    ));
    assert!(matches!(
        HostSnapshot::new([19; 16], 0, f.project.clone(), "main.tex".into(), vec![]),
        Err(GenerationError::InvalidInput)
    ));
    assert!(matches!(
        HostSnapshot::new(
            [19; 16],
            1,
            f.project.clone(),
            "missing.tex".into(),
            vec![HostInput::disk("main.tex", 1, vec![]).unwrap()]
        ),
        Err(GenerationError::InvalidInput)
    ));
}

#[test]
fn cancelled_capture_never_publishes_a_ready_generation() {
    let f = Fixture::new();
    assert!(matches!(
        CapturedInputs::capture(
            &f.authority,
            &f.binding(),
            &f.generation,
            &AtomicBool::new(true)
        ),
        Err(GenerationError::Cancelled)
    ));
    assert!(!f.generation.exists());
    let cancel = AtomicBool::new(false);
    let result = CapturedInputs::capture_with_hook(
        &f.authority,
        &f.binding(),
        &f.generation,
        &cancel,
        &mut |point| {
            if matches!(point, CapturePoint::AfterInput(0)) {
                cancel.store(true, Ordering::Release);
            }
        },
    );
    assert!(matches!(result, Err(GenerationError::Cancelled)));
    assert!(!f.generation.join("input").exists());
}

#[test]
fn deterministic_change_during_capture_prevents_atomic_publication() {
    let f = Fixture::new();
    let result = CapturedInputs::capture_with_hook(
        &f.authority,
        &f.binding(),
        &f.generation,
        &AtomicBool::new(false),
        &mut |point| {
            if matches!(point, CapturePoint::AfterInput(0)) {
                overwrite_same_metadata(&f.project.join("main.tex"), b"bad project");
            }
        },
    );
    assert!(matches!(result, Err(GenerationError::SourceChanged)));
    assert!(!f.generation.join("input").exists());
    assert_eq!(
        fs::read(f.generation.join("input.pending/main.tex")).unwrap(),
        b"own project"
    );
}

#[test]
fn partial_capture_failure_and_existing_publish_target_are_not_promoted() {
    let f = Fixture::new();
    fs::write(f.project.join("z.tex"), b"z").unwrap();
    *f.authority.0.lock().unwrap() = snapshot(
        &f.project,
        2,
        vec![
            HostInput::disk("main.tex", 1, b"own project".to_vec()).unwrap(),
            HostInput::disk("z.tex", 1, b"z".to_vec()).unwrap(),
        ],
    );
    fs::remove_file(f.project.join("z.tex")).unwrap();
    symlink(f.base.join("no-file"), f.project.join("z.tex")).unwrap();
    assert!(f.capture().is_err());
    assert!(!f.generation.join("input").exists());
    assert_eq!(
        fs::read(f.generation.join("input.pending/main.tex")).unwrap(),
        b"own project"
    );
    let g = Fixture::new();
    let result = CapturedInputs::capture_with_hook(
        &g.authority,
        &g.binding(),
        &g.generation,
        &AtomicBool::new(false),
        &mut |point| {
            if matches!(point, CapturePoint::BeforePublish) {
                private_dir(&g.generation.join("input"));
                fs::write(g.generation.join("input/foreign"), b"must remain").unwrap();
            }
        },
    );
    assert!(matches!(result, Err(GenerationError::Storage)));
    assert_eq!(
        fs::read(g.generation.join("input/foreign")).unwrap(),
        b"must remain"
    );
}

fn recipe(
    layout: &GenerationLayout,
    script: &str,
) -> Result<super::super::execution::HostExecutionConfig, GenerationError> {
    use super::super::execution::*;
    Ok(HostExecutionConfig {
        trusted: true,
        project_root: layout.input_root().into(),
        project_inputs: layout.inputs().to_vec(),
        read_only_roots: vec![],
        writable: WritableRoots {
            output: layout.output_root().into(),
            cache: layout.cache_root().into(),
            temporary: layout.temporary_root().into(),
        },
        command: HostCommand {
            executable: "/usr/bin/perl".into(),
            arguments: vec![
                "-e".into(),
                script.into(),
                layout.main_input().as_os_str().into(),
                layout.output_root().join("main.pdf").into_os_string(),
            ],
        },
        executable_allowlist: vec!["/usr/bin/perl".into()],
        environment: ExecutionEnvironment::Minimal,
        sandbox_executable: "/usr/bin/sandbox-exec".into(),
        timeout: std::time::Duration::from_secs(3),
    })
}
const COPY_INPUT: &str = "open(my $i,'<',$ARGV[0]) or die $!; binmode($i); local $/; my $s=<$i>; open(my $o,'>',$ARGV[1]) or die $!; binmode($o); print $o 'candidate '; print $o $s; close($o) or die $!;";
fn prepare(f: &Fixture) -> PreparedGeneration {
    f.capture()
        .unwrap()
        .prepare(&f.authority, &AtomicBool::new(false), |layout| {
            recipe(layout, COPY_INPUT)
        })
        .unwrap()
}
fn pending(f: &Fixture) -> PendingOutput {
    prepare(f)
        .run(&f.authority, &AtomicBool::new(false))
        .unwrap()
}

#[test]
fn compiler_reads_only_its_captured_bytes_and_current_result_is_host_locked() {
    let f = Fixture::new();
    let expected = f.binding();
    let pending = pending(&f);
    let data = pending
        .with_current_result(&f.authority, &AtomicBool::new(false), |artifact| {
            assert_eq!(artifact.binding(), &expected);
            assert!(
                f.authority.0.try_lock().is_err(),
                "publication must hold the real host lock"
            );
            artifact.bytes().to_vec()
        })
        .unwrap();
    assert_eq!(data, b"candidate own project");
    assert_eq!(
        fs::read(f.project.join("main.tex")).unwrap(),
        b"own project"
    );
}

#[test]
fn dirty_generation_actual_child_consumes_buffer_bytes_and_leaves_disk_unchanged() {
    let f = Fixture::new();
    *f.authority.0.lock().unwrap() = snapshot(
        &f.project,
        2,
        vec![
            HostInput::buffer(
                "main.tex",
                8,
                "\u{feff}Dirty ő\r\n".as_bytes().to_vec(),
                Digest::of(b"own project").unwrap(),
            )
            .unwrap(),
        ],
    );
    let data = pending(&f)
        .with_current_result(&f.authority, &AtomicBool::new(false), |artifact| {
            artifact.bytes().to_vec()
        })
        .unwrap();
    assert_eq!(data, "candidate \u{feff}Dirty ő\r\n".as_bytes());
    assert_eq!(
        fs::read(f.project.join("main.tex")).unwrap(),
        b"own project"
    );
}

#[test]
fn old_or_foreign_host_binding_cannot_accept_a_finished_output() {
    for foreign in [false, true] {
        let f = Fixture::new();
        let output = pending(&f);
        let workspace = if foreign { [21; 16] } else { [19; 16] };
        *f.authority.0.lock().unwrap() = HostSnapshot::new(
            workspace,
            if foreign { 1 } else { 2 },
            f.project.clone(),
            "main.tex".into(),
            vec![HostInput::disk("main.tex", 7, b"own project".to_vec()).unwrap()],
        )
        .unwrap();
        let published = AtomicBool::new(false);
        let result = output.with_current_result(&f.authority, &AtomicBool::new(false), |_| {
            published.store(true, Ordering::Release)
        });
        assert!(matches!(result, Err(GenerationError::StaleHost)));
        assert!(!published.load(Ordering::Acquire));
    }
}

#[test]
fn source_change_after_run_is_not_published_current_even_with_same_mtime() {
    let f = Fixture::new();
    let output = pending(&f);
    overwrite_same_metadata(&f.project.join("main.tex"), b"bad project");
    let published = AtomicBool::new(false);
    let result = output.with_current_result(&f.authority, &AtomicBool::new(false), |_| {
        published.store(true, Ordering::Release)
    });
    assert!(matches!(result, Err(GenerationError::SourceChanged)));
    assert!(!published.load(Ordering::Acquire));
}

#[test]
fn deterministic_prelaunch_change_is_rechecked_at_the_actual_spawn_gate() {
    let f = Fixture::new();
    let prepared = prepare(&f);
    let result = prepared.run_with_hook(&f.authority, &AtomicBool::new(false), &mut |point| {
        if matches!(point, GenerationPoint::PrelaunchValidated) {
            overwrite_same_metadata(&f.project.join("main.tex"), b"bad project");
        }
    });
    assert!(matches!(result, Err(GenerationError::SourceChanged)));
    assert!(!f.generation.join("output/main.pdf").exists());
}

#[test]
fn deterministic_final_source_change_prevents_publication() {
    let f = Fixture::new();
    let output = pending(&f);
    let published = AtomicBool::new(false);
    let result = output.with_current_result_and_hook(
        &f.authority,
        &AtomicBool::new(false),
        |_| published.store(true, Ordering::Release),
        &mut |point| {
            if matches!(point, GenerationPoint::FinalOutputRead) {
                overwrite_same_metadata(&f.project.join("main.tex"), b"bad project");
            }
        },
    );
    assert!(matches!(result, Err(GenerationError::SourceChanged)));
    assert!(!published.load(Ordering::Acquire));
}

#[test]
fn readonly_snapshot_mode_does_not_replace_a_final_content_identity_check() {
    let f = Fixture::new();
    let output = pending(&f);
    let published = AtomicBool::new(false);
    let result = output.with_current_result_and_hook(
        &f.authority,
        &AtomicBool::new(false),
        |_| published.store(true, Ordering::Release),
        &mut |point| {
            if matches!(point, GenerationPoint::FinalOutputRead) {
                let file = f.generation.join("input/main.tex");
                fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
                overwrite_same_metadata(&file, b"bad project");
                fs::set_permissions(file, fs::Permissions::from_mode(0o400)).unwrap();
            }
        },
    );
    assert!(matches!(result, Err(GenerationError::SnapshotChanged)));
    assert!(!published.load(Ordering::Acquire));
}

#[test]
fn a_previous_run_file_cannot_replace_the_finished_output() {
    let old = Fixture::new();
    let previous = pending(&old);
    previous
        .with_current_result(&old.authority, &AtomicBool::new(false), |_| ())
        .unwrap();
    let f = Fixture::new();
    let output = pending(&f);
    fs::remove_file(f.generation.join("output/main.pdf")).unwrap();
    fs::copy(
        old.generation.join("output/main.pdf"),
        f.generation.join("output/main.pdf"),
    )
    .unwrap();
    let published = AtomicBool::new(false);
    assert!(matches!(
        output.with_current_result(&f.authority, &AtomicBool::new(false), |_| published
            .store(true, Ordering::Release)),
        Err(GenerationError::OutputChanged)
    ));
    assert!(!published.load(Ordering::Acquire));
}

#[test]
fn cancellation_after_completion_and_partial_failed_build_never_publish_current() {
    let f = Fixture::new();
    let output = pending(&f);
    let published = AtomicBool::new(false);
    assert!(matches!(
        output.with_current_result(&f.authority, &AtomicBool::new(true), |_| published
            .store(true, Ordering::Release)),
        Err(GenerationError::Cancelled)
    ));
    assert!(!published.load(Ordering::Acquire));
    let g = Fixture::new();
    let prepared = g
        .capture()
        .unwrap()
        .prepare(&g.authority, &AtomicBool::new(false), |layout| {
            recipe(
                layout,
                "open(my $f,'>',$ARGV[1]) or die $!; print $f 'partial'; close($f); exit 7;",
            )
        })
        .unwrap();
    assert!(matches!(
        prepared.run(&g.authority, &AtomicBool::new(false)),
        Err(GenerationError::CompilerFailed(ExecutionState::Failed))
    ));
}

#[test]
fn revision_bytes_main_and_input_set_are_bound_even_if_host_counter_is_misused() {
    for change in 0..4 {
        for before_launch in [true, false] {
            let f = Fixture::new();
            let mut prepared = Some(prepare(&f));
            let completed = if before_launch {
                None
            } else {
                Some(
                    prepared
                        .take()
                        .unwrap()
                        .run(&f.authority, &AtomicBool::new(false))
                        .unwrap(),
                )
            };
            // A host must advance generation. Even this mistaken reuse cannot
            // conceal a changed revision/content/main/input manifest digest.
            let mut files = vec![
                HostInput::disk(
                    "main.tex",
                    if change == 0 { 8 } else { 7 },
                    b"own project".to_vec(),
                )
                .unwrap(),
            ];
            if change == 1 {
                files[0] = HostInput::buffer(
                    "main.tex",
                    7,
                    b"dirty buffer".to_vec(),
                    Digest::of(b"own project").unwrap(),
                )
                .unwrap();
            }
            if change >= 2 {
                fs::write(f.project.join("z.tex"), b"z").unwrap();
                files.push(HostInput::disk("z.tex", 1, b"z".to_vec()).unwrap());
            }
            let main = if change == 3 { "z.tex" } else { "main.tex" };
            *f.authority.0.lock().unwrap() =
                HostSnapshot::new([19; 16], 1, f.project.clone(), main.into(), files).unwrap();
            let published = AtomicBool::new(false);
            let result = if let Some(output) = completed {
                output.with_current_result(&f.authority, &AtomicBool::new(false), |_| {
                    published.store(true, Ordering::Release)
                })
            } else {
                prepared
                    .take()
                    .unwrap()
                    .run(&f.authority, &AtomicBool::new(false))
                    .map(|_| ())
            };
            assert!(matches!(result, Err(GenerationError::StaleHost)));
            assert!(!published.load(Ordering::Acquire));
            if before_launch {
                assert!(!f.generation.join("output/main.pdf").exists());
            }
        }
    }
}

fn wait_for_file(path: &Path) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while !path.exists() {
        assert!(
            std::time::Instant::now() < deadline,
            "own child marker missing: {}",
            path.display()
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}
fn paused_recipe(
    layout: &GenerationLayout,
    produce: bool,
) -> Result<super::super::execution::HostExecutionConfig, GenerationError> {
    let body = if produce { COPY_INPUT } else { "exit 0;" };
    let script = format!(
        "open(my $r,'>',$ARGV[2]) or die $!; print $r 'ready'; close($r) or die $!; while (!-e $ARGV[3]) {{select(undef,undef,undef,0.01);}} {body}"
    );
    let mut config = recipe(layout, &script)?;
    config.command.arguments.extend([
        layout.output_root().join("ready").into_os_string(),
        layout.output_root().join("go").into_os_string(),
    ]);
    Ok(config)
}

#[test]
fn edits_continue_while_the_child_compiles_but_old_generation_result_is_refused() {
    for host_edit in [false, true] {
        let f = Fixture::new();
        let prepared = f
            .capture()
            .unwrap()
            .prepare(&f.authority, &AtomicBool::new(false), |layout| {
                paused_recipe(layout, true)
            })
            .unwrap();
        let cancel = AtomicBool::new(false);
        let output = std::thread::scope(|scope| {
            let auth = &f.authority;
            let flag = &cancel;
            let worker = scope.spawn(move || prepared.run(auth, flag));
            wait_for_file(&f.generation.join("output/ready"));
            assert!(
                f.authority.0.try_lock().is_ok(),
                "host lease must release after spawn"
            );
            if host_edit {
                *f.authority.0.lock().unwrap() = snapshot(
                    &f.project,
                    2,
                    vec![
                        HostInput::buffer(
                            "main.tex",
                            8,
                            b"new dirty buffer".to_vec(),
                            Digest::of(b"own project").unwrap(),
                        )
                        .unwrap(),
                    ],
                );
            } else {
                overwrite_same_metadata(&f.project.join("main.tex"), b"bad project");
            }
            fs::write(f.generation.join("output/go"), b"go").unwrap();
            worker.join().unwrap().unwrap()
        });
        assert_eq!(
            fs::read(f.generation.join("output/main.pdf")).unwrap(),
            b"candidate own project"
        );
        let published = AtomicBool::new(false);
        let result = output.with_current_result(&f.authority, &cancel, |_| {
            published.store(true, Ordering::Release)
        });
        if host_edit {
            assert!(matches!(result, Err(GenerationError::StaleHost)));
        } else {
            assert!(matches!(result, Err(GenerationError::SourceChanged)));
        }
        assert!(!published.load(Ordering::Acquire));
    }
}

#[test]
fn a_foreign_output_inode_injected_during_compilation_cannot_become_current() {
    let f = Fixture::new();
    let previous = f.base.join("previous.pdf");
    fs::write(&previous, b"foreign previous-run bytes").unwrap();
    let prepared = f
        .capture()
        .unwrap()
        .prepare(&f.authority, &AtomicBool::new(false), |layout| {
            paused_recipe(layout, false)
        })
        .unwrap();
    let cancel = AtomicBool::new(false);
    let result = std::thread::scope(|scope| {
        let auth = &f.authority;
        let flag = &cancel;
        let worker = scope.spawn(move || prepared.run(auth, flag));
        wait_for_file(&f.generation.join("output/ready"));
        fs::rename(&previous, f.generation.join("output/main.pdf")).unwrap();
        fs::write(f.generation.join("output/go"), b"go").unwrap();
        worker.join().unwrap()
    });
    let published = AtomicBool::new(false);
    let accepted = match result {
        Ok(output) => output
            .with_current_result(&f.authority, &cancel, |artifact| {
                eprintln!("FOREIGN_OUTPUT_ACCEPTED={:?}", artifact.bytes());
                published.store(true, Ordering::Release);
            })
            .is_ok(),
        Err(GenerationError::OutputChanged) => false,
        Err(error) => panic!("unexpected result {error:?}"),
    };
    assert!(
        !accepted,
        "foreign prior-run inode was accepted after process success"
    );
    assert!(!published.load(Ordering::Acquire));
}

#[test]
fn snapshot_file_must_be_readonly_even_when_its_content_digest_matches() {
    let f = Fixture::new();
    let result = CapturedInputs::capture_with_hook(
        &f.authority,
        &f.binding(),
        &f.generation,
        &AtomicBool::new(false),
        &mut |point| {
            if matches!(point, CapturePoint::BeforePublish) {
                fs::set_permissions(
                    f.generation.join("input.pending/main.tex"),
                    fs::Permissions::from_mode(0o600),
                )
                .unwrap();
            }
        },
    );
    assert!(matches!(result, Err(GenerationError::SnapshotChanged)));
}

#[test]
fn original_and_snapshot_symlink_hardlink_inode_replacement_fail_before_acceptance() {
    for original in [false, true] {
        for attack in 0..3 {
            let f = Fixture::new();
            let output = pending(&f);
            let directory = if original {
                f.project.clone()
            } else {
                f.generation.join("input")
            };
            if !original {
                fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
            }
            let file = directory.join("main.tex");
            match attack {
                0 => {
                    fs::rename(&file, f.base.join("old.tex")).unwrap();
                    fs::write(&file, b"own project").unwrap();
                }
                1 => {
                    fs::rename(&file, f.base.join("old.tex")).unwrap();
                    symlink(f.base.join("old.tex"), &file).unwrap();
                }
                _ => {
                    fs::hard_link(&file, f.base.join("hardlink.tex")).unwrap();
                }
            }
            if !original {
                fs::set_permissions(&directory, fs::Permissions::from_mode(0o500)).unwrap();
            }
            assert_eq!(fs::read(&file).unwrap(), b"own project");
            let published = AtomicBool::new(false);
            let result = output.with_current_result(&f.authority, &AtomicBool::new(false), |_| {
                published.store(true, Ordering::Release)
            });
            if original {
                assert!(matches!(result, Err(GenerationError::SourceChanged)));
            } else {
                assert!(matches!(result, Err(GenerationError::SnapshotChanged)));
            }
            assert!(!published.load(Ordering::Acquire));
        }
    }
}

#[test]
fn prepopulated_output_reused_storage_and_foreign_host_configuration_are_rejected() {
    let f = Fixture::new();
    let _first = f.capture().unwrap();
    assert!(matches!(f.capture(), Err(GenerationError::Storage)));
    let g = Fixture::new();
    let prepared = prepare(&g);
    fs::write(g.generation.join("output/main.pdf"), b"previous result").unwrap();
    assert!(prepared.run(&g.authority, &AtomicBool::new(false)).is_err());
    for change in 0..4 {
        let h = Fixture::new();
        let captured = h.capture().unwrap();
        let result = captured.prepare(&h.authority, &AtomicBool::new(false), |layout| {
            let mut config = recipe(layout, COPY_INPUT)?;
            match change {
                0 => config.project_root = h.project.clone(),
                1 => config.project_inputs.push(h.project.join("main.tex")),
                2 => config.writable.output = h.project.clone(),
                _ => config.read_only_roots.push(h.project.clone()),
            }
            Ok(config)
        });
        assert!(matches!(result, Err(GenerationError::Configuration)));
    }
}

#[test]
fn late_cancel_and_same_inode_output_change_cannot_invoke_publication() {
    let f = Fixture::new();
    let prepared = prepare(&f);
    let cancel = AtomicBool::new(false);
    let result = prepared.run_with_hook(&f.authority, &cancel, &mut |point| {
        if matches!(point, GenerationPoint::PrelaunchValidated) {
            cancel.store(true, Ordering::Release)
        }
    });
    assert!(matches!(result, Err(GenerationError::Cancelled)));
    assert!(!f.generation.join("output/main.pdf").exists());
    let g = Fixture::new();
    let output = pending(&g);
    let published = AtomicBool::new(false);
    let result = output.with_current_result_and_hook(
        &g.authority,
        &AtomicBool::new(false),
        |_| published.store(true, Ordering::Release),
        &mut |point| {
            if matches!(point, GenerationPoint::FinalOutputRead) {
                overwrite_same_metadata(
                    &g.generation.join("output/main.pdf"),
                    b"candidate bad project",
                );
            }
        },
    );
    assert!(matches!(result, Err(GenerationError::OutputChanged)));
    assert!(!published.load(Ordering::Acquire));
}

#[test]
fn successful_exit_without_output_bytes_and_timeout_do_not_produce_a_current_token() {
    for (script, timeout, want) in [
        ("exit 0;", 3000, GenerationError::OutputChanged),
        (
            "sleep 2;",
            80,
            GenerationError::CompilerFailed(ExecutionState::TimedOut),
        ),
    ] {
        let f = Fixture::new();
        let prepared = f
            .capture()
            .unwrap()
            .prepare(&f.authority, &AtomicBool::new(false), |layout| {
                let mut config = recipe(layout, script)?;
                config.timeout = std::time::Duration::from_millis(timeout);
                Ok(config)
            })
            .unwrap();
        let result = prepared.run(&f.authority, &AtomicBool::new(false));
        assert!(matches!(result,Err(ref error) if error==&want));
    }
}

#[test]
fn contended_host_authority_refuses_without_waiting_or_creating_storage() {
    let f = Fixture::new();
    let expected = f.binding();
    let held = f.authority.0.lock().unwrap();
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    let (result_tx, result_rx) = std::sync::mpsc::channel();
    let observed = std::thread::scope(|scope| {
        let auth = &f.authority;
        let directory = &f.generation;
        let worker = scope.spawn(move || {
            ready_tx.send(()).unwrap();
            let result =
                CapturedInputs::capture(auth, &expected, directory, &AtomicBool::new(false))
                    .map(|_| ());
            result_tx.send(result).unwrap();
        });
        ready_rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        let answer = result_rx.recv_timeout(std::time::Duration::from_millis(500));
        drop(held); // Always release so a broken blocking provider can be joined.
        worker.join().unwrap();
        answer
    });
    assert!(
        matches!(observed, Ok(Err(GenerationError::HostUnavailable))),
        "host lease acquisition waited instead of refusing: {observed:?}"
    );
    assert!(!f.generation.exists());
}

#[test]
fn generation_storage_cannot_enter_the_original_project_through_an_os_case_alias() {
    for spelling in [
        "ÁRVÍZTŰRŐ PROJEKT",
        "A\u{301}rvi\u{301}ztu\u{30b}ro\u{30b} projekt",
    ] {
        let f = Fixture::new();
        let alias = f.base.join(spelling);
        assert_eq!(
            fs::metadata(&alias).unwrap().ino(),
            fs::metadata(&f.project).unwrap().ino(),
            "this test requires the observed case-equivalent own volume route"
        );
        let wrong = alias.join("must-not-create");
        let result =
            CapturedInputs::capture(&f.authority, &f.binding(), &wrong, &AtomicBool::new(false));
        assert!(
            matches!(result, Err(GenerationError::InvalidInput)),
            "physical project alias was accepted as separate generation storage"
        );
        assert!(!wrong.exists());
    }
}

#[test]
fn read_only_grants_cannot_alias_original_project_by_case_or_unicode() {
    for spelling in [
        "ÁRVÍZTŰRŐ PROJEKT",
        "A\u{301}rvi\u{301}ztu\u{30b}ro\u{30b} projekt",
    ] {
        for child in [false, true] {
            let f = Fixture::new();
            private_dir(&f.project.join("assets"));
            let mut alias = f.base.join(spelling);
            assert_eq!(
                fs::metadata(&alias).unwrap().ino(),
                fs::metadata(&f.project).unwrap().ino()
            );
            if child {
                alias = alias.join("assets");
            }
            let captured = f.capture().unwrap();
            let result = captured.prepare(&f.authority, &AtomicBool::new(false), |layout| {
                let mut value = recipe(layout, COPY_INPUT)?;
                value.read_only_roots.push(alias.clone());
                Ok(value)
            });
            assert!(
                matches!(result, Err(GenerationError::Configuration)),
                "physical project alias became a read grant"
            );
        }
    }
}
