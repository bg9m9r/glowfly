//! Reverting a hunk or lines of a commit diff shown with whitespace changes hidden: only the
//! shown changes are undone, and the hidden re-indent stays in the working tree.

use super::revert_tests::{commit, text};
use super::*;
use crate::error::Result;
use crate::testutil::*;
use std::path::{Path, PathBuf};

const IGNORE: LoadDiffOptions = LoadDiffOptions { ignore_whitespace: true, whole_file: false };
const OLD: &str = "1\n2\n3\n4\n    x\n    y\n7\n8\n9\n";
/// `OLD` with `x` and `y` wrapped in a block and re-indented.
const NEW: &str = "1\n2\n3\n4\nif {\n        x\n        y\n}\n7\n8\n9\n";

/// `f.txt` committed as `OLD`, then as `NEW` (HEAD).
fn reindented(name: &str) -> (PathBuf, DiffTarget) {
    let dir = scratch_repo(name);
    commit(&dir, &[("f.txt", OLD)], "base");
    let c = commit(&dir, &[("f.txt", NEW)], "wrap in a block");
    (dir, DiffTarget::Commit(c, "f.txt".into()))
}

fn shown(dir: &Path, target: &DiffTarget) -> Hunk {
    let d = load_diff_with(dir, target, IGNORE).unwrap();
    assert_eq!(d.hunks.len(), 1);
    d.hunks[0].clone()
}

fn revert_ws(dir: &Path, target: &DiffTarget, part: RevertPart) -> Result<Reverted> {
    super::revert(dir, &RevertRequest { target: target.clone(), part, dest: None, ignore_whitespace: true })
}

fn hunk_part(dir: &Path, target: &DiffTarget) -> RevertPart {
    RevertPart::Hunk { index: 0, expected: shown(dir, target) }
}

#[test]
fn reverts_the_shown_hunk_and_keeps_the_reindent() {
    let (dir, target) = reindented("revert-ws-hunk");
    let part = hunk_part(&dir, &target);
    assert_eq!(revert_ws(&dir, &target, part).map_err(|e| e.to_string()), Ok(Reverted::Applied));
    assert_eq!(text(&dir, "f.txt"), "1\n2\n3\n4\n        x\n        y\n7\n8\n9\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn reverts_one_shown_line() {
    let (dir, target) = reindented("revert-ws-line");
    let expected = shown(&dir, &target);
    let close = expected.lines.iter().position(|l| l.origin == '+' && l.text == "}").unwrap();
    let part = RevertPart::Lines { hunk: 0, expected, lines: vec![close] };
    assert_eq!(revert_ws(&dir, &target, part).map_err(|e| e.to_string()), Ok(Reverted::Applied));
    assert_eq!(text(&dir, "f.txt"), "1\n2\n3\n4\nif {\n        x\n        y\n7\n8\n9\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn merges_when_the_context_was_edited() {
    let (dir, target) = reindented("revert-ws-merge");
    let part = hunk_part(&dir, &target);
    write(&dir, "f.txt", &NEW.replacen("2\n", "TWO\n", 1));
    assert_eq!(revert_ws(&dir, &target, part).map_err(|e| e.to_string()), Ok(Reverted::Merged));
    assert_eq!(text(&dir, "f.txt"), "1\nTWO\n3\n4\n        x\n        y\n7\n8\n9\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn file_part_ignores_the_flag() {
    let (dir, target) = reindented("revert-ws-file");
    assert!(revert_ws(&dir, &target, RevertPart::File).is_ok());
    assert_eq!(text(&dir, "f.txt"), OLD, "the whole change, whitespace too");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unfiltered_indexes_are_refused_as_stale() {
    // The unfiltered hunk is not what the filtered diff shows at index 0.
    let (dir, target) = reindented("revert-ws-stale");
    let expected = load_diff(&dir, &target).unwrap().hunks[0].clone();
    let r = revert_ws(&dir, &target, RevertPart::Hunk { index: 0, expected });
    assert!(r.as_ref().is_err_and(|e| e.is_not_found()), "{r:?}");
    assert_eq!(text(&dir, "f.txt"), NEW);
    let _ = std::fs::remove_dir_all(&dir);
}
