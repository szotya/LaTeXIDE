#![cfg(target_os = "macos")]
use crate::recovery::durable::private_directory::{checked_open, create_child};
use crate::{Workspace, recovery::durable};
use std::{
    ffi::OsStr,
    fs::{self, DirBuilder, File},
    io::Read,
    os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt, symlink},
    path::PathBuf,
};

fn fixture() -> (PathBuf, PathBuf, PathBuf) {
    let mut nonce = [0; 16];
    File::open("/dev/urandom")
        .unwrap()
        .read_exact(&mut nonce)
        .unwrap();
    let nonce: String = nonce.iter().map(|b| format!("{b:02x}")).collect();
    let root = std::env::temp_dir()
        .canonicalize()
        .unwrap()
        .join(format!("ancestor-{}-{nonce}", std::process::id()));
    DirBuilder::new().mode(0o700).create(&root).unwrap();
    let project = root.join("project");
    DirBuilder::new().mode(0o755).create(&project).unwrap();
    fs::set_permissions(&project, fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(project.join("main.tex"), b"abc").unwrap();
    let outside = root.join("outside");
    DirBuilder::new().mode(0o700).create(&outside).unwrap();
    fs::write(outside.join("sentinel.txt"), &nonce).unwrap();
    fs::write(root.join("nonce.txt"), &nonce).unwrap();
    println!("P01_ANCESTOR_FIXTURE root={} nonce={nonce}", root.display());
    (root, project, outside)
}
fn unchanged(outside: &std::path::Path) {
    let names: Vec<_> = fs::read_dir(outside)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names, [OsStr::new("sentinel.txt")]);
    assert!(!fs::read(outside.join("sentinel.txt")).unwrap().is_empty());
    assert_eq!(fs::metadata(outside).unwrap().mode() & 0o777, 0o700);
}

#[test]
fn ancestor_parent_symlink_is_rejected_before_any_outside_creation() {
    let (_, project, outside) = fixture();
    symlink(&outside, project.join(".latexide")).unwrap();
    assert!(
        Workspace::open_recoverable(project.clone(), project.join(".latexide/recovery")).is_err()
    );
    unchanged(&outside);
}
#[test]
fn ancestor_nested_missing_components_do_not_follow_an_existing_link() {
    let (root, project, outside) = fixture();
    fs::create_dir(root.join("one")).unwrap();
    symlink(&outside, root.join("one/two")).unwrap();
    assert!(
        Workspace::open_recoverable(project, root.join("one/two/three/four/recovery")).is_err()
    );
    unchanged(&outside);
}
#[test]
fn ancestor_leaf_symlink_is_rejected_without_chmod_or_lock() {
    let (root, project, outside) = fixture();
    symlink(&outside, root.join("recovery")).unwrap();
    assert!(Workspace::open_recoverable(project, root.join("recovery")).is_err());
    unchanged(&outside);
}
#[test]
fn ancestor_normal_new_nested_and_existing_private_roots_preserve_public_project_mode() {
    let (root, project, outside) = fixture();
    let storage = root.join("new/inner/recovery");
    for _ in 0..2 {
        let mut w = Workspace::open_recoverable(project.clone(), storage.clone()).unwrap();
        assert_eq!(w.open_document("main.tex").unwrap().text, "abc");
        drop(w);
        assert_eq!(fs::metadata(&project).unwrap().mode() & 0o777, 0o755);
        for p in [root.join("new"), root.join("new/inner"), storage.clone()] {
            let m = fs::metadata(p).unwrap();
            assert_eq!(m.mode() & 0o777, 0o700);
            assert_eq!(m.uid(), fs::metadata(&root).unwrap().uid());
        }
    }
    unchanged(&outside);
}
#[test]
fn ancestor_existing_permissive_root_is_rejected_without_repair() {
    let (root, project, outside) = fixture();
    let storage = root.join("recovery");
    fs::create_dir(&storage).unwrap();
    fs::set_permissions(&storage, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(Workspace::open_recoverable(project, storage.clone()).is_err());
    assert_eq!(fs::metadata(&storage).unwrap().mode() & 0o777, 0o755);
    assert_eq!(fs::read_dir(storage).unwrap().count(), 0);
    unchanged(&outside);
}
#[test]
fn ancestor_regular_file_parent_is_rejected_without_write() {
    let (root, project, outside) = fixture();
    let file = root.join("parent-file");
    fs::write(&file, b"sentinel").unwrap();
    assert!(Workspace::open_recoverable(project, file.join("recovery")).is_err());
    assert_eq!(fs::read(file).unwrap(), b"sentinel");
    unchanged(&outside);
}
#[test]
fn ancestor_final_open_rejects_a_parent_link_swap_after_metadata_validation() {
    let (root, _, outside) = fixture();
    let path = root.join("checked");
    fs::create_dir(&path).unwrap();
    let before = fs::symlink_metadata(&path).unwrap();
    fs::rename(&path, root.join("moved")).unwrap();
    symlink(&outside, &path).unwrap();
    assert!(checked_open(&path, &before).is_err());
    assert!(durable::create_dir(&path.join("child")).is_err());
    unchanged(&outside);
}
#[test]
fn ancestor_relative_creation_stays_with_held_parent_after_path_swap() {
    let (root, _, outside) = fixture();
    let path = root.join("checked");
    fs::create_dir(&path).unwrap();
    let held = checked_open(&path, &fs::symlink_metadata(&path).unwrap()).unwrap();
    let moved = root.join("moved");
    fs::rename(&path, &moved).unwrap();
    symlink(&outside, &path).unwrap();
    let created = create_child(&held, OsStr::new("child")).unwrap();
    assert_eq!(
        created.metadata().unwrap().ino(),
        fs::metadata(moved.join("child")).unwrap().ino()
    );
    unchanged(&outside);
}
#[test]
fn ancestor_leaf_swap_after_parent_open_cannot_redirect_relative_creation() {
    let (root, _, outside) = fixture();
    let held = checked_open(&root, &fs::symlink_metadata(&root).unwrap()).unwrap();
    symlink(&outside, root.join("child")).unwrap();
    assert!(create_child(&held, OsStr::new("child")).is_err());
    unchanged(&outside);
}
