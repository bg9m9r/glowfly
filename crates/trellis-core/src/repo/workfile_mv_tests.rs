//! Staged renames (`git mv`) against real repositories: the index records the rename, other
//! changes to the file are kept, folders are made and pruned, and every refusal or git failure
//! changes nothing.

use super::workfile_tests::{commit, conflicted_repo, index_id, names, status, text};
use super::*;
use crate::testutil::*;
use git2::Repository;
use std::path::Path;

#[test]
fn staged_rename_keeps_staged_and_unstaged_changes() {
    let dir = scratch_repo("wfmv-keep");
    commit(&dir, &[("old.txt", "x\n")], "one");
    write(&dir, "old.txt", "x\ny\n");
    git(&dir, &["add", "old.txt"]);
    let staged = index_id(&dir, "old.txt");
    write(&dir, "old.txt", "x\ny\nz\n");
    rename_file_staged(&dir, "old.txt", "sub/new.txt").unwrap();
    assert_eq!(status(&dir), "RM old.txt -> sub/new.txt");
    assert_eq!(index_id(&dir, "sub/new.txt"), staged);
    assert_eq!(index_id(&dir, "old.txt"), None);
    assert_eq!(text(&dir, "sub/new.txt"), "x\ny\nz\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn staged_rename_prunes_emptied_source_folders() {
    let dir = scratch_repo("wfmv-prune");
    commit(&dir, &[("old/deep/n.txt", "n\n")], "one");
    rename_file_staged(&dir, "old/deep/n.txt", "n.txt").unwrap();
    assert!(!dir.join("old").exists());
    assert_eq!(status(&dir), "R  old/deep/n.txt -> n.txt");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn staged_rename_of_a_staged_new_file() {
    let dir = scratch_repo("wfmv-new");
    commit(&dir, &[("a.txt", "a\n")], "one");
    write(&dir, "new.txt", "n\n");
    git(&dir, &["add", "new.txt"]);
    rename_file_staged(&dir, "new.txt", "moved.txt").unwrap();
    assert_eq!(status(&dir), "A  moved.txt");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn staged_rename_case_only() {
    let dir = scratch_repo("wfmv-case");
    commit(&dir, &[("readme.md", "hi\n")], "one");
    rename_file_staged(&dir, "readme.md", "README.md").unwrap();
    let listed = names(&dir);
    assert!(listed.contains(&"README.md".to_owned()) && !listed.contains(&"readme.md".to_owned()), "{listed:?}");
    assert!(!listed.iter().any(|n| n.contains("trellis-rename")), "{listed:?}");
    let index = Repository::open(&dir).unwrap().index().unwrap();
    let paths: Vec<String> = index.iter().map(|e| String::from_utf8_lossy(&e.path).into_owned()).collect();
    assert_eq!(paths, ["README.md"]);
    assert!(index.get_path(Path::new("README.md"), 0).is_some());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn staged_rename_odd_names() {
    let dir = scratch_repo("wfmv-odd");
    commit(&dir, &[("-x.txt", "dash\n"), ("[a].txt", "brackets\n"), ("a.txt", "a\n")], "one");
    rename_file_staged(&dir, "-x.txt", "y.txt").unwrap();
    assert_eq!(status(&dir), "R  -x.txt -> y.txt");
    git(&dir, &["commit", "-q", "-m", "two"]);
    rename_file_staged(&dir, "[a].txt", "b.txt").unwrap();
    assert_eq!(status(&dir), "R  [a].txt -> b.txt");
    assert_eq!(text(&dir, "a.txt"), "a\n", "the pattern matched nothing else");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn staged_rename_refusals_change_nothing() {
    let dir = scratch_repo("wfmv-refuse");
    commit(&dir, &[("a.txt", "a\n"), ("b.txt", "b\n"), ("c.txt", "c\n"), ("k.txt", "k\n")], "one");
    write(&dir, "u.txt", "u\n");
    git(&dir, &["rm", "-q", "--cached", "k.txt"]);
    std::fs::remove_file(dir.join("c.txt")).unwrap();
    let before = status(&dir);
    let ids = || ["a.txt", "b.txt", "c.txt", "k.txt"].map(|p| index_id(&dir, p));
    let ids_before = ids();
    let err = |from: &str, to: &str| rename_file_staged(&dir, from, to).unwrap_err();

    for from in ["u.txt", "k.txt"] {
        let e = err(from, "moved.txt");
        assert!(e.to_string().contains("isn't tracked"), "{from}: {e}");
    }
    assert!(err("a.txt", "b.txt").to_string().contains("already exists"));
    assert!(err("a.txt", "./a.txt").to_string().contains("same"));
    let _ = err("a.txt", ".git/x");
    assert!(err("ghost.txt", "g.txt").is_not_found());
    let e = err("a.txt", "c.txt");
    assert!(e.to_string().contains("still in the index"), "{e}");

    assert_eq!(status(&dir), before);
    assert_eq!(ids(), ids_before);
    assert!(!dir.join("moved.txt").exists() && !dir.join("c.txt").exists());
    assert!(!dir.join(".git/x").exists());
    let _ = std::fs::remove_dir_all(&dir);

    let dir = conflicted_repo("wfmv-conflict");
    let before = (status(&dir), text(&dir, "f.txt"));
    assert!(rename_file_staged(&dir, "f.txt", "g.txt").unwrap_err().to_string().contains("conflicts"));
    assert_eq!((status(&dir), text(&dir, "f.txt")), before);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn staged_rename_git_failure_removes_created_folders() {
    let dir = scratch_repo("wfmv-lock");
    commit(&dir, &[("a.txt", "a\n")], "one");
    let id = index_id(&dir, "a.txt");
    let lock = dir.join(".git/index.lock");
    std::fs::write(&lock, "").unwrap();
    let e = rename_file_staged(&dir, "a.txt", "new/dir/a.txt").unwrap_err();
    assert!(e.to_string().contains("index.lock"), "{e}");
    assert!(!dir.join("new").exists());
    assert_eq!(text(&dir, "a.txt"), "a\n");
    assert_eq!(index_id(&dir, "a.txt"), id);
    std::fs::remove_file(&lock).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}
