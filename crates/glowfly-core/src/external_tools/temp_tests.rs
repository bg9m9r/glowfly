//! Session folders for tool copies: fresh folders, read-only copies, removal and the sweep, the
//! names copies get, and (on Unix) refusing a root other users could open.

use super::*;

/// A fresh scratch root in the temporary folder.
fn scratch(name: &str) -> PathBuf {
    let root = crate::paths::temp_dir().join(format!("trellis-test-tooltemp-{name}-{}", std::process::id()));
    let _ = remove_tree(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

#[test]
fn each_session_gets_a_new_folder() {
    let root = scratch("sessions");
    let a = Session::create(&root).unwrap();
    let b = Session::create(&root).unwrap();
    assert_ne!(a.dir(), b.dir());
    assert!(a.dir().is_dir() && b.dir().is_dir());
    assert_eq!(a.dir().parent(), Some(root.as_path()));
    let _ = remove_tree(&root);
}

#[test]
fn copies_are_read_only_under_their_label_and_removed_with_the_session() {
    let root = scratch("write");
    let s = Session::create(&root).unwrap();
    let a = s.write("ours", "src/main.rs", b"fn main() {}\n").unwrap();
    assert_eq!(a, s.dir().join("ours").join("main.rs"));
    assert_eq!(fs::read(&a).unwrap(), b"fn main() {}\n");
    assert!(fs::metadata(&a).unwrap().permissions().readonly());
    // The same label again gets a folder of its own.
    let b = s.write("ours", "main.rs", b"x").unwrap();
    assert_eq!(b, s.dir().join("ours-2").join("main.rs"));
    let c = s.write("ours", "main.rs", b"y").unwrap();
    assert_eq!(c, s.dir().join("ours-3").join("main.rs"));
    let dir = s.dir().to_owned();
    s.remove().unwrap();
    assert!(!dir.exists(), "read-only copies are removed too");
    let _ = remove_tree(&root);
}

#[test]
fn sweep_removes_only_old_session_folders() {
    let root = scratch("sweep");
    let old = root.join("old");
    fs::create_dir_all(old.join("base")).unwrap();
    let file = root.join("note.txt");
    fs::write(&file, "x").unwrap();
    sweep(&root, SWEEP_AGE, SystemTime::now());
    assert!(old.is_dir(), "a new folder stays");
    // Seen from two days on, the folder is old.
    sweep(&root, SWEEP_AGE, SystemTime::now() + 2 * SWEEP_AGE);
    assert!(!old.exists(), "an old folder goes");
    assert!(file.is_file(), "files are not sessions");
    let new = root.join("new");
    fs::create_dir_all(&new).unwrap();
    sweep(&root, SWEEP_AGE, SystemTime::now() + SWEEP_AGE / 2);
    assert!(new.is_dir(), "younger than the age");
    let _ = remove_tree(&root);
}

#[test]
fn copy_names_are_portable_and_keep_the_extension() {
    assert_eq!(file_name_for("src/aux.c"), "aux-file.c");
    assert_eq!(file_name_for("a/b/notes.txt"), "notes.txt");
    assert_eq!(file_name_for(r"dir/a\b.txt"), "a-b.txt", "one name, never a folder");
    assert_eq!(file_name_for("what?.md"), "what-.md");
    assert_eq!(file_name_for("..."), "file");
    assert_eq!(file_name_for(""), "file");
}

#[test]
fn the_root_is_a_private_folder() {
    let base = scratch("root").join("tools");
    assert_eq!(private_root(&base).unwrap(), base);
    assert!(base.is_dir());
    assert_eq!(private_root(&base).unwrap(), base, "an existing folder is fine");
    let file = base.join("plain");
    fs::write(&file, "").unwrap();
    assert!(private_root(&file).is_err(), "a file is not a folder");
    assert!(root().is_ok());
    assert_eq!(root_in(&file, Some(&base)).unwrap(), base, "the fallback when the first is refused");
    let _ = remove_tree(base.parent().unwrap());
}

#[cfg(unix)]
#[test]
fn a_shared_or_linked_root_is_refused() {
    use std::os::unix::fs::PermissionsExt;
    let scratch = scratch("unix-root");
    let open = scratch.join("open");
    fs::create_dir_all(&open).unwrap();
    fs::set_permissions(&open, fs::Permissions::from_mode(0o777)).unwrap();
    assert!(private_root(&open).is_err(), "other users could read the copies");
    let private = scratch.join("private");
    private_root(&private).unwrap();
    assert_eq!(fs::metadata(&private).unwrap().permissions().mode() & 0o777, 0o700);
    let link = scratch.join("link");
    std::os::unix::fs::symlink(&private, &link).unwrap();
    assert!(private_root(&link).is_err(), "a link could point anywhere");
    // The fallback takes over.
    let fallback = scratch.join("cache").join("tool-files");
    assert_eq!(root_in(&open, Some(&fallback)).unwrap(), fallback);
    assert_eq!(root_in(&link, Some(&fallback)).unwrap(), fallback);
    assert!(root_in(&open, None).is_err());
    let _ = remove_tree(&scratch);
}
