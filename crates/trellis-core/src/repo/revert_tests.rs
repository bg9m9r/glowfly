//! Reverting a hunk of a commit or comparison diff against real repositories: exact working-tree
//! bytes afterwards, with the index and HEAD left alone.

use super::*;
use crate::testutil::*;
use git2::{Oid, Repository};
use std::path::{Path, PathBuf};

const TEN: &str = "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n";
/// `5` replaced by `five`, and `6a`, `6b` inserted after `6`: one hunk.
const EDITED: &str = "1\n2\n3\n4\nfive\n6\n6a\n6b\n7\n8\n9\n10\n";
const NO_MATCH: &str = "no longer has this hunk";

fn read(dir: &Path, path: &str) -> Vec<u8> {
    std::fs::read(dir.join(path)).unwrap()
}

fn text(dir: &Path, path: &str) -> String {
    String::from_utf8(read(dir, path)).unwrap()
}

fn head(dir: &Path) -> Oid {
    Repository::open(dir).unwrap().head().unwrap().target().unwrap()
}

fn index_text(dir: &Path, path: &str) -> String {
    let repo = Repository::open(dir).unwrap();
    let entry = repo.index().unwrap().get_path(Path::new(path), 0).expect("in the index");
    String::from_utf8(repo.find_blob(entry.id).unwrap().content().to_vec()).unwrap()
}

fn commit(dir: &Path, files: &[(&str, &str)], msg: &str) -> Oid {
    for (f, t) in files {
        write(dir, f, t);
    }
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", msg]);
    head(dir)
}

/// Reverts hunk `h` of `target` as `load_diff` shows it now.
fn revert(dir: &Path, target: &DiffTarget, h: usize) -> crate::error::Result<()> {
    let hunk = load_diff(dir, target).unwrap().hunks[h].clone();
    revert_hunk(dir, target, h, &hunk)
}

/// `f.txt` committed as `TEN`, then as `EDITED` (HEAD).
fn edited(name: &str) -> (PathBuf, DiffTarget) {
    let dir = scratch_repo(name);
    commit(&dir, &[("f.txt", TEN)], "base");
    let c = commit(&dir, &[("f.txt", EDITED)], "edit");
    (dir, DiffTarget::Commit(c, "f.txt".into()))
}

fn numbered(n: usize, changed: &[(usize, &str)]) -> String {
    (1..=n).map(|i| changed.iter().find(|(c, _)| *c == i).map_or(format!("{i}\n"), |(_, t)| format!("{t}\n"))).collect()
}

