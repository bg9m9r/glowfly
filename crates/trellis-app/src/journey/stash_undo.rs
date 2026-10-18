//! Stash and pop from the toolbar, then undo and redo a commit and a merge: HEAD moves back and
//! forth between the same commits, and local changes are kept.

use super::find::{Pick, Region};
use super::fixture::{self as disk, Fixture, commit_files, git, write};
use super::harness::Journey;

#[test]
fn stash_pop_and_undo_redo_commit_and_merge() {
    let fx = Fixture::new("stash-undo");
    let repo = fx.work().join("app");
    fx.init(&repo, &[("a.txt", "a\n")]);
    git(&repo, &["checkout", "-q", "-b", "side"]);
    commit_files(&repo, &[("side.txt", "side\n")], "Add side");
    let side_tip = disk::head_oid(&repo);
    git(&repo, &["checkout", "-q", "main"]);
    commit_files(&repo, &[("b.txt", "b\n")], "Main moves on");
    let mut j = Journey::start(fx, Some(repo.clone()), vec![repo.clone()]);
    j.see("✔  main", Region::Sidebar);
    j.disabled("Pop the latest stash", Region::Toolbar);

    // step 1: stash a local edit.
    write(&repo, "a.txt", "a edited\n");
    j.refresh();
    j.click("Stash all changes", Region::Toolbar, Pick::Only);
    j.toast(true, "Stash succeeded");
    assert!(disk::is_clean(&repo));
    assert_eq!(disk::stash_count(&repo), 1);
    j.see_contains("WIP on main", Region::Sidebar);
    j.enabled("Pop the latest stash", Region::Toolbar);

    // step 2: pop it back.
    j.click("Pop the latest stash", Region::Toolbar, Pick::Only);
    j.toast(true, "Pop stash succeeded");
    assert_eq!(disk::work_text(&repo, "a.txt").as_deref(), Some("a edited\n"));
    assert_eq!(disk::stash_count(&repo), 0);
    assert_eq!(j.count_containing("WIP on main", Region::Sidebar), 0);
    j.disabled("Pop the latest stash", Region::Toolbar);

    // step 3: commit it, undo the commit (the edit stays), redo it.
    let before = disk::head_oid(&repo);
    j.commit_all("Edit a");
    let commit = disk::head_oid(&repo);
    assert_eq!(disk::parents(&repo, commit), [before]);
    j.click("Undo the last action", Region::Toolbar, Pick::Only);
    j.toast(true, "Undo commit 'Edit a'");
    assert_eq!(disk::head_oid(&repo), before);
    assert_eq!(disk::work_text(&repo, "a.txt").as_deref(), Some("a edited\n"));
    assert!(!disk::is_clean(&repo), "the change is back in the working tree");
    j.enabled("Redo the undone action", Region::Toolbar);
    j.click("Redo the undone action", Region::Toolbar, Pick::Only);
    j.toast(true, "Redo commit 'Edit a'");
    assert_eq!(disk::head_oid(&repo), commit);
    assert!(disk::is_clean(&repo));

    // step 4: merge side, undo the merge, redo it: the same merge commit comes back.
    j.branch_menu("side", "Merge side into main");
    j.toast(true, "Merge succeeded");
    let merge = disk::head_oid(&repo);
    assert_eq!(disk::parents(&repo, merge), [commit, side_tip]);
    assert!(disk::work_text(&repo, "side.txt").is_some());
    j.click("Undo the last action", Region::Toolbar, Pick::Only);
    j.toast(true, "Undo");
    assert_eq!(disk::head_oid(&repo), commit);
    assert!(disk::work_text(&repo, "side.txt").is_none());
    j.click("Redo the undone action", Region::Toolbar, Pick::Only);
    j.toast(true, "Redo");
    assert_eq!(disk::head_oid(&repo), merge);
    assert_eq!(disk::work_text(&repo, "side.txt").as_deref(), Some("side\n"));
    assert!(disk::is_clean(&repo));
    j.see("main", Region::Toolbar);
}
