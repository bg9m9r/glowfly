//! File diffs loaded with the whole file, against real repositories.

use super::*;
use crate::testutil::*;
use crate::whole_file::WholeFile;
use git2::{Oid, Repository};
use std::path::Path;

const WHOLE: LoadDiffOptions = LoadDiffOptions { ignore_whitespace: false, whole_file: true };

fn head(dir: &Path) -> Oid {
    Repository::open(dir).unwrap().head().unwrap().peel_to_commit().unwrap().id()
}

/// `n` numbered lines, with the line numbers in `changed` edited.
fn numbered(n: usize, changed: &[usize]) -> String {
    (1..=n).map(|i| if changed.contains(&i) { format!("changed {i}\n") } else { format!("line {i}\n") }).collect()
}

/// A repository with `file` committed as `text`.
fn committed(name: &str, file: &str, text: &str) -> std::path::PathBuf {
    let dir = scratch_repo(name);
    write(&dir, file, text);
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-m", "base"]);
    dir
}

fn whole(d: &FileDiff) -> &WholeFile {
    d.whole.as_deref().expect("whole file loaded")
}

/// `origin` and text of the changed lines in `lines`.
fn changes(lines: &[DiffLine]) -> Vec<(char, &str)> {
    lines.iter().filter(|l| matches!(l.origin, '+' | '-')).map(|l| (l.origin, l.text.as_str())).collect()
}

/// The whole hunk's blocks hold exactly the changed lines of their hunks, and no change lies
/// outside a block.
fn check_blocks(d: &FileDiff) {
    let w = whole(d);
    assert_eq!(w.diff.hunks.len(), 1);
    assert_eq!(w.blocks.len(), d.hunks.len());
    let lines = &w.diff.hunks[0].lines;
    for b in &w.blocks {
        assert_eq!(changes(&lines[b.start..b.end]), changes(&d.hunks[b.hunk].lines), "block {b:?}");
    }
    for (i, l) in lines.iter().enumerate() {
        if matches!(l.origin, '+' | '-') {
            assert!(w.blocks.iter().any(|b| (b.start..b.end).contains(&i)), "line {i} {l:?} is in a block");
        }
    }
}

/// The text of one side of the whole hunk: context plus `-` (old) or `+` (new) lines.
fn side(w: &WholeFile, origin: char) -> String {
    w.diff.hunks[0].lines.iter().filter(|l| l.origin == ' ' || l.origin == origin).map(|l| format!("{}\n", l.text)).collect()
}

