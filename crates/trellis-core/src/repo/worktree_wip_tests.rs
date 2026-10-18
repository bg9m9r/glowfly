//! Other worktrees' uncommitted changes against real repositories: counted by full loads (libgit2
//! only, also while untrusted) for every worktree but the current one, and laid out as a WIP row
//! above the worktree's HEAD commit; skipped for clean and missing worktrees, with the option
//! off, and by `worktrees_at`; no row for a HEAD outside the loaded history. The helpers serve
//! `worktree_counts`' tests too.

use crate::repo::status_cli_tests::marker_script;
use crate::repo::*;
use crate::testutil::*;
use git2::{Oid, Repository};
use std::path::{Path, PathBuf};

/// A repository with a.txt and d.txt committed on main.
pub(in crate::repo) fn two_files(name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    write(&dir, "a.txt", "one\n");
    write(&dir, "d.txt", "doomed\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    dir
}

/// A sibling folder for a linked worktree of `main`, with branch `branch` checked out there.
pub(in crate::repo) fn linked(main: &Path, branch: &str) -> PathBuf {
    let p = main.with_file_name(format!("{}-wt-{branch}", main.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&p);
    git(main, &["worktree", "add", "-q", "-b", branch, &p.display().to_string()]);
    p
}

/// One modified, two added (untracked and staged) and one deleted file in `wt`.
pub(in crate::repo) fn dirty(wt: &Path) {
    write(wt, "a.txt", "one\ntwo\n");
    std::fs::remove_file(wt.join("d.txt")).unwrap();
    write(wt, "b.txt", "untracked\n");
    write(wt, "c.txt", "staged\n");
    git(wt, &["add", "c.txt"]);
}

pub(in crate::repo) const DIRTY: ChangeCounts = ChangeCounts { modified: 1, added: 2, deleted: 1 };

fn tip(dir: &Path, branch: &str) -> Oid {
    Repository::open(dir).unwrap().refname_to_id(&format!("refs/heads/{branch}")).unwrap()
}

pub(in crate::repo) fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn changes_in_a_linked_worktree_are_counted() {
    let dir = two_files("wtwip-counted");
    let wt = linked(&dir, "feat");
    dirty(&wt);
    let snap = load(&dir, &LoadOptions::all(100)).unwrap();
    assert_eq!(snap.worktrees.len(), 2);
    assert!(snap.worktrees[0].current);
    assert_eq!(snap.worktrees[0].changes, ChangeCounts::default(), "the current worktree is not counted");
    assert_eq!(snap.worktrees[1].changes, DIRTY);
    assert_eq!(snap.worktrees[0].read, ChangesRead::NotCounted);
    assert_eq!(snap.worktrees[1].read, ChangesRead::Counted);
    let rows: Vec<(usize, usize)> = snap.graph.worktree_wips.iter().map(|w| (w.worktree, w.before)).collect();
    assert_eq!(rows, [(1, snap.row_of[&tip(&dir, "feat")])]);
    assert_eq!(worktree_wip_heads(&snap.worktrees), [(1, tip(&dir, "feat"))]);
    cleanup(&[&wt, &dir]);
}

#[test]
fn a_clean_worktree_gets_no_row() {
    let dir = two_files("wtwip-clean");
    let wt = linked(&dir, "feat");
    let snap = load(&dir, &LoadOptions::all(100)).unwrap();
    assert!(snap.worktrees[1].changes.is_empty());
    assert_eq!(snap.worktrees[1].read, ChangesRead::Counted, "counted, and clean");
    assert!(snap.graph.worktree_wips.is_empty());
    cleanup(&[&wt, &dir]);
}

#[test]
fn loading_from_a_linked_worktree_counts_the_main_one() {
    let dir = two_files("wtwip-from-linked");
    let wt = linked(&dir, "feat");
    write(&dir, "a.txt", "changed in main\n");
    let snap = load(&wt, &LoadOptions::all(100)).unwrap();
    let (main, here) = (&snap.worktrees[0], &snap.worktrees[1]);
    assert!(main.is_main() && !main.current && here.current);
    assert_eq!(main.changes, ChangeCounts { modified: 1, added: 0, deleted: 0 });
    assert!(here.changes.is_empty());
    assert_eq!((main.read, here.read), (ChangesRead::Counted, ChangesRead::NotCounted));
    let rows: Vec<(usize, usize)> = snap.graph.worktree_wips.iter().map(|w| (w.worktree, w.before)).collect();
    assert_eq!(rows, [(0, snap.row_of[&tip(&dir, "main")])]);
    cleanup(&[&wt, &dir]);
}

#[test]
fn missing_worktrees_and_the_option_off_are_skipped() {
    let dir = two_files("wtwip-skipped");
    let gone = linked(&dir, "gone");
    std::fs::remove_dir_all(&gone).unwrap();
    let wt = linked(&dir, "feat");
    dirty(&wt);

    let snap = load(&dir, &LoadOptions::all(100)).unwrap();
    let missing = snap.worktrees.iter().find(|w| w.branch.as_deref() == Some("gone")).unwrap();
    assert!(missing.missing && missing.changes.is_empty());
    assert_eq!(missing.read, ChangesRead::NotCounted);
    assert_eq!(snap.graph.worktree_wips.len(), 1, "only feat's row");

    let off = load(&dir, &LoadOptions { worktree_wips: false, ..LoadOptions::all(100) }).unwrap();
    assert!(off.worktrees.iter().all(|w| w.changes.is_empty() && w.read == ChangesRead::NotCounted));
    assert!(off.graph.worktree_wips.is_empty());
    assert!(
        worktrees_at(&dir).unwrap().iter().all(|w| w.changes.is_empty() && w.read == ChangesRead::NotCounted),
        "the cheap listing never counts"
    );
    cleanup(&[&wt, &gone, &dir]);
}

#[test]
fn a_head_outside_the_loaded_history_gets_no_row() {
    let dir = two_files("wtwip-hidden");
    let wt = linked(&dir, "feat");
    write(&wt, "f.txt", "feature\n");
    git(&wt, &["add", "."]);
    git(&wt, &["commit", "-q", "-m", "feature work"]);
    dirty(&wt);
    let opts = LoadOptions { max_commits: 100, hidden: vec!["refs/heads/feat".into()], worktree_wips: true, ..Default::default() };
    let snap = load(&dir, &opts).unwrap();
    assert_eq!(snap.worktrees[1].changes, DIRTY);
    assert!(!snap.row_of.contains_key(&tip(&dir, "feat")));
    assert!(snap.graph.worktree_wips.is_empty());
    cleanup(&[&wt, &dir]);
}

#[test]
fn counted_while_untrusted() {
    let dir = two_files("wtwip-untrusted");
    let wt = linked(&dir, "feat");
    dirty(&wt);
    let marker = dir.join("ran.txt");
    let hook = marker_script(&dir.join(".git/fsmon.sh"), &marker);
    git(&dir, &["config", "core.fsmonitor", &hook]);

    let snap = load(&dir, &LoadOptions { trusted: false, ..LoadOptions::all(100) }).unwrap();
    assert!(!snap.trust_scan.is_empty(), "the scan finds the hook");
    assert_eq!(snap.worktrees[1].changes, DIRTY);
    assert_eq!(snap.graph.worktree_wips.len(), 1);
    assert!(!marker.exists(), "no hook ran");
    cleanup(&[&wt, &dir]);
}
