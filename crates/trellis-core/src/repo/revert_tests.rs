//! Reverting a hunk or some lines of a commit or comparison diff against real repositories:
//! exact working-tree bytes afterwards, with the index and HEAD left alone, the three-way
//! fallback, and the File history rename mapping.

use super::*;
use crate::error::Result;
use crate::testutil::*;
use git2::{Oid, Repository};
use std::path::{Path, PathBuf};

pub(super) const TEN: &str = "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n";
/// `5` replaced by `five`, and `6a`, `6b` inserted after `6`: one hunk.
pub(super) const EDITED: &str = "1\n2\n3\n4\nfive\n6\n6a\n6b\n7\n8\n9\n10\n";

pub(super) fn read(dir: &Path, path: &str) -> Vec<u8> {
    std::fs::read(dir.join(path)).unwrap()
}

pub(super) fn text(dir: &Path, path: &str) -> String {
    String::from_utf8(read(dir, path)).unwrap()
}

pub(super) fn head(dir: &Path) -> Oid {
    Repository::open(dir).unwrap().head().unwrap().target().unwrap()
}

pub(super) fn index_text(dir: &Path, path: &str) -> String {
    let repo = Repository::open(dir).unwrap();
    let entry = repo.index().unwrap().get_path(Path::new(path), 0).expect("in the index");
    String::from_utf8(repo.find_blob(entry.id).unwrap().content().to_vec()).unwrap()
}

pub(super) fn commit(dir: &Path, files: &[(&str, &str)], msg: &str) -> Oid {
    for (f, t) in files {
        write(dir, f, t);
    }
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", msg]);
    head(dir)
}

/// Reverts `part` of `target`, in `dest` when set.
pub(super) fn request(dir: &Path, target: &DiffTarget, part: RevertPart, dest: Option<&str>) -> Result<Reverted> {
    super::revert(dir, &RevertRequest { target: target.clone(), part, dest: dest.map(str::to_owned), ignore_whitespace: false })
}

/// Reverts hunk `h` of `target`, which must still be `expected`.
fn revert_hunk(dir: &Path, target: &DiffTarget, h: usize, expected: &Hunk) -> Result<Reverted> {
    request(dir, target, RevertPart::Hunk { index: h, expected: expected.clone() }, None)
}

/// Reverts hunk `h` of `target` as `load_diff` shows it now.
fn revert(dir: &Path, target: &DiffTarget, h: usize) -> Result<Reverted> {
    let hunk = load_diff(dir, target).unwrap().hunks[h].clone();
    revert_hunk(dir, target, h, &hunk)
}

/// Reverts the lines `lines` of hunk `h` of `target` as `load_diff` shows it now.
fn revert_lines(dir: &Path, target: &DiffTarget, h: usize, lines: &[usize]) -> Result<Reverted> {
    let expected = load_diff(dir, target).unwrap().hunks[h].clone();
    request(dir, target, RevertPart::Lines { hunk: h, expected, lines: lines.to_vec() }, None)
}

/// `f.txt` committed as `TEN`, then as `EDITED` (HEAD).
pub(super) fn edited(name: &str) -> (PathBuf, DiffTarget) {
    let dir = scratch_repo(name);
    commit(&dir, &[("f.txt", TEN)], "base");
    let c = commit(&dir, &[("f.txt", EDITED)], "edit");
    (dir, DiffTarget::Commit(c, "f.txt".into()))
}

pub(super) fn numbered(n: usize, changed: &[(usize, &str)]) -> String {
    (1..=n).map(|i| changed.iter().find(|(c, _)| *c == i).map_or(format!("{i}\n"), |(_, t)| format!("{t}\n"))).collect()
}