#[test]
fn reverts_a_hunk_of_the_head_commit() {
    let (dir, target) = edited("revert-head");
    let before = head(&dir);
    revert(&dir, &target, 0).unwrap();
    assert_eq!(text(&dir, "f.txt"), TEN);
    assert_eq!(index_text(&dir, "f.txt"), EDITED, "the index is untouched");
    assert_eq!(head(&dir), before);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn reverts_only_the_chosen_hunk() {
    let dir = scratch_repo("revert-chosen");
    commit(&dir, &[("f.txt", &numbered(30, &[]))], "base");
    let c = commit(&dir, &[("f.txt", &numbered(30, &[(2, "two"), (25, "twenty-five")]))], "two edits");
    let target = DiffTarget::Commit(c, "f.txt".into());
    assert_eq!(load_diff(&dir, &target).unwrap().hunks.len(), 2);
    revert(&dir, &target, 1).unwrap();
    assert_eq!(text(&dir, "f.txt"), numbered(30, &[(2, "two")]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn finds_the_hunk_after_lines_moved() {
    let (dir, target) = edited("revert-moved");
    let top = "n1\nn2\nn3\nn4\nn5\n";
    commit(&dir, &[("f.txt", &format!("{top}{EDITED}"))], "insert at the top");
    revert(&dir, &target, 0).unwrap();
    assert_eq!(text(&dir, "f.txt"), format!("{top}{TEN}"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn keeps_other_uncommitted_edits() {
    let dir = scratch_repo("revert-keep");
    commit(&dir, &[("f.txt", &numbered(30, &[]))], "base");
    let c = commit(&dir, &[("f.txt", &numbered(30, &[(20, "twenty")]))], "edit");
    write(&dir, "f.txt", &numbered(30, &[(1, "ONE"), (20, "twenty")]));
    revert(&dir, &DiffTarget::Commit(c, "f.txt".into()), 0).unwrap();
    assert_eq!(text(&dir, "f.txt"), numbered(30, &[(1, "ONE")]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn refuses_when_the_working_file_changed() {
    let (dir, target) = edited("revert-changed");
    let changed = EDITED.replace("five", "FIVE");
    write(&dir, "f.txt", &changed);
    let err = revert(&dir, &target, 0).unwrap_err();
    assert!(err.to_string().contains(NO_MATCH), "{err}");
    assert_eq!(read(&dir, "f.txt"), changed.as_bytes());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn second_revert_is_refused() {
    let (dir, target) = edited("revert-twice");
    let hunk = load_diff(&dir, &target).unwrap().hunks[0].clone();
    revert_hunk(&dir, &target, 0, &hunk).unwrap();
    let err = revert_hunk(&dir, &target, 0, &hunk).unwrap_err();
    assert!(err.to_string().contains(NO_MATCH), "{err}");
    assert_eq!(text(&dir, "f.txt"), TEN);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn refuses_a_stale_expected_hunk() {
    let (dir, target) = edited("revert-stale");
    let hunk = load_diff(&dir, &target).unwrap().hunks[0].clone();
    let mut other_header = hunk.clone();
    other_header.header = "@@ -1,9 +1,9 @@".into();
    let mut extra_line = hunk.clone();
    extra_line.lines.push(extra_line.lines[0].clone());
    let attempts =
        [revert_hunk(&dir, &target, 0, &other_header), revert_hunk(&dir, &target, 0, &extra_line), revert_hunk(&dir, &target, 1, &hunk)];
    for (i, r) in attempts.into_iter().enumerate() {
        assert!(r.as_ref().is_err_and(|e| e.is_not_found()), "attempt {i}: {:?}", r.err());
    }
    assert_eq!(text(&dir, "f.txt"), EDITED);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn refuses_when_the_file_is_missing() {
    let (dir, target) = edited("revert-missing");
    std::fs::remove_file(dir.join("f.txt")).unwrap();
    let r = revert(&dir, &target, 0);
    assert!(r.as_ref().is_err_and(|e| e.is_not_found()), "{:?}", r.err());
    assert!(!dir.join("f.txt").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn reverts_a_comparison() {
    let dir = scratch_repo("revert-between");
    let a = commit(&dir, &[("f.txt", &numbered(10, &[]))], "A");
    commit(&dir, &[("f.txt", &numbered(10, &[(4, "four")]))], "B");
    let c = commit(&dir, &[("f.txt", &numbered(10, &[(4, "four"), (5, "five")]))], "C");
    revert(&dir, &DiffTarget::Between(a, c, "f.txt".into()), 0).unwrap();
    assert_eq!(text(&dir, "f.txt"), numbered(10, &[]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn renamed_file_reverts_content_only() {
    let dir = scratch_repo("revert-renamed");
    commit(&dir, &[("old.txt", &numbered(20, &[]))], "base");
    std::fs::remove_file(dir.join("old.txt")).unwrap();
    let c = commit(&dir, &[("new.txt", &numbered(20, &[(10, "ten")]))], "rename and edit");
    let target = DiffTarget::Commit(c, "new.txt".into());
    revert(&dir, &target, 0).unwrap();
    assert_eq!(text(&dir, "new.txt"), numbered(20, &[]));
    assert!(!dir.join("old.txt").exists(), "the rename is not undone");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn deleted_file_comes_back() {
    let dir = scratch_repo("revert-deleted");
    commit(&dir, &[("g.txt", "a\nb\nc\n"), ("keep.txt", "k\n")], "base");
    std::fs::remove_file(dir.join("g.txt")).unwrap();
    let c = commit(&dir, &[], "delete g");
    let target = DiffTarget::Commit(c, "g.txt".into());
    let hunk = load_diff(&dir, &target).unwrap().hunks[0].clone();

    write(&dir, "g.txt", "something else\n");
    let err = revert_hunk(&dir, &target, 0, &hunk).unwrap_err();
    assert!(err.to_string().contains("exists in the working tree"), "{err}");
    assert_eq!(read(&dir, "g.txt"), b"something else\n");

    std::fs::remove_file(dir.join("g.txt")).unwrap();
    revert_hunk(&dir, &target, 0, &hunk).unwrap();
    assert_eq!(read(&dir, "g.txt"), b"a\nb\nc\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn added_file_becomes_empty() {
    let dir = scratch_repo("revert-added");
    commit(&dir, &[("keep.txt", "k\n")], "base");
    let c = commit(&dir, &[("h.txt", "x\ny\n")], "add h");
    revert(&dir, &DiffTarget::Commit(c, "h.txt".into()), 0).unwrap();
    assert_eq!(read(&dir, "h.txt"), b"", "left empty rather than removed");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn keeps_crlf_and_missing_final_newline() {
    let dir = scratch_repo("revert-crlf");
    let before = "a\r\nb\r\nc\r\nlast";
    commit(&dir, &[("w.txt", before)], "base");
    let c = commit(&dir, &[("w.txt", "a\r\nb\r\nc\r\nLAST")], "edit");
    revert(&dir, &DiffTarget::Commit(c, "w.txt".into()), 0).unwrap();
    assert_eq!(read(&dir, "w.txt"), before.as_bytes());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn working_tree_targets_are_unsupported() {
    let (dir, _) = edited("revert-worktree");
    write(&dir, "f.txt", TEN);
    let hunk = load_diff(&dir, &DiffTarget::Unstaged("f.txt".into())).unwrap().hunks[0].clone();
    for target in [DiffTarget::Unstaged("f.txt".into()), DiffTarget::Staged("f.txt".into())] {
        let r = revert_hunk(&dir, &target, 0, &hunk);
        assert!(matches!(r, Err(crate::error::Error::Unsupported(_))), "{target:?}: {:?}", r.err());
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn reverts_a_tab_indented_hunk() {
    let dir = scratch_repo("revert-tabs");
    let before = "fn f() {\n\tx();\n}\n";
    commit(&dir, &[("f.rs", before)], "base");
    let c = commit(&dir, &[("f.rs", "fn f() {\n\tx();\n\t\ty();\n}\n")], "add a tab-indented line");
    revert(&dir, &DiffTarget::Commit(c, "f.rs".into()), 0).unwrap();
    assert_eq!(text(&dir, "f.rs"), before);
    let _ = std::fs::remove_dir_all(&dir);
}
