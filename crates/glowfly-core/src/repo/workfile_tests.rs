//! Creating, renaming and deleting working-tree files against real repositories: the path
//! checks, what ends up on disk and in status, the index left alone, and every refusal changing
//! nothing.

use super::*;
use crate::error::Error;
use crate::gitcli::{args, run};
use crate::testutil::*;
use git2::{Oid, Repository};
use std::path::Path;

pub(super) fn commit(dir: &Path, files: &[(&str, &str)], msg: &str) {
    for (f, t) in files {
        if let Some(parent) = dir.join(f).parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        write(dir, f, t);
    }
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", msg]);
}

pub(super) fn status(dir: &Path) -> String {
    run(dir, &args(&["status", "--porcelain"])).1
}

/// The index's stage-0 blob id for `path`.
pub(super) fn index_id(dir: &Path, path: &str) -> Option<Oid> {
    Some(Repository::open(dir).unwrap().index().unwrap().get_path(Path::new(path), 0)?.id)
}

pub(super) fn text(dir: &Path, path: &str) -> String {
    std::fs::read_to_string(dir.join(path)).unwrap()
}

pub(super) fn names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    v.sort();
    v
}

#[test]
fn check_new_path_normalizes_and_refuses() {
    use PathProblem::*;
    assert_eq!(check_new_path("docs/notes.md"), Ok("docs/notes.md".into()));
    assert_eq!(check_new_path(" a//b/./c.txt "), Ok("a/b/c.txt".into()));
    assert_eq!(check_new_path("a\\b.txt"), Ok("a/b.txt".into()));
    for t in ["", "  ", "./"] {
        assert_eq!(check_new_path(t), Err(Empty), "{t:?}");
    }
    for t in ["/etc/x", "C:/x", "//server/x", "c:\\x", "\\\\server\\x"] {
        assert_eq!(check_new_path(t), Err(Absolute), "{t:?}");
    }
    for t in ["../x", "a/../../x", "a/.."] {
        assert_eq!(check_new_path(t), Err(OutsideRepo), "{t:?}");
    }
    for t in
        [".git/config", "sub/.GIT/hooks/x", ".git./x", "GIT~1/x", ".git", ".git /x", ".g\u{200C}it/x", "\u{FEFF}.GIT/hooks/post-checkout"]
    {
        assert_eq!(check_new_path(t), Err(InsideGitDir), "{t:?}");
    }
    assert_eq!(check_new_path("dir/"), Err(Folder));
    // Names Windows can't use: refused there, allowed with a warning elsewhere.
    for (t, bad) in [
        ("a:b", "a:b"),
        ("what?.txt", "what?.txt"),
        ("nul", "nul"),
        ("Com1.txt", "Com1.txt"),
        ("conin$", "conin$"),
        ("COM\u{b9}.txt", "COM\u{b9}.txt"),
        ("x/trailing.", "trailing."),
        ("tab\tname", "tab\tname"),
        ("lpt9", "lpt9"),
        ("a/quote\"", "quote\""),
        ("aux/x.c", "aux"),
    ] {
        let why = crate::names::win_name_problem(bad).unwrap();
        assert_eq!(check_new_path_for(t, true), Err(BadName(bad.into(), why.clone())), "{t:?}");
        assert!(check_new_path_for(t, false).is_ok(), "{t:?}");
        assert_eq!(new_path_warning_for(&check_new_path_for(t, false).unwrap(), false), Some(why.warning(bad)), "{t:?}");
        assert_eq!(new_path_warning_for(t, true), None);
        assert_eq!(check_new_path(t).is_err(), cfg!(windows), "{t:?}");
    }
    for t in [".gitignore", "my.git/x", "com10.txt", "console.log", "héllo.txt", "abé"] {
        assert!(check_new_path_for(t, true).is_ok(), "{t:?}");
        assert_eq!(new_path_warning_for(t, false), None, "{t:?}");
    }
    assert_eq!(Empty.message(), "Enter a file name.");
    assert_eq!(Absolute.message(), "Enter a path inside the repository.");
    assert_eq!(OutsideRepo.message(), "That path leads outside the repository.");
    assert_eq!(InsideGitDir.message(), "Files can't be created inside the .git folder.");
    assert_eq!(Folder.message(), "Enter a file name, not a folder.");
    let colon = crate::names::WinNameProblem::Char(':');
    assert_eq!(BadName("a:b".into(), colon).message(), "a:b can't be used: Windows doesn't allow : in names.");
}