#[test]
fn reverts_a_hunk_of_the_head_commit() {
    let (dir, target) = edited("revert-head");
    let before = head(&dir);
    assert_eq!(revert(&dir, &target, 0).map_err(|e| e.to_string()), Ok(Reverted::Applied));
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
    assert_eq!(revert(&dir, &target, 1).map_err(|e| e.to_string()), Ok(Reverted::Applied));
    assert_eq!(text(&dir, "f.txt"), numbered(30, &[(2, "two")]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn finds_the_hunk_after_lines_moved() {
    let (dir, target) = edited("revert-moved");
    let top = "n1\nn2\nn3\nn4\nn5\n";
    commit(&dir, &[("f.txt", &format!("{top}{EDITED}"))], "insert at the top");
    assert_eq!(revert(&dir, &target, 0).map_err(|e| e.to_string()), Ok(Reverted::Applied));
    assert_eq!(text(&dir, "f.txt"), format!("{top}{TEN}"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn keeps_other_uncommitted_edits() {
    let dir = scratch_repo("revert-keep");
    commit(&dir, &[("f.txt", &numbered(30, &[]))], "base");
    let c = commit(&dir, &[("f.txt", &numbered(30, &[(20, "twenty")]))], "edit");
    write(&dir, "f.txt", &numbered(30, &[(1, "ONE"), (20, "twenty")]));
    assert_eq!(revert(&dir, &DiffTarget::Commit(c, "f.txt".into()), 0).map_err(|e| e.to_string()), Ok(Reverted::Applied));
    assert_eq!(text(&dir, "f.txt"), numbered(30, &[(1, "ONE")]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn falls_back_to_conflict_markers() {
    let (dir, target) = edited("revert-changed");
    let before = head(&dir);
    write(&dir, "f.txt", &EDITED.replace("five", "FIVE"));
    assert_eq!(revert(&dir, &target, 0).map_err(|e| e.to_string()), Ok(Reverted::Conflicts));
    let t = text(&dir, "f.txt");
    for want in ["<<<<<<< working copy", "FIVE", "5", ">>>>>>> revert of"] {
        assert!(t.contains(want), "{want:?} in {t}");
    }
    assert_eq!(index_text(&dir, "f.txt"), EDITED, "the index is untouched");
    assert_eq!(head(&dir), before);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn second_revert_says_already() {
    let (dir, target) = edited("revert-twice");
    let hunk = load_diff(&dir, &target).unwrap().hunks[0].clone();
    assert_eq!(revert_hunk(&dir, &target, 0, &hunk).map_err(|e| e.to_string()), Ok(Reverted::Applied));
    let err = revert_hunk(&dir, &target, 0, &hunk).unwrap_err();
    assert!(err.to_string().contains(ALREADY_REVERTED), "{err}");
    assert_eq!(text(&dir, "f.txt"), TEN);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn clean_merge_when_context_moved_on() {
    let (dir, target) = edited("revert-context");
    write(&dir, "f.txt", &EDITED.replacen("2\n", "TWO\n", 1));
    assert_eq!(revert(&dir, &target, 0).map_err(|e| e.to_string()), Ok(Reverted::Merged));
    assert_eq!(text(&dir, "f.txt"), TEN.replacen("2\n", "TWO\n", 1));
    assert_eq!(index_text(&dir, "f.txt"), EDITED);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn three_way_keeps_crlf_with_autocrlf() {
    let dir = scratch_repo("revert-autocrlf");
    set_config(&dir, &[("core.autocrlf", "true")]);
    let crlf = |s: &str| s.replace('\n', "\r\n");
    commit(&dir, &[("f.txt", &crlf(TEN))], "base");
    let c = commit(&dir, &[("f.txt", &crlf(EDITED))], "edit");
    write(&dir, "f.txt", &crlf(&EDITED.replacen("3\n", "THREE\n", 1)));
    assert_eq!(revert(&dir, &DiffTarget::Commit(c, "f.txt".into()), 0).map_err(|e| e.to_string()), Ok(Reverted::Merged));
    let t = text(&dir, "f.txt");
    assert_eq!(t, crlf(&TEN.replacen("3\n", "THREE\n", 1)));
    assert!(t.split_inclusive('\n').all(|l| l.ends_with("\r\n")), "{t:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn reverts_selected_lines_only() {
    let (dir, target) = edited("revert-lines");
    let hunk = load_diff(&dir, &target).unwrap().hunks[0].clone();
    let at = |t: &str| hunk.lines.iter().position(|l| format!("{}{}", l.origin, l.text) == t).unwrap();
    assert_eq!(revert_lines(&dir, &target, 0, &[at("+6a")]).map_err(|e| e.to_string()), Ok(Reverted::Applied));
    assert_eq!(text(&dir, "f.txt"), EDITED.replace("6a\n", ""));
    write(&dir, "f.txt", EDITED);
    assert_eq!(revert_lines(&dir, &target, 0, &[at("-5")]).map_err(|e| e.to_string()), Ok(Reverted::Applied));
    assert_eq!(text(&dir, "f.txt"), EDITED.replace("five\n", "5\nfive\n"));
    write(&dir, "f.txt", EDITED);
    for lines in [vec![], vec![0]] {
        let r = revert_lines(&dir, &target, 0, &lines);
        assert!(r.as_ref().is_err_and(|e| e.is_not_found()), "{lines:?}: {r:?}");
    }
    let mut stale = hunk.clone();
    stale.header = "@@ -1,9 +1,9 @@".into();
    let r = request(&dir, &target, RevertPart::Lines { hunk: 0, expected: stale, lines: vec![at("+6a")] }, None);
    assert!(r.as_ref().is_err_and(|e| e.is_not_found()), "{r:?}");
    assert_eq!(text(&dir, "f.txt"), EDITED);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn lines_fall_back_to_merge() {
    let (dir, target) = edited("revert-lines-merge");
    let hunk = load_diff(&dir, &target).unwrap().hunks[0].clone();
    let at = hunk.lines.iter().position(|l| l.origin == '+' && l.text == "6b").unwrap();
    write(&dir, "f.txt", &EDITED.replacen("3\n", "THREE\n", 1));
    assert_eq!(revert_lines(&dir, &target, 0, &[at]).map_err(|e| e.to_string()), Ok(Reverted::Merged));
    assert_eq!(text(&dir, "f.txt"), EDITED.replacen("3\n", "THREE\n", 1).replace("6b\n", ""));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn lines_merge_refuses_a_binary_working_file() {
    let (dir, target) = edited("revert-lines-nul");
    let hunk = load_diff(&dir, &target).unwrap().hunks[0].clone();
    let at = hunk.lines.iter().position(|l| l.origin == '+' && l.text == "6b").unwrap();
    let mine = EDITED.replacen("3\n", "TH\0REE\n", 1);
    write(&dir, "f.txt", &mine);
    let r = revert_lines(&dir, &target, 0, &[at]);
    assert!(matches!(&r, Err(crate::error::Error::Unsupported(e)) if e.contains("binary")), "{r:?}");
    assert_eq!(text(&dir, "f.txt"), mine, "the working file is untouched");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn dest_follows_a_rename() {
    let dir = scratch_repo("revert-dest");
    commit(&dir, &[("old.txt", &numbered(20, &[]))], "base");
    let c = commit(&dir, &[("old.txt", &numbered(20, &[(10, "ten")]))], "edit");
    std::fs::rename(dir.join("old.txt"), dir.join("new.txt")).unwrap();
    commit(&dir, &[], "rename");
    let target = DiffTarget::Commit(c, "old.txt".into());
    let expected = load_diff(&dir, &target).unwrap().hunks[0].clone();
    let part = || RevertPart::Hunk { index: 0, expected: expected.clone() };
    let r = request(&dir, &target, part(), None);
    assert!(r.as_ref().is_err_and(|e| e.is_not_found()), "{r:?}");
    let r = request(&dir, &target, part(), Some("nope.txt"));
    assert!(r.as_ref().is_err_and(|e| e.is_not_found()), "{r:?}");
    assert_eq!(request(&dir, &target, part(), Some("new.txt")).map_err(|e| e.to_string()), Ok(Reverted::Applied));
    assert_eq!(text(&dir, "new.txt"), numbered(20, &[]));
    assert!(!dir.join("old.txt").exists());
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
    assert_eq!(revert(&dir, &DiffTarget::Between(a, c, "f.txt".into()), 0).map_err(|e| e.to_string()), Ok(Reverted::Applied));
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
    assert_eq!(revert(&dir, &target, 0).map_err(|e| e.to_string()), Ok(Reverted::Applied));
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
    assert_eq!(revert_hunk(&dir, &target, 0, &hunk).map_err(|e| e.to_string()), Ok(Reverted::Applied));
    assert_eq!(read(&dir, "g.txt"), b"a\nb\nc\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn added_file_is_removed() {
    let dir = scratch_repo("revert-added");
    commit(&dir, &[("keep.txt", "k\n")], "base");
    let c = commit(&dir, &[("h.txt", "x\ny\n")], "add h");
    assert_eq!(revert(&dir, &DiffTarget::Commit(c, "h.txt".into()), 0).map_err(|e| e.to_string()), Ok(Reverted::Removed));
    assert!(!dir.join("h.txt").exists());
    assert_eq!(index_text(&dir, "h.txt"), "x\ny\n", "still tracked");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn added_file_with_later_edits_keeps_them() {
    let dir = scratch_repo("revert-added-edited");
    commit(&dir, &[("keep.txt", "k\n")], "base");
    let c = commit(&dir, &[("h.txt", "x\ny\n")], "add h");
    write(&dir, "h.txt", "x\ny\nz\n");
    assert_eq!(revert(&dir, &DiffTarget::Commit(c, "h.txt".into()), 0).map_err(|e| e.to_string()), Ok(Reverted::Applied));
    assert_eq!(text(&dir, "h.txt"), "z\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn keeps_crlf_and_missing_final_newline() {
    let dir = scratch_repo("revert-crlf");
    let before = "a\r\nb\r\nc\r\nlast";
    commit(&dir, &[("w.txt", before)], "base");
    let c = commit(&dir, &[("w.txt", "a\r\nb\r\nc\r\nLAST")], "edit");
    assert_eq!(revert(&dir, &DiffTarget::Commit(c, "w.txt".into()), 0).map_err(|e| e.to_string()), Ok(Reverted::Applied));
    assert_eq!(read(&dir, "w.txt"), before.as_bytes());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn working_tree_targets_are_unsupported() {
    let (dir, commit) = edited("revert-worktree");
    let DiffTarget::Commit(c, _) = commit else { unreachable!() };
    write(&dir, "f.txt", TEN);
    let hunk = load_diff(&dir, &DiffTarget::Unstaged("f.txt".into())).unwrap().hunks[0].clone();
    let targets = [DiffTarget::Unstaged("f.txt".into()), DiffTarget::Staged("f.txt".into()), DiffTarget::ToWorkdir(c, "f.txt".into())];
    for target in targets {
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
    assert_eq!(revert(&dir, &DiffTarget::Commit(c, "f.rs".into()), 0).map_err(|e| e.to_string()), Ok(Reverted::Applied));
    assert_eq!(text(&dir, "f.rs"), before);
    let _ = std::fs::remove_dir_all(&dir);
}
