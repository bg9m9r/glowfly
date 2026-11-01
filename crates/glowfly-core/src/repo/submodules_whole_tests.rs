//! The submodule list after a whole status read against the list that scans each submodule
//! itself: the same for every state (in sync, untracked or modified content, a staged change
//! inside, new commits with commits left behind, uninitialized, missing), read by libgit2 and by
//! git, under each `ignore` setting and `diff.ignoreSubmodules`; and how many scans and walks
//! the list makes on its own (none kept from a walk that can change without a ref moving).

use super::*;
use crate::repo::status_and_index;
use crate::repo::submodules_tests::{cleanup, lib_dir};
use crate::testutil::*;
use std::path::Path;

/// The list a whole read (by libgit2, then by git) leads to, with the memo that counts.
fn after_whole(parent: &Path, git_status: bool, memo: &SubmoduleMemo) -> Vec<SubmoduleInfo> {
    let (rows, ..) = status_and_index(parent, git_status).unwrap();
    submodules_if_any(parent, true, Some(&rows), Some(memo)).unwrap().expect("a .gitmodules is there")
}

/// Asserts the scanned list shows `want` and both whole reads lead to the same list.
fn same(parent: &Path, want: SubmoduleState, why: &str) {
    let old = submodules_at(parent).unwrap();
    assert_eq!(old.iter().map(|m| m.state).collect::<Vec<_>>(), vec![want], "{why}");
    for git_status in [false, true] {
        let new = after_whole(parent, git_status, &SubmoduleMemo::default());
        assert_eq!(new, old, "{why}, read by {}", if git_status { "git" } else { "libgit2" });
    }
}

/// Walks one submodule through every state, `setup` first setting what hides its changes.
/// `core.untrackedCache` makes the parent's status a read by git when git may read it.
fn every_state(name: &str, setup: impl FnOnce(&Path)) {
    let (parent, sub) = with_submodule(name);
    set_config(&parent, &[("core.untrackedCache", "true")]);
    setup(&parent);
    same(&parent, SubmoduleState::InSync, "in sync");
    write(&sub, "new.txt", "untracked\n");
    same(&parent, SubmoduleState::Dirty, "untracked content");
    std::fs::remove_file(sub.join("new.txt")).unwrap();
    write(&sub, "lib.txt", "changed\n");
    same(&parent, SubmoduleState::Dirty, "modified content");
    git(&sub, &["add", "lib.txt"]);
    same(&parent, SubmoduleState::Dirty, "a staged change inside");
    git(&sub, &["reset", "-q", "--hard"]);
    same(&parent, SubmoduleState::InSync, "clean again");

    git(&sub, &["checkout", "-q", "--detach"]);
    write(&sub, "lib.txt", "two\n");
    git(&sub, &["commit", "-q", "-am", "inside"]);
    same(&parent, SubmoduleState::OutOfSync, "new commits, one left behind");
    assert_eq!(submodules_at(&parent).unwrap()[0].left_behind.len(), 1);
    write(&sub, "new.txt", "untracked\n");
    same(&parent, SubmoduleState::DirtyOutOfSync, "new commits and untracked content");
    write(&sub, "lib.txt", "three\n");
    std::fs::remove_file(sub.join("new.txt")).unwrap();
    same(&parent, SubmoduleState::DirtyOutOfSync, "new commits and modified content");

    git(&parent, &["submodule", "deinit", "-q", "-f", "libs/lib"]);
    same(&parent, SubmoduleState::Uninitialized, "uninitialized");
    git(&parent, &["submodule", "update", "-q", "--init", "--", "libs/lib"]);
    same(&parent, SubmoduleState::InSync, "initialized again");
    std::fs::remove_dir_all(&sub).unwrap();
    same(&parent, SubmoduleState::Missing, "folder deleted");
    cleanup(&parent);
}

#[test]
fn whole_read_list_matches_the_scanned_list() {
    every_state("smw-none", |_| {});
}

#[test]
fn whole_read_list_matches_with_untracked_ignored() {
    every_state("smw-untracked", |p| git(p, &["config", "-f", ".gitmodules", "submodule.libs/lib.ignore", "untracked"]));
}

#[test]
fn whole_read_list_matches_with_dirty_ignored() {
    every_state("smw-dirty", |p| git(p, &["config", "-f", ".gitmodules", "submodule.libs/lib.ignore", "dirty"]));
}

#[test]
fn whole_read_list_matches_with_all_ignored() {
    every_state("smw-all", |p| git(p, &["config", "-f", ".gitmodules", "submodule.libs/lib.ignore", "all"]));
}

#[test]
fn whole_read_list_matches_with_ignore_in_the_config() {
    every_state("smw-config", |p| set_config(p, &[("submodule.libs/lib.ignore", "dirty")]));
}

