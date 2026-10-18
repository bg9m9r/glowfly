//! File history opened at an older commit: following the file forwards through later renames
//! to its name at HEAD (`history_forward.rs`), and the `Moved` start.

use super::*;
use crate::testutil::*;
use git2::Repository;
use std::path::Path;

fn head(dir: &Path) -> Oid {
    Repository::open(dir).unwrap().head().unwrap().target().unwrap()
}

fn commit(dir: &Path, file: &str, text: &str, message: &str) -> Oid {
    write(dir, file, text);
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", message]);
    head(dir)
}

fn summaries(h: &FileHistory) -> Vec<&str> {
    h.entries.iter().map(|e| e.summary.as_str()).collect()
}

fn lines(n: usize) -> String {
    (1..=n).map(|i| format!("line {i}\n")).collect()
}

/// `git mv from to`, then commits `to` with `text`.
fn rename(dir: &Path, from: &str, to: &str, text: &str, message: &str) -> Oid {
    git(dir, &["mv", from, to]);
    commit(dir, to, text, message)
}

#[test]
fn history_at_a_commit_before_a_rename_starts_from_head_under_the_new_name() {
    let dir = scratch_repo("forward-basic");
    let c1 = commit(&dir, "old.txt", &lines(10), "add old");
    commit(&dir, "old.txt", &lines(10).replace("line 3\n", "line three\n"), "edit old");
    let text = lines(10).replace("line 3\n", "line three\n").replace("line 8\n", "line eight\n");
    rename(&dir, "old.txt", "new.txt", &text, "rename");
    commit(&dir, "new.txt", &text.replace("line 1\n", "line one\n"), "edit new");

    let h = file_history(&dir, "old.txt", &HistoryStart::At(c1), 100).unwrap();
    assert!(h.from_head);
    assert_eq!(h.head_path.as_deref(), Some("new.txt"));
    assert_eq!(summaries(&h), ["edit new", "rename", "edit old", "add old"]);
    assert_eq!(h.entries[0].path, "new.txt");
    assert_eq!(h.entries[3].oid, c1);
    assert!(h.on_disk, "on_disk is about the name at HEAD");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn forward_tracking_follows_a_chain_of_renames() {
    let dir = scratch_repo("forward-chain");
    let c1 = commit(&dir, "a", &lines(12), "add a");
    rename(&dir, "a", "b", &lines(12).replace("line 2\n", "two\n"), "a to b");
    rename(&dir, "b", "c", &lines(12).replace("line 2\n", "two\n").replace("line 9\n", "nine\n"), "b to c");
    let h = file_history(&dir, "a", &HistoryStart::At(c1), 100).unwrap();
    assert!(h.from_head);
    assert_eq!(h.head_path.as_deref(), Some("c"));
    assert_eq!(summaries(&h), ["b to c", "a to b", "add a"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn forward_tracking_through_a_merged_branch() {
    let dir = scratch_repo("forward-merge");
    let base = commit(&dir, "f.txt", &lines(10), "base");
    git(&dir, &["checkout", "-q", "-b", "side"]);
    rename(&dir, "f.txt", "g.txt", &lines(10).replace("line 4\n", "four\n"), "rename on side");
    git(&dir, &["checkout", "-q", "main"]);
    commit(&dir, "other.txt", "x\n", "main edit");
    git(&dir, &["merge", "-q", "--no-ff", "side", "-m", "merge side"]);
    let h = file_history(&dir, "f.txt", &HistoryStart::At(base), 100).unwrap();
    assert!(h.from_head);
    assert_eq!(h.head_path.as_deref(), Some("g.txt"));
    assert_eq!(summaries(&h), ["rename on side", "base"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_file_deleted_after_the_commit_falls_back_to_as_of() {
    let dir = scratch_repo("forward-deleted");
    commit(&dir, "d.txt", "1\n", "add d");
    let edit = commit(&dir, "d.txt", "2\n", "edit d");
    git(&dir, &["rm", "-q", "d.txt"]);
    git(&dir, &["commit", "-q", "-m", "remove d"]);
    let h = file_history(&dir, "d.txt", &HistoryStart::At(edit), 100).unwrap();
    assert!(!h.from_head && h.head_path.is_none());
    assert_eq!(h.entries[0].oid, edit);
    assert_eq!(summaries(&h), ["edit d", "add d"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_commit_not_below_head_keeps_its_own_history() {
    let dir = scratch_repo("forward-side");
    commit(&dir, "f.txt", &lines(10), "base");
    git(&dir, &["checkout", "-q", "-b", "side"]);
    let side = commit(&dir, "f.txt", &lines(10).replace("line 1\n", "one\n"), "side edit");
    git(&dir, &["checkout", "-q", "main"]);
    rename(&dir, "f.txt", "g.txt", &lines(10).replace("line 9\n", "nine\n"), "rename on main");
    let h = file_history(&dir, "f.txt", &HistoryStart::At(side), 100).unwrap();
    assert!(!h.from_head && h.head_path.is_none());
    assert_eq!(summaries(&h), ["side edit", "base"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn moved_start_follows_a_rename_committed_since() {
    let dir = scratch_repo("forward-moved");
    commit(&dir, "a.txt", &lines(10), "add a");
    let old_head = commit(&dir, "a.txt", &lines(10).replace("line 5\n", "five\n"), "edit a");
    rename(&dir, "a.txt", "b.txt", &lines(10).replace("line 5\n", "five\n").replace("line 6\n", "six\n"), "rename");
    let h = file_history(&dir, "a.txt", &HistoryStart::Moved(old_head), 100).unwrap();
    assert!(h.from_head);
    assert_eq!(h.head_path.as_deref(), Some("b.txt"));
    assert_eq!(summaries(&h), ["rename", "edit a", "add a"]);
    // Still under its name: the same as a HEAD start.
    let moved = file_history(&dir, "b.txt", &HistoryStart::Moved(head(&dir)), 100).unwrap();
    assert_eq!(moved, file_history(&dir, "b.txt", &HistoryStart::Head, 100).unwrap());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn moved_start_off_the_line_is_plain_head() {
    let dir = scratch_repo("forward-moved-off");
    commit(&dir, "a.txt", "1\n", "add a");
    git(&dir, &["checkout", "-q", "-b", "side"]);
    let side = commit(&dir, "a.txt", "side\n", "side edit");
    git(&dir, &["checkout", "-q", "main"]);
    commit(&dir, "a.txt", "2\n", "main edit");
    let moved = file_history(&dir, "a.txt", &HistoryStart::Moved(side), 100).unwrap();
    assert!(moved.from_head && moved.head_path.is_none());
    assert_eq!(moved, file_history(&dir, "a.txt", &HistoryStart::Head, 100).unwrap());
    // A commit that no longer exists (gone after an amend and a gc) is not below HEAD either.
    let gone = Oid::from_bytes(&[7; 20]).unwrap();
    let moved = file_history(&dir, "a.txt", &HistoryStart::Moved(gone), 100).unwrap();
    assert_eq!(moved, file_history(&dir, "a.txt", &HistoryStart::Head, 100).unwrap());
    let _ = std::fs::remove_dir_all(&dir);
}
