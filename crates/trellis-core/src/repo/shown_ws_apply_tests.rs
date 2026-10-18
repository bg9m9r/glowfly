//! Staging, unstaging and discarding a hunk or lines shown with whitespace changes hidden,
//! against real repositories: only the shown changes move, in the order shown, and the hidden
//! whitespace changes stay where they were.

use super::*;
use crate::testutil::*;
use git2::Repository;
use std::path::{Path, PathBuf};

const IGNORE: LoadDiffOptions = LoadDiffOptions { ignore_whitespace: true, whole_file: false };
const OLD: &str = "a\n    x\n    y\nb\n";
/// `OLD` with `x` and `y` wrapped in a block, re-indented.
const NEW: &str = "a\nif {\n        x\n        y\n}\nb\n";

fn index_text(dir: &Path, path: &str) -> String {
    let repo = Repository::open(dir).unwrap();
    let entry = repo.index().unwrap().get_path(Path::new(path), 0).expect("in the index");
    String::from_utf8(repo.find_blob(entry.id).unwrap().content().to_vec()).unwrap()
}

fn read(dir: &Path, path: &str) -> String {
    String::from_utf8(std::fs::read(dir.join(path)).unwrap()).unwrap()
}

/// `f.txt` committed as `old` and changed to `new` in the working tree.
fn changed(name: &str, old: &str, new: &str) -> PathBuf {
    let dir = scratch_repo(name);
    write(&dir, "f.txt", old);
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    write(&dir, "f.txt", new);
    dir
}

/// The one hunk `target` shows with whitespace hidden.
fn shown(dir: &Path, target: &DiffTarget) -> Hunk {
    let d = load_diff_with(dir, target, IGNORE).unwrap();
    assert_eq!(d.hunks.len(), 1, "{:?}", d.hunks);
    d.hunks[0].clone()
}

fn unstaged() -> DiffTarget {
    DiffTarget::Unstaged("f.txt".into())
}

fn staged() -> DiffTarget {
    DiffTarget::Staged("f.txt".into())
}

/// The changed lines of `target`'s unfiltered diff.
fn changes(dir: &Path, target: &DiffTarget) -> Vec<String> {
    let d = load_diff(dir, target).unwrap();
    d.hunks.iter().flat_map(|h| &h.lines).filter(|l| l.origin != ' ').map(|l| format!("{}{}", l.origin, l.text)).collect()
}

fn whole(dir: &Path, target: &DiffTarget, op: HunkOp) -> crate::error::Result<()> {
    apply_shown(dir, "f.txt", 0, &shown(dir, target), None, op)
}