#[test]
fn whole_read_list_matches_with_diff_ignore_submodules() {
    every_state("smw-diff", |p| set_config(p, &[("diff.ignoreSubmodules", "dirty")]));
}

#[test]
fn whole_read_leaves_each_submodule_to_its_one_scan() {
    let (parent, sub) = with_submodule("smw-scans");
    let memo = SubmoduleMemo::default();
    for git_status in [false, true] {
        after_whole(&parent, git_status, &memo);
    }
    write(&sub, "new.txt", "untracked\n");
    assert_eq!(after_whole(&parent, false, &memo)[0].state, SubmoduleState::Dirty);
    write(&sub, "lib.txt", "changed\n");
    assert_eq!(after_whole(&parent, false, &memo)[0].state, SubmoduleState::Dirty);
    assert_eq!(memo.scans(), 0, "the status read already scanned it");

    // Out of sync, the read's row says nothing of its files.
    git(&sub, &["commit", "-q", "-am", "inside"]);
    assert_eq!(after_whole(&parent, false, &memo)[0].state, SubmoduleState::DirtyOutOfSync);
    assert_eq!(memo.scans(), 1);
    git(&parent, &["add", "libs/lib"]);

    // What the read hid is scanned for.
    git(&parent, &["config", "-f", ".gitmodules", "submodule.libs/lib.ignore", "all"]);
    assert_eq!(after_whole(&parent, false, &memo)[0].state, SubmoduleState::Dirty);
    assert_eq!(memo.scans(), 2);
    cleanup(&parent);
}

#[test]
fn left_behind_is_walked_again_only_when_commits_or_refs_change() {
    let (parent, sub) = with_submodule("smw-walks");
    let memo = SubmoduleMemo::default();
    let left = |memo: &SubmoduleMemo| after_whole(&parent, false, memo)[0].left_behind.len();
    assert_eq!((left(&memo), memo.walks()), (0, 0), "in sync: nothing to walk");
    git(&sub, &["checkout", "-q", "--detach"]);
    write(&sub, "lib.txt", "two\n");
    git(&sub, &["commit", "-q", "-am", "first inside"]);
    assert_eq!((left(&memo), memo.walks()), (1, 1));
    write(&sub, "new.txt", "untracked\n");
    assert_eq!((left(&memo), memo.walks()), (1, 1), "the same commits and refs");

    git(&sub, &["commit", "-q", "--allow-empty", "-m", "second inside"]);
    assert_eq!((left(&memo), memo.walks()), (2, 2), "HEAD moved");
    git(&sub, &["tag", "keep"]);
    assert_eq!((left(&memo), memo.walks()), (0, 3), "a tag keeps them");
    git(&sub, &["tag", "-d", "keep"]);
    assert_eq!((left(&memo), memo.walks()), (2, 4));
    git(&parent, &["add", "libs/lib"]);
    assert_eq!((left(&memo), memo.walks()), (0, 4), "recorded: in sync");
    cleanup(&parent);
}

/// libgit2's read uses .gitmodules' rule only, so the config's `none` over it doesn't make that
/// read see everything.
#[test]
fn whole_read_list_matches_with_none_in_the_config_over_all() {
    every_state("smw-over-all", |p| {
        git(p, &["config", "-f", ".gitmodules", "submodule.libs/lib.ignore", "all"]);
        set_config(p, &[("submodule.libs/lib.ignore", "none")]);
    });
}

#[test]
fn whole_read_list_matches_with_none_in_the_config_over_untracked() {
    every_state("smw-over-untracked", |p| {
        git(p, &["config", "-f", ".gitmodules", "submodule.libs/lib.ignore", "untracked"]);
        set_config(p, &[("submodule.libs/lib.ignore", "none")]);
    });
}

#[test]
fn a_walk_without_the_recorded_commit_is_not_kept() {
    let (parent, _sub) = with_submodule("smw-unfetched");
    let lib = lib_dir(&parent);
    write(&lib, "lib.txt", "upstream\n");
    git(&lib, &["commit", "-q", "-am", "not fetched"]);
    let unfetched = git2::Repository::open(&lib).unwrap().head().unwrap().target().unwrap();
    git(&parent, &["update-index", "--cacheinfo", &format!("160000,{unfetched},libs/lib")]);
    let memo = SubmoduleMemo::default();
    for walks in 1..=2 {
        let sm = &after_whole(&parent, false, &memo)[0];
        assert_eq!((sm.state, sm.recorded), (SubmoduleState::OutOfSync, unfetched));
        assert_eq!(memo.walks(), walks, "walked again: the commit may arrive without a ref moving");
    }
    cleanup(&parent);
}
