//! Counting other worktrees' changes: the index header, the load's limits, how a background
//! count reads a worktree, and that git runs there only when it may.

use super::*;
use crate::repo::status_cli_tests::marker_script;
use crate::repo::worktrees::wip_tests::{DIRTY, cleanup, dirty, linked, two_files};
use crate::testutil::*;

#[test]
fn index_entries_reads_the_header() {
    let dir = two_files("wtcount-header");
    assert_eq!(index_entries(&dir.join(".git").join("index")), Some(2));
    assert_eq!(index_entries(&dir.join(".git").join("no-such-index")), None);
    assert_eq!(index_entries(&dir.join("a.txt")), None, "not an index");
    cleanup(&[&dir]);
}

#[test]
fn a_load_counts_only_small_worktrees_within_its_budget() {
    let dir = two_files("wtcount-load");
    let wt = linked(&dir, "feat");
    dirty(&wt);
    let repo = Repository::open(&dir).unwrap();
    let listed = || super::super::worktrees::list(&repo);

    let mut small = listed();
    count_in_load(&mut small, &LoadLimits { max_entries: 1, ..LoadLimits::DEFAULT });
    assert_eq!((small[1].read, small[1].changes), (ChangesRead::Pending, ChangeCounts::default()), "its index is too large");

    let mut late = listed();
    count_in_load(&mut late, &LoadLimits { budget: Duration::ZERO, ..LoadLimits::DEFAULT });
    assert_eq!(late[1].read, ChangesRead::Pending, "the budget is spent");

    let mut all = listed();
    count_in_load(&mut all, &LoadLimits::DEFAULT);
    assert_eq!((all[1].read, all[1].changes), (ChangesRead::Counted, DIRTY));
    for list in [&small, &late, &all] {
        assert!(list[0].current);
        assert_eq!(list[0].read, ChangesRead::NotCounted, "the current worktree is never counted");
    }
    drop(repo);
    cleanup(&[&wt, &dir]);
}

#[test]
fn the_plan_follows_size_and_trust() {
    let large = super::super::status::LARGE_INDEX as u32 + 1;
    let cases = [
        // (entries, git_ok, wants_git) -> plan
        ((10, true, true), Plan::Git),
        ((10, true, false), Plan::Libgit2),
        ((10, false, true), Plan::Libgit2),
        ((large, true, false), Plan::Git),
        ((large, false, false), Plan::Libgit2),
        ((NO_GIT_COUNT_INDEX, false, true), Plan::Libgit2),
        ((NO_GIT_COUNT_INDEX + 1, false, true), Plan::TooLarge),
        ((NO_GIT_COUNT_INDEX + 1, true, false), Plan::Git),
    ];
    for ((entries, git_ok, wants_git), want) in cases {
        assert_eq!(plan(entries, git_ok, wants_git), want, "{entries} {git_ok} {wants_git}");
    }
}

#[test]
fn an_untrusted_worktree_is_counted_without_running_its_hooks() {
    let dir = two_files("wtcount-untrusted");
    let wt = linked(&dir, "feat");
    dirty(&wt);
    let marker = dir.join("ran.txt");
    let hook = marker_script(&dir.join(".git/fsmon.sh"), &marker);
    git(&dir, &["config", "core.fsmonitor", &hook]);

    let count = count_worktree(&wt, false, COUNT_TIMEOUT);
    assert_eq!((count.read, count.changes, &count.path), (ChangesRead::Counted, DIRTY, &wt));
    assert!(!marker.exists(), "no hook ran");
    cleanup(&[&wt, &dir]);
}

#[test]
fn a_trusted_worktree_is_counted_through_git() {
    let dir = two_files("wtcount-trusted");
    let wt = linked(&dir, "feat");
    dirty(&wt);
    git(&dir, &["config", "core.untrackedCache", "true"]);
    let count = count_worktree(&wt, true, COUNT_TIMEOUT);
    assert_eq!((count.read, count.changes), (ChangesRead::Counted, DIRTY));

    let plain = crate::paths::temp_dir().join(format!("trellis-wtcount-plain-{}", std::process::id()));
    std::fs::create_dir_all(&plain).unwrap();
    let none = count_worktree(&plain, true, COUNT_TIMEOUT);
    assert_eq!((none.read, none.changes), (ChangesRead::Counted, ChangeCounts::default()), "not a repository");
    cleanup(&[&wt, &dir, &plain]);
}
