//! Folder history: the commits that changed something under a folder, and the files each
//! changed there (`folder_history.rs`).

use super::*;
use crate::testutil::*;
use git2::Repository;
use std::path::Path;

fn head(dir: &Path) -> Oid {
    Repository::open(dir).unwrap().head().unwrap().target().unwrap()
}

/// Writes each of `files` (creating folders), then commits everything.
fn commit(dir: &Path, files: &[(&str, &str)], message: &str) -> Oid {
    for (file, text) in files {
        let path = dir.join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", message]);
    head(dir)
}

fn summaries(h: &FileHistory) -> Vec<&str> {
    h.entries.iter().map(|e| e.summary.as_str()).collect()
}

fn statuses(h: &FileHistory) -> String {
    h.entries.iter().map(|e| e.status).collect()
}

/// Each entry's files as "<status> <path>".
fn files(e: &HistoryEntry) -> Vec<String> {
    e.files.iter().map(|f| format!("{} {}", f.status, f.path)).collect()
}

fn lines(n: usize) -> String {
    (1..=n).map(|i| format!("line {i}\n")).collect()
}

#[test]
fn folder_history_lists_commits_touching_the_folder() {
    let dir = scratch_repo("folder-basic");
    commit(&dir, &[("src/a.rs", "a\n")], "add a");
    commit(&dir, &[("README.md", "readme\n")], "readme");
    commit(&dir, &[("src/sub/b.rs", "b\n")], "add b");
    commit(&dir, &[("src/a.rs", "a2\n"), ("src/c.rs", "c\n")], "edit a, add c");
    let h = folder_history(&dir, "src", &HistoryStart::Head, 100).unwrap();
    assert_eq!(summaries(&h), ["edit a, add c", "add b", "add a"]);
    assert_eq!(statuses(&h), "MMA");
    assert!(h.from_head && h.resume.is_empty() && h.on_disk);
    assert!(h.entries.iter().all(|e| e.path == "src" && e.old_path.is_none()));
    assert_eq!(files(&h.entries[0]), ["M src/a.rs", "A src/c.rs"]);
    assert_eq!(files(&h.entries[1]), ["A src/sub/b.rs"]);
    assert_eq!(files(&h.entries[2]), ["A src/a.rs"]);
    // A subfolder has its own history.
    let sub = folder_history(&dir, "src/sub", &HistoryStart::Head, 100).unwrap();
    assert_eq!(summaries(&sub), ["add b"]);
    assert_eq!(statuses(&sub), "A");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn folder_prefix_is_exact() {
    let dir = scratch_repo("folder-prefix");
    commit(&dir, &[("src/a.rs", "a\n")], "add a");
    commit(&dir, &[("src2/x", "x\n")], "add src2");
    commit(&dir, &[("srcfile", "f\n")], "add srcfile");
    commit(&dir, &[("src/a.rs", "a2\n"), ("src2/x", "x2\n"), ("srcfile", "f2\n")], "edit all");
    let h = folder_history(&dir, "src", &HistoryStart::Head, 100).unwrap();
    assert_eq!(summaries(&h), ["edit all", "add a"]);
    assert_eq!(files(&h.entries[0]), ["M src/a.rs"]);
    assert!(h.entries.iter().flat_map(|e| &e.files).all(|f| f.path.starts_with("src/")));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn folder_rename_inside_is_r_and_moves_in_or_out_are_a_or_d() {
    let dir = scratch_repo("folder-moves");
    let (b, c) = (lines(12).replace("line", "bee"), lines(12).replace("line", "sea"));
    commit(&dir, &[("src/a.rs", &lines(10)), ("src/b.rs", &b), ("lib/c.rs", &c)], "base");
    git(&dir, &["mv", "src/a.rs", "src/renamed.rs"]);
    git(&dir, &["commit", "-q", "-m", "rename inside"]);
    git(&dir, &["mv", "src/b.rs", "lib/b.rs"]);
    git(&dir, &["mv", "lib/c.rs", "src/c.rs"]);
    git(&dir, &["commit", "-q", "-m", "move out and in"]);
    let h = folder_history(&dir, "src", &HistoryStart::Head, 100).unwrap();
    assert_eq!(summaries(&h), ["move out and in", "rename inside", "base"]);
    assert_eq!(files(&h.entries[0]), ["D src/b.rs", "A src/c.rs"]);
    assert_eq!(files(&h.entries[1]), ["R src/renamed.rs"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn folder_history_skips_a_merge_equal_to_a_parent() {
    let dir = scratch_repo("folder-merge");
    commit(&dir, &[("src/a.rs", "a\n")], "base");
    git(&dir, &["checkout", "-q", "-b", "side"]);
    commit(&dir, &[("src/a.rs", "side\n")], "side edit");
    git(&dir, &["checkout", "-q", "main"]);
    commit(&dir, &[("other.txt", "o\n")], "main edit");
    git(&dir, &["merge", "-q", "--no-ff", "side", "-m", "merge side"]);
    let h = folder_history(&dir, "src", &HistoryStart::Head, 100).unwrap();
    assert_eq!(summaries(&h), ["side edit", "base"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn folder_history_batches_and_resumes() {
    let dir = scratch_repo("folder-batches");
    for i in 0..5 {
        commit(&dir, &[("src/f.txt", &format!("{i}\n"))], &format!("commit {i}"));
    }
    let full = folder_history(&dir, "src", &HistoryStart::Head, 100).unwrap();
    assert_eq!(full.entries.len(), 5);
    let part = folder_history(&dir, "src", &HistoryStart::Head, 2).unwrap();
    assert_eq!(part.entries.len(), 2);
    assert!(part.searched_until.is_some() && !part.resume.is_empty());
    let rest = folder_history(&dir, "src", &HistoryStart::Resume(part.resume.clone()), 100).unwrap();
    assert!(rest.resume.is_empty());
    let mut all: Vec<Oid> = part.entries.iter().map(|e| e.oid).collect();
    all.extend(rest.entries.iter().map(|e| e.oid));
    assert_eq!(all, full.entries.iter().map(|e| e.oid).collect::<Vec<_>>(), "no duplicates, nothing missed");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn folder_history_of_a_removed_folder_starts_with_d() {
    let dir = scratch_repo("folder-removed");
    commit(&dir, &[("gone/a.txt", "a\n"), ("keep.txt", "k\n")], "add gone");
    let edit = commit(&dir, &[("gone/a.txt", "a2\n")], "edit gone");
    git(&dir, &["rm", "-q", "-r", "gone"]);
    git(&dir, &["commit", "-q", "-m", "remove gone"]);
    let removed = head(&dir);

    // HEAD is the commit that removes the folder: it comes first, as 'D'.
    let h = folder_history(&dir, "gone", &HistoryStart::Head, 100).unwrap();
    assert_eq!(statuses(&h), "DMA");
    assert_eq!(h.entries[0].oid, removed);
    assert_eq!(files(&h.entries[0]), ["D gone/a.txt"]);

    commit(&dir, &[("keep.txt", "k2\n")], "later");
    assert!(folder_history(&dir, "gone", &HistoryStart::Head, 100).unwrap().entries.is_empty());
    // Asked about a commit that has it, while HEAD doesn't: the walk starts there.
    let h = folder_history(&dir, "gone", &HistoryStart::At(edit), 100).unwrap();
    assert!(!h.from_head && h.head_path.is_none() && !h.on_disk);
    assert_eq!(summaries(&h), ["edit gone", "add gone"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn empty_folder_name_is_rejected() {
    let dir = scratch_repo("folder-empty");
    commit(&dir, &[("a.txt", "a\n")], "add a");
    assert!(matches!(folder_history(&dir, "", &HistoryStart::Head, 100), Err(crate::error::Error::Unsupported(_))));
    // A file is not a folder.
    assert!(folder_history(&dir, "a.txt", &HistoryStart::Head, 100).unwrap().entries.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Distinct text for a file, long enough for rename detection to pair it.
fn text(name: &str) -> String {
    lines(12).replace("line", name)
}

#[test]
fn a_renamed_folder_is_followed_backwards() {
    let dir = scratch_repo("folder-renamed");
    let (a, b, c) = (text("a"), text("b"), text("c"));
    commit(&dir, &[("src/a.rs", &a), ("src/b.rs", &b), ("src/c.rs", &c), ("README", "r\n")], "base");
    commit(&dir, &[("src/a.rs", &format!("{a}more\n"))], "edit src");
    git(&dir, &["mv", "src", "lib"]);
    git(&dir, &["commit", "-q", "-m", "move to lib"]);
    commit(&dir, &[("lib/b.rs", &format!("{b}more\n"))], "edit lib");
    let h = folder_history(&dir, "lib", &HistoryStart::Head, 100).unwrap();
    assert_eq!(summaries(&h), ["edit lib", "move to lib", "edit src", "base"]);
    assert_eq!(statuses(&h), "MRMA");
    assert_eq!(h.entries[1].old_path.as_deref(), Some("src"));
    assert_eq!(files(&h.entries[1]), ["R lib/a.rs", "R lib/b.rs", "R lib/c.rs"]);
    let paths: Vec<&str> = h.entries.iter().map(|e| e.path.as_str()).collect();
    assert_eq!(paths, ["lib", "lib", "src", "src"]);
    assert_eq!(files(&h.entries[2]), ["M src/a.rs"]);
    assert!(h.entries.iter().filter(|e| e.status != 'R').all(|e| e.old_path.is_none()));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_folder_moved_into_a_subfolder_is_followed() {
    let dir = scratch_repo("folder-into-sub");
    commit(&dir, &[("pkg/a.rs", &text("a")), ("pkg/b.rs", &text("b"))], "base");
    std::fs::create_dir_all(dir.join("crates")).unwrap();
    git(&dir, &["mv", "pkg", "crates/pkg"]);
    git(&dir, &["commit", "-q", "-m", "into crates"]);
    let h = folder_history(&dir, "crates/pkg", &HistoryStart::Head, 100).unwrap();
    assert_eq!(summaries(&h), ["into crates", "base"]);
    assert_eq!(statuses(&h), "RA");
    assert_eq!(h.entries[0].old_path.as_deref(), Some("pkg"));
    assert_eq!(h.entries[1].path, "pkg");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_folder_with_a_minority_of_moved_files_starts_there() {
    let dir = scratch_repo("folder-minority");
    commit(&dir, &[("old/x.rs", &text("x")), ("old/y.rs", &text("y"))], "base");
    commit(&dir, &[("old/x.rs", &format!("{}more\n", text("x")))], "edit old");
    std::fs::create_dir_all(dir.join("new")).unwrap();
    git(&dir, &["mv", "old/x.rs", "new/x.rs"]);
    commit(&dir, &[("new/p.rs", &text("p")), ("new/q.rs", &text("q")), ("new/r.rs", &text("r"))], "make new");
    let h = folder_history(&dir, "new", &HistoryStart::Head, 100).unwrap();
    assert_eq!(summaries(&h), ["make new"]);
    assert_eq!(statuses(&h), "A");
    assert_eq!(h.entries[0].old_path, None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_merge_that_moved_the_folder_against_one_parent_follows_both() {
    let dir = scratch_repo("folder-merge-moved");
    let a = text("a");
    commit(&dir, &[("src/a.rs", &a), ("src/b.rs", &text("b")), ("src/c.rs", &text("c"))], "base");
    git(&dir, &["checkout", "-q", "-b", "side"]);
    commit(&dir, &[("src/a.rs", &format!("{a}side\n"))], "side edit");
    git(&dir, &["checkout", "-q", "main"]);
    git(&dir, &["mv", "src", "lib"]);
    git(&dir, &["commit", "-q", "-m", "move to lib"]);
    git(&dir, &["merge", "-q", "--no-ff", "side", "-m", "merge side"]);
    let h = folder_history(&dir, "lib", &HistoryStart::Head, 100).unwrap();
    assert_eq!(h.entries[0].summary, "merge side");
    assert_eq!(h.entries[0].status, 'M', "the first parent has the folder");
    assert_eq!(files(&h.entries[0]), ["M lib/a.rs"]);
    let mut rest: Vec<&str> = summaries(&h)[1..].to_vec();
    rest.sort();
    assert_eq!(rest, ["base", "move to lib", "side edit"], "both parents are followed");
    let side = h.entries.iter().find(|e| e.summary == "side edit").unwrap();
    assert_eq!(side.path, "src");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn absent_lists_folder_files_missing_on_disk_by_their_names_today() {
    let dir = scratch_repo("folder-absent");
    let x = text("x");
    commit(&dir, &[("src/a.rs", &text("a")), ("src/b.rs", &text("b")), ("src/x.rs", &x)], "base");
    commit(&dir, &[("src/x.rs", &format!("{x}more\n"))], "edit x");
    git(&dir, &["mv", "src", "lib"]);
    git(&dir, &["commit", "-q", "-m", "move to lib"]);
    commit(&dir, &[("lib/old.rs", "old\n")], "add old");
    git(&dir, &["rm", "-q", "lib/old.rs", "lib/x.rs"]);
    git(&dir, &["commit", "-q", "-m", "remove old and x"]);
    let h = folder_history(&dir, "lib", &HistoryStart::Head, 100).unwrap();
    let mut absent: Vec<&str> = h.absent.iter().map(String::as_str).collect();
    absent.sort();
    assert_eq!(absent, ["lib/old.rs", "lib/x.rs"]);
    assert!(file_history(&dir, "lib/a.rs", &HistoryStart::Head, 100).unwrap().absent.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn folder_file_today_moves_only_files_inside_the_folder() {
    assert_eq!(folder_file_today("src", "lib", "src/a/b.rs"), "lib/a/b.rs");
    assert_eq!(folder_file_today("lib", "lib", "lib/a.rs"), "lib/a.rs");
    assert_eq!(folder_file_today("src", "lib", "other/x.rs"), "other/x.rs");
    assert_eq!(folder_file_today("src", "lib", "srcx/y.rs"), "srcx/y.rs");
}