#[test]
fn reindent_stage_keeps_order() {
    let dir = changed("ws-stage-order", OLD, NEW);
    let hunk = shown(&dir, &unstaged());
    let lines: Vec<String> = hunk.lines.iter().map(|l| format!("{}{}", l.origin, l.text)).collect();
    assert_eq!(lines, [" a", "+if {", "         x", "         y", "+}", " b"], "context shows one side's indent");
    apply_shown(&dir, "f.txt", 0, &hunk, None, HunkOp::Stage).unwrap();
    assert_eq!(index_text(&dir, "f.txt"), "a\nif {\n    x\n    y\n}\nb\n");
    assert_eq!(read(&dir, "f.txt"), NEW, "the working tree is untouched");
    assert_eq!(changes(&dir, &unstaged()), ["-    x", "-    y", "+        x", "+        y"], "only the re-indent is left");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn reindent_discard_keeps_reindent() {
    let dir = changed("ws-discard-reindent", OLD, NEW);
    whole(&dir, &unstaged(), HunkOp::Discard).unwrap();
    assert_eq!(read(&dir, "f.txt"), "a\n        x\n        y\nb\n");
    assert_eq!(index_text(&dir, "f.txt"), OLD);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn trailing_space_context_discard() {
    // The shown context `x` and the file being patched may differ by the two spaces.
    let dir = changed("ws-trailing-context", "x\nfoo\ny\n", "x  \nbar\ny\n");
    whole(&dir, &unstaged(), HunkOp::Discard).unwrap();
    assert_eq!(read(&dir, "f.txt"), "x  \nfoo\ny\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn trailing_space_context_stage() {
    let dir = changed("ws-trailing-stage", "x\nfoo\ny\n", "x  \nbar\ny\n");
    whole(&dir, &unstaged(), HunkOp::Stage).unwrap();
    assert_eq!(index_text(&dir, "f.txt"), "x\nbar\ny\n", "the index keeps its own x");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unstage_shown() {
    let dir = changed("ws-unstage", OLD, NEW);
    git(&dir, &["add", "f.txt"]);
    whole(&dir, &staged(), HunkOp::Unstage).unwrap();
    assert_eq!(index_text(&dir, "f.txt"), "a\n        x\n        y\nb\n", "the re-indent stays staged");
    assert_eq!(read(&dir, "f.txt"), NEW);
    assert_eq!(changes(&dir, &staged()), ["-    x", "-    y", "+        x", "+        y"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn shown_lines_stage() {
    let dir = changed("ws-stage-line", OLD, NEW);
    let hunk = shown(&dir, &unstaged());
    let close = hunk.lines.iter().position(|l| l.origin == '+' && l.text == "}").unwrap();
    apply_shown(&dir, "f.txt", 0, &hunk, Some(&[close]), HunkOp::Stage).unwrap();
    assert_eq!(index_text(&dir, "f.txt"), "a\n    x\n    y\n}\nb\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn shown_lines_discard() {
    let dir = changed("ws-discard-line", OLD, NEW);
    let hunk = shown(&dir, &unstaged());
    let close = hunk.lines.iter().position(|l| l.origin == '+' && l.text == "}").unwrap();
    apply_shown(&dir, "f.txt", 0, &hunk, Some(&[close]), HunkOp::Discard).unwrap();
    assert_eq!(
        read(&dir, "f.txt"),
        "a
if {
        x
        y
b
",
        "only the brace goes; the re-indent stays"
    );
    assert_eq!(index_text(&dir, "f.txt"), OLD);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn shown_lines_unstage() {
    let dir = changed("ws-unstage-line", OLD, NEW);
    git(&dir, &["add", "f.txt"]);
    let hunk = shown(&dir, &staged());
    let close = hunk.lines.iter().position(|l| l.origin == '+' && l.text == "}").unwrap();
    apply_shown(&dir, "f.txt", 0, &hunk, Some(&[close]), HunkOp::Unstage).unwrap();
    assert_eq!(
        index_text(&dir, "f.txt"),
        "a
if {
        x
        y
b
",
        "the rest stays staged"
    );
    assert_eq!(read(&dir, "f.txt"), NEW, "the working tree is untouched");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn eof_newline_stays_hidden() {
    let dir = changed("ws-eof-stage", "a\nb", "A\nb\n");
    whole(&dir, &unstaged(), HunkOp::Stage).unwrap();
    assert_eq!(index_text(&dir, "f.txt"), "A\nb", "the last line still has no newline");
    let _ = std::fs::remove_dir_all(&dir);

    let dir = changed("ws-eof-discard", "a\nb", "A\nb\n");
    whole(&dir, &unstaged(), HunkOp::Discard).unwrap();
    assert_eq!(read(&dir, "f.txt"), "a\nb\n", "the added newline stays");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stale_shown_hunk_refused() {
    let dir = changed("ws-stale", OLD, NEW);
    let mut hunk = shown(&dir, &unstaged());
    hunk.lines[1].text = "if (x) {".into();
    for op in [HunkOp::Stage, HunkOp::Discard] {
        let r = apply_shown(&dir, "f.txt", 0, &hunk, None, op);
        assert!(r.as_ref().is_err_and(|e| e.is_not_found()), "{op:?}: {r:?}");
        let r = apply_shown(&dir, "f.txt", 0, &hunk, Some(&[1]), op);
        assert!(r.as_ref().is_err_and(|e| e.is_not_found()), "{op:?} lines: {r:?}");
    }
    assert_eq!(read(&dir, "f.txt"), NEW);
    assert_eq!(index_text(&dir, "f.txt"), OLD);
    let _ = std::fs::remove_dir_all(&dir);
}