#[test]
fn creates_in_new_nested_folder() {
    let dir = scratch_repo("wf-create");
    commit(&dir, &[("a.txt", "a\n")], "one");
    let before = index_id(&dir, "a.txt");
    assert!(!create_file(&dir, "deep/er/new.txt").unwrap().ignored);
    assert_eq!(text(&dir, "deep/er/new.txt"), "");
    assert_eq!(status(&dir), "?? deep/");
    assert_eq!(index_id(&dir, "a.txt"), before);
    assert_eq!(index_id(&dir, "deep/er/new.txt"), None, "nothing staged");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn create_refuses_existing_paths() {
    let dir = scratch_repo("wf-create-exists");
    commit(&dir, &[("a.txt", "kept\n"), (".gitignore", "secret.env\n"), ("sub/x.txt", "x\n")], "one");
    write(&dir, "secret.env", "TOKEN\n");
    for (path, contents) in [("a.txt", "kept\n"), ("secret.env", "TOKEN\n")] {
        let e = create_file(&dir, path).unwrap_err();
        assert!(e.to_string().contains("already exists"), "{e}");
        assert_eq!(text(&dir, path), contents);
    }
    if cfg!(any(windows, target_os = "macos")) {
        // The same file under another case on a case-insensitive file system.
        let e = create_file(&dir, "A.TXT").unwrap_err();
        assert!(e.to_string().contains("already exists"), "{e}");
    }
    let e = create_file(&dir, "sub").unwrap_err();
    assert!(e.to_string().contains("already exists") && e.to_string().contains("folder"), "{e}");
    assert_eq!(status(&dir), "");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn create_in_ignored_folder_reports_ignored() {
    let dir = scratch_repo("wf-create-ignored");
    commit(&dir, &[(".gitignore", "build/\n")], "ignore");
    assert!(create_file(&dir, "build/out.log").unwrap().ignored);
    assert!(dir.join("build/out.log").is_file());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn create_under_a_file_creates_nothing() {
    let dir = scratch_repo("wf-create-under-file");
    commit(&dir, &[("a.txt", "a\n")], "one");
    let e = create_file(&dir, "a.txt/new.txt").unwrap_err();
    assert!(e.to_string().contains("a.txt is a file"), "{e}");
    assert_eq!(text(&dir, "a.txt"), "a\n");
    assert_eq!(status(&dir), "");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn create_refuses_escapes() {
    let dir = scratch_repo("wf-create-escape");
    commit(&dir, &[("a.txt", "a\n")], "one");
    assert!(create_file(&dir, "../escape.txt").is_err());
    assert!(!dir.parent().unwrap().join("escape.txt").exists());
    assert!(create_file(&dir, ".git/evil").is_err());
    assert!(!dir.join(".git/evil").exists());
    assert!(create_file(&dir, "GIT~1/evil").is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Links `link` (inside `dir`) to the folder `target`; false where the system doesn't allow it.
fn symlink_dir(target: &Path, link: &Path) -> bool {
    #[cfg(unix)]
    return std::os::unix::fs::symlink(target, link).is_ok();
    #[cfg(windows)]
    return std::os::windows::fs::symlink_dir(target, link).is_ok();
}

#[test]
fn refuses_to_go_through_a_symlinked_folder() {
    let dir = scratch_repo("wf-symlink");
    commit(&dir, &[("a.txt", "a\n")], "one");
    let outside = dir.with_file_name(format!("{}-outside", dir.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&outside);
    std::fs::create_dir_all(&outside).unwrap();
    write(&outside, "victim.txt", "v\n");
    if symlink_dir(&outside, &dir.join("link")) {
        let e = create_file(&dir, "link/new.txt").unwrap_err();
        assert!(matches!(e, Error::Unsupported(_)), "{e}");
        assert!(rename_file(&dir, "a.txt", "link/a.txt").is_err());
        assert!(rename_file(&dir, "link/victim.txt", "b.txt").is_err());
        assert!(delete_file(&dir, "link/victim.txt", Removal::Permanent).is_err());
        assert_eq!(names(&outside), ["victim.txt"]);
        assert_eq!(text(&dir, "a.txt"), "a\n");
        // The link itself can be deleted; its target stays.
        assert_eq!(delete_file(&dir, "link", Removal::Permanent).unwrap(), Deleted::Permanently);
        assert!(dir.join("link").symlink_metadata().is_err());
        assert_eq!(names(&outside), ["victim.txt"]);
    }
    let _ = std::fs::remove_dir_all(&outside);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn renames_untracked_into_new_folder() {
    let dir = scratch_repo("wf-rename-untracked");
    commit(&dir, &[("a.txt", "a\n")], "one");
    std::fs::create_dir_all(dir.join("old/sub")).unwrap();
    write(&dir, "old/sub/n.txt", "note\n");
    assert!(!rename_file(&dir, "old/sub/n.txt", "new/place/n.txt").unwrap().ignored);
    assert_eq!(text(&dir, "new/place/n.txt"), "note\n");
    assert!(!dir.join("old").exists(), "the emptied folders are removed");
    assert_eq!(status(&dir), "?? new/");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rename_tracked_leaves_index() {
    let dir = scratch_repo("wf-rename-tracked");
    commit(&dir, &[("old.txt", "x\n")], "one");
    let before = index_id(&dir, "old.txt");
    assert!(!rename_file(&dir, "old.txt", "new.txt").unwrap().ignored);
    assert_eq!(status(&dir), " D old.txt\n?? new.txt");
    assert_eq!(index_id(&dir, "old.txt"), before);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rename_refusals_change_nothing() {
    let dir = scratch_repo("wf-rename-refuse");
    commit(&dir, &[("a.txt", "a\n"), ("b.txt", "b\n"), ("d/x.txt", "x\n")], "one");
    let e = rename_file(&dir, "a.txt", "b.txt").unwrap_err();
    assert!(e.to_string().contains("b.txt already exists"), "{e}");
    assert_eq!((text(&dir, "a.txt"), text(&dir, "b.txt")), ("a\n".into(), "b\n".into()));
    assert!(rename_file(&dir, "ghost.txt", "g.txt").unwrap_err().is_not_found());
    assert!(matches!(rename_file(&dir, "d", "e"), Err(Error::Unsupported(_))));
    assert!(rename_file(&dir, "a.txt", "./a.txt").unwrap_err().to_string().contains("same"));
    assert!(rename_file(&dir, "a.txt", ".git/a.txt").is_err());
    assert!(rename_file(&dir, "a.txt", "d").is_err(), "a folder is in the way");
    assert_eq!(status(&dir), "");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A merge leaves f.txt conflicted.
pub(super) fn conflicted_repo(name: &str) -> std::path::PathBuf {
    let dir = scratch_repo(name);
    commit(&dir, &[("f.txt", "base\n")], "base");
    git(&dir, &["checkout", "-q", "-b", "side"]);
    commit(&dir, &[("f.txt", "side\n")], "side");
    git(&dir, &["checkout", "-q", "main"]);
    commit(&dir, &[("f.txt", "main\n")], "main");
    assert!(!run(&dir, &args(&["merge", "side"])).0, "the merge conflicts");
    dir
}

#[test]
fn conflicted_paths_are_refused() {
    let dir = conflicted_repo("wf-conflict");
    let before = text(&dir, "f.txt");
    assert!(rename_file(&dir, "f.txt", "g.txt").unwrap_err().to_string().contains("conflicts"));
    assert!(delete_file(&dir, "f.txt", Removal::Permanent).unwrap_err().to_string().contains("conflicts"));
    assert_eq!(text(&dir, "f.txt"), before);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rename_case_only() {
    let dir = scratch_repo("wf-rename-case");
    commit(&dir, &[("readme.md", "hi\n")], "one");
    rename_file(&dir, "readme.md", "README.md").unwrap();
    let listed = names(&dir);
    assert!(listed.contains(&"README.md".to_owned()) && !listed.contains(&"readme.md".to_owned()), "{listed:?}");
    assert_eq!(text(&dir, "README.md"), "hi\n");
    assert!(!listed.iter().any(|n| n.contains("glowfly-rename")), "{listed:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn deletes_and_prunes_empty_folders() {
    let dir = scratch_repo("wf-delete");
    commit(&dir, &[("a.txt", "a\n"), ("keep/k.txt", "k\n")], "one");
    std::fs::create_dir_all(dir.join("notes/deep")).unwrap();
    write(&dir, "notes/deep/n.txt", "n\n");
    std::fs::create_dir_all(dir.join("keep/deep")).unwrap();
    write(&dir, "keep/deep/u.txt", "u\n");
    assert_eq!(delete_file(&dir, "notes/deep/n.txt", Removal::Permanent).unwrap(), Deleted::Permanently);
    assert!(!dir.join("notes").exists());
    assert_eq!(delete_file(&dir, "keep/deep/u.txt", Removal::Permanent).unwrap(), Deleted::Permanently);
    assert!(!dir.join("keep/deep").exists());
    assert!(dir.join("keep/k.txt").exists(), "a non-empty folder stays");
    assert!(dir.exists());

    let before = index_id(&dir, "a.txt");
    assert_eq!(delete_file(&dir, "a.txt", Removal::Permanent).unwrap(), Deleted::Permanently);
    assert_eq!(status(&dir), " D a.txt");
    assert_eq!(index_id(&dir, "a.txt"), before);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn delete_refusals() {
    let dir = scratch_repo("wf-delete-refuse");
    commit(&dir, &[("d/x.txt", "x\n")], "one");
    assert!(matches!(delete_file(&dir, "d", Removal::Permanent), Err(Error::Unsupported(_))));
    assert!(delete_file(&dir, "gone.txt", Removal::Permanent).unwrap_err().is_not_found());
    assert!(delete_file(&dir, "../x", Removal::Permanent).is_err());
    assert!(delete_file(&dir, ".git/config", Removal::Permanent).is_err());
    assert_eq!(text(&dir, "d/x.txt"), "x\n");
    assert!(dir.join(".git/config").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn deletes_an_oddly_named_file() {
    // Names check_new_path refuses can still exist (made elsewhere) and be deleted.
    let dir = scratch_repo("wf-delete-odd");
    write(&dir, "ends with dot.", "x\n");
    if cfg!(windows) {
        // Windows saved it as "ends with dot", so that name would reach another file: refused.
        assert!(delete_file(&dir, "ends with dot.", Removal::Permanent).is_err());
        assert_eq!(text(&dir, "ends with dot"), "x\n");
    } else {
        assert_eq!(delete_file(&dir, "ends with dot.", Removal::Permanent).unwrap(), Deleted::Permanently);
        assert!(dir.join("ends with dot.").symlink_metadata().is_err());
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(windows)]
#[test]
fn deletes_a_read_only_file() {
    let dir = scratch_repo("wf-delete-readonly");
    write(&dir, "ro.txt", "x\n");
    let mut p = std::fs::metadata(dir.join("ro.txt")).unwrap().permissions();
    p.set_readonly(true);
    std::fs::set_permissions(dir.join("ro.txt"), p).unwrap();
    assert_eq!(delete_file(&dir, "ro.txt", Removal::Permanent).unwrap(), Deleted::Permanently);
    assert!(!dir.join("ro.txt").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn bare_repository_is_unsupported() {
    let dir = crate::paths::temp_dir().join(format!("glowfly-test-wf-bare-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "-q", "--bare"]);
    assert!(matches!(create_file(&dir, "a.txt"), Err(Error::Unsupported(_))));
    assert!(matches!(rename_file(&dir, "a.txt", "b.txt"), Err(Error::Unsupported(_))));
    assert!(matches!(delete_file(&dir, "a.txt", Removal::Permanent), Err(Error::Unsupported(_))));
    let _ = std::fs::remove_dir_all(&dir);
}
