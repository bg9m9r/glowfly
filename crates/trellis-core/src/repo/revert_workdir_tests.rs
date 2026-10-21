//! Reverting a hunk or some lines of a commit-to-working-tree diff: the working file gets the
//! commit's lines back and the index is left alone; an untracked file emptied is removed, one
//! deleted from disk recreated; an edited file is refused as stale (never merged); whole-file
//! reverts and the reverse direction are refused.

use super::revert_tests::{EDITED, TEN, commit, index_text, numbered, request, text};
use super::*;
use crate::error::{Error, Result};
use crate::testutil::*;
use std::path::{Path, PathBuf};

/// `f.txt` committed as `TEN`; the target compares that commit with the working tree.
fn committed(name: &str) -> (PathBuf, DiffTarget) {
    let dir = scratch_repo(name);
    let c = commit(&dir, &[("f.txt", TEN)], "base");
    (dir, DiffTarget::ToWorkdir(c, "f.txt".into()))
}

fn at(target: &DiffTarget, path: &str) -> DiffTarget {
    let DiffTarget::ToWorkdir(c, _) = target else { unreachable!() };
    DiffTarget::ToWorkdir(*c, path.into())
}

fn hunk(dir: &Path, target: &DiffTarget, h: usize) -> Hunk {
    load_diff(dir, target).unwrap().hunks[h].clone()
}

fn revert_hunk(dir: &Path, target: &DiffTarget, h: usize) -> Result<Reverted> {
    request(dir, target, RevertPart::Hunk { index: h, expected: hunk(dir, target, h) }, None)
}

fn ok(r: Result<Reverted>) -> std::result::Result<Reverted, String> {
    r.map_err(|e| e.to_string())
}

#[test]
fn one_hunk_restores_only_its_region() {
    let dir = scratch_repo("revert-workdir-hunk");
    let c = commit(&dir, &[("f.txt", &numbered(20, &[]))], "base");
    let target = DiffTarget::ToWorkdir(c, "f.txt".into());
    write(&dir, "f.txt", &numbered(20, &[(2, "two"), (18, "eighteen")]));
    assert_eq!(load_diff(&dir, &target).unwrap().hunks.len(), 2);
    assert_eq!(ok(revert_hunk(&dir, &target, 0)), Ok(Reverted::Applied));
    assert_eq!(text(&dir, "f.txt"), numbered(20, &[(18, "eighteen")]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_staged_change_is_undone_in_the_working_tree_only() {
    let (dir, target) = committed("revert-workdir-staged");
    write(&dir, "f.txt", EDITED);
    git(&dir, &["add", "f.txt"]);
    assert_eq!(ok(revert_hunk(&dir, &target, 0)), Ok(Reverted::Applied));
    assert_eq!(text(&dir, "f.txt"), TEN);
    assert_eq!(index_text(&dir, "f.txt"), EDITED, "the staged blob stays");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn one_added_line_is_removed() {
    let (dir, target) = committed("revert-workdir-line");
    write(&dir, "f.txt", EDITED);
    let expected = hunk(&dir, &target, 0);
    let line = expected.lines.iter().position(|l| l.origin == '+' && l.text == "6a").unwrap();
    let part = RevertPart::Lines { hunk: 0, expected, lines: vec![line] };
    assert_eq!(ok(request(&dir, &target, part, None)), Ok(Reverted::Applied));
    assert_eq!(text(&dir, "f.txt"), "1\n2\n3\n4\nfive\n6\n6b\n7\n8\n9\n10\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_untracked_file_emptied_is_removed() {
    let (dir, target) = committed("revert-workdir-untracked");
    write(&dir, "new.txt", "new\nfile\n");
    let target = at(&target, "new.txt");
    assert_eq!(ok(revert_hunk(&dir, &target, 0)), Ok(Reverted::RemovedNew));
    assert!(!dir.join("new.txt").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_file_deleted_from_disk_is_recreated() {
    let (dir, target) = committed("revert-workdir-deleted");
    std::fs::remove_file(dir.join("f.txt")).unwrap();
    assert_eq!(ok(revert_hunk(&dir, &target, 0)), Ok(Reverted::Applied));
    assert_eq!(text(&dir, "f.txt"), TEN);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_edit_since_the_diff_was_read_is_stale() {
    let (dir, target) = committed("revert-workdir-stale");
    write(&dir, "f.txt", EDITED);
    let expected = hunk(&dir, &target, 0);
    let edited = EDITED.replace("9\n", "nine\n");
    write(&dir, "f.txt", &edited);
    let r = request(&dir, &target, RevertPart::Hunk { index: 0, expected }, None);
    assert!(matches!(r, Err(Error::NotFound(_))), "{r:?}");
    assert_eq!(text(&dir, "f.txt"), edited, "nothing was merged");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn whole_files_and_the_reverse_direction_are_refused() {
    let (dir, target) = committed("revert-workdir-refused");
    write(&dir, "f.txt", EDITED);
    let r = request(&dir, &target, RevertPart::File, None);
    assert!(matches!(r, Err(Error::Unsupported(_))), "{r:?}");
    let DiffTarget::ToWorkdir(c, _) = target else { unreachable!() };
    let back = DiffTarget::FromWorkdir(c, "f.txt".into());
    let expected = hunk(&dir, &back, 0);
    let parts = [
        RevertPart::Hunk { index: 0, expected: expected.clone() },
        RevertPart::Lines { hunk: 0, expected, lines: vec![1] },
        RevertPart::File,
    ];
    for part in parts {
        let r = request(&dir, &back, part.clone(), None);
        assert!(matches!(r, Err(Error::Unsupported(_))), "{part:?}: {r:?}");
    }
    assert_eq!(text(&dir, "f.txt"), EDITED);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn with_whitespace_hidden_only_the_shown_change_is_reverted() {
    let dir = scratch_repo("revert-workdir-ws");
    let c = commit(&dir, &[("f.txt", "1\n2\n3\n4\n    x\n    y\n7\n8\n9\n")], "base");
    let target = DiffTarget::ToWorkdir(c, "f.txt".into());
    write(&dir, "f.txt", "1\n2\n3\n4\nif {\n        x\n        y\n}\n7\n8\n9\n");
    let ws = LoadDiffOptions { ignore_whitespace: true, whole_file: false };
    let expected = load_diff_with(&dir, &target, ws).unwrap().hunks[0].clone();
    let req = RevertRequest { target, part: RevertPart::Hunk { index: 0, expected }, dest: None, ignore_whitespace: true };
    assert_eq!(ok(revert(&dir, &req)), Ok(Reverted::Applied));
    assert_eq!(text(&dir, "f.txt"), "1\n2\n3\n4\n        x\n        y\n7\n8\n9\n", "the re-indent stays");
    let _ = std::fs::remove_dir_all(&dir);
}