#[test]
fn whole_file_unstaged_two_distant_changes() {
    let dir = committed("whole-two", "f.txt", &numbered(40, &[]));
    write(&dir, "f.txt", &numbered(40, &[3, 30]));
    let target = DiffTarget::Unstaged("f.txt".into());

    let plain = load_diff_with(&dir, &target, LoadDiffOptions::default()).unwrap();
    assert!(plain.whole.is_none());
    let d = load_diff_with(&dir, &target, WHOLE).unwrap();
    assert_eq!(d.hunks, plain.hunks, "the normal hunks are unchanged");
    assert_eq!(d.hunks.len(), 2);
    let w = whole(&d);
    assert!(w.diff.hunks[0].header.starts_with("@@ -1,40 +1,40 @@"), "{}", w.diff.hunks[0].header);
    assert_eq!(side(w, '+'), numbered(40, &[3, 30]));
    assert_eq!(side(w, '-'), numbered(40, &[]));
    assert_eq!(w.blocks.len(), 2);
    check_blocks(&d);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn whole_file_long_file_is_one_hunk() {
    let dir = committed("whole-long", "f.txt", &numbered(500, &[]));
    write(&dir, "f.txt", &numbered(500, &[10, 490]));
    let d = load_diff_with(&dir, &DiffTarget::Unstaged("f.txt".into()), WHOLE).unwrap();
    assert_eq!(d.hunks.len(), 2);
    assert_eq!(whole(&d).diff.hunks[0].lines.len(), 502);
    check_blocks(&d);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn whole_file_staged_commit_untracked_deleted_renamed() {
    let dir = committed("whole-targets", "f.txt", &numbered(20, &[]));
    write(&dir, "gone.txt", "a\nb\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-m", "gone"]);

    write(&dir, "f.txt", &numbered(20, &[2, 18]));
    git(&dir, &["add", "f.txt"]);
    let staged = load_diff_with(&dir, &DiffTarget::Staged("f.txt".into()), WHOLE).unwrap();
    check_blocks(&staged);
    git(&dir, &["commit", "-m", "edit"]);
    let commit = load_diff_with(&dir, &DiffTarget::Commit(head(&dir), "f.txt".into()), WHOLE).unwrap();
    check_blocks(&commit);
    assert_eq!(whole(&commit).blocks.len(), 2);

    // An untracked file is one block of added lines; a deleted file one of removed lines.
    write(&dir, "new.txt", "x\ny\nz\n");
    let new = load_diff_with(&dir, &DiffTarget::Unstaged("new.txt".into()), WHOLE).unwrap();
    check_blocks(&new);
    let w = whole(&new);
    assert_eq!(w.blocks.len(), 1);
    assert!(w.diff.hunks[0].lines.iter().all(|l| l.origin == '+'));
    std::fs::remove_file(dir.join("gone.txt")).unwrap();
    let gone = load_diff_with(&dir, &DiffTarget::Unstaged("gone.txt".into()), WHOLE).unwrap();
    check_blocks(&gone);
    assert!(whole(&gone).diff.hunks[0].lines.iter().all(|l| l.origin == '-'));

    // A rename with an edit, in a commit: the new path's diff maps its block.
    git(&dir, &["checkout", "--", "gone.txt"]);
    git(&dir, &["mv", "f.txt", "g.txt"]);
    write(&dir, "g.txt", &numbered(20, &[2, 18, 10]));
    git(&dir, &["add", "g.txt"]);
    git(&dir, &["commit", "-m", "rename"]);
    let renamed = load_diff_with(&dir, &DiffTarget::Commit(head(&dir), "g.txt".into()), WHOLE).unwrap();
    assert_eq!(changes(&renamed.hunks[0].lines), [('-', "line 10"), ('+', "changed 10")], "a rename, not a new file");
    check_blocks(&renamed);
    assert_eq!(whole(&renamed).blocks.len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn whole_file_no_newline_at_eof() {
    let dir = committed("whole-eofnl", "f.txt", &numbered(10, &[]));
    write(&dir, "f.txt", numbered(10, &[]).trim_end());
    let d = load_diff_with(&dir, &DiffTarget::Unstaged("f.txt".into()), WHOLE).unwrap();
    check_blocks(&d);
    let w = whole(&d);
    let lines = &w.diff.hunks[0].lines;
    assert_eq!(w.blocks[0].end, lines.len(), "the block runs to the end, past the marker");
    assert!(matches!(lines[lines.len() - 1].origin, '<' | '>'), "{lines:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn whole_file_with_ignore_whitespace() {
    let dir = committed("whole-ws", "f.txt", &numbered(20, &[]));
    // More space inside line 3 and trailing spaces on line 4 (-b ignores both), one real edit.
    write(&dir, "f.txt", &numbered(20, &[15]).replace("line 3\n", "line  3\n").replace("line 4\n", "line 4   \n"));
    let opts = LoadDiffOptions { ignore_whitespace: true, whole_file: true };
    let d = load_diff_with(&dir, &DiffTarget::Unstaged("f.txt".into()), opts).unwrap();
    assert_eq!(d.hunks.len(), 1);
    assert_eq!(changes(&d.hunks[0].lines), [('-', "line 15"), ('+', "changed 15")]);
    check_blocks(&d);
    let lines = &whole(&d).diff.hunks[0].lines;
    assert!(lines.iter().any(|l| l.origin == ' ' && l.text == "line  3"), "the re-spaced line is context");

    // Only whitespace changed: no hunks, so nothing to map.
    write(&dir, "f.txt", &numbered(20, &[]).replace("line 4\n", "line 4   \n"));
    let d = load_diff_with(&dir, &DiffTarget::Unstaged("f.txt".into()), opts).unwrap();
    assert!(d.hunks.is_empty() && d.whole.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn whole_file_binary_is_none() {
    let dir = committed("whole-bin", "b.bin", "a\0b\n");
    std::fs::write(dir.join("b.bin"), b"a\0c\n").unwrap();
    let d = load_diff_with(&dir, &DiffTarget::Unstaged("b.bin".into()), WHOLE).unwrap();
    assert!(d.binary && d.whole.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stage_by_block_hunk_index() {
    let dir = committed("whole-stage", "f.txt", &numbered(30, &[]));
    write(&dir, "f.txt", &numbered(30, &[3, 27]));
    let target = DiffTarget::Unstaged("f.txt".into());
    let d = load_diff_with(&dir, &target, WHOLE).unwrap();
    let second = whole(&d).blocks[1].hunk;
    apply_hunk(&dir, "f.txt", second, HunkOp::Stage).unwrap();

    let staged = load_diff(&dir, &DiffTarget::Staged("f.txt".into())).unwrap();
    let staged: Vec<_> = staged.hunks.iter().flat_map(|h| changes(&h.lines)).map(|(o, t)| format!("{o}{t}")).collect();
    assert_eq!(staged, ["-line 27", "+changed 27"]);
    let left = load_diff_with(&dir, &target, WHOLE).unwrap();
    assert_eq!(whole(&left).blocks.len(), 1);
    check_blocks(&left);
    let _ = std::fs::remove_dir_all(&dir);
}
