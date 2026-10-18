//! Create a branch from the toolbar, commit on it, check out main and diverge, merge the branch
//! with a merge commit, delete it through the confirm dialog, then check out with local
//! changes: carried over when they don't collide, refused when they do.

use super::find::{Pick, Region};
use super::fixture::{self as disk, Fixture, write};
use super::harness::Journey;

/// Creates branch `name` from the toolbar's Branch button (checked out after creating).
fn new_branch(j: &mut Journey, name: &str) {
    j.click("Create a branch", Region::Toolbar, Pick::Only);
    j.type_into("Branch name", name);
    j.click("Create", Region::Anywhere, Pick::Only);
    j.toast(true, "Create branch succeeded");
}

#[test]
fn branch_merge_delete_and_checkout_with_changes() {
    let fx = Fixture::new("branches");
    let (bare, _seed) = fx.seeded_remote("app", &[("a.txt", "a\n"), ("b.txt", "b\n")]);
    let repo = fx.work().join("app");
    fx.clone_cli(&bare, &repo);
    let mut j = Journey::start(fx, Some(repo.clone()), vec![repo.clone()]);
    j.see("✔  main", Region::Sidebar);

    // step 1: a new branch from the toolbar, checked out.
    new_branch(&mut j, "feature");
    j.see("feature", Region::Toolbar);
    j.see("✔  feature", Region::Sidebar);
    assert_eq!(disk::head_branch(&repo).as_deref(), Some("feature"));

    // step 2: a commit on it.
    write(&repo, "feature.txt", "feature\n");
    j.commit_all("Add feature");
    let feature_tip = disk::head_oid(&repo);
    assert_eq!(disk::summary(&repo, feature_tip), "Add feature");
    assert_eq!(disk::branch_oid(&repo, "feature"), Some(feature_tip));

    // step 3: back to main from the branch's menu.
    j.branch_menu("main", "Checkout main");
    j.toast(true, "Checkout succeeded");
    j.see("main", Region::Toolbar);
    j.see("✔  main", Region::Sidebar);
    assert_eq!(disk::head_branch(&repo).as_deref(), Some("main"));
    assert!(disk::work_text(&repo, "feature.txt").is_none());

    // step 4: main moves on, so the two diverge.
    write(&repo, "main.txt", "main\n");
    j.commit_all("Main change");
    let old_main = disk::head_oid(&repo);
    assert_eq!(disk::parents(&repo, old_main).len(), 1);

    // step 5: merging feature makes a merge commit with both files.
    j.branch_menu("feature", "Merge feature into main");
    j.toast(true, "Merge succeeded");
    let merge = disk::head_oid(&repo);
    assert_eq!(disk::parents(&repo, merge), [old_main, feature_tip]);
    assert_eq!(disk::work_text(&repo, "feature.txt").as_deref(), Some("feature\n"));
    assert_eq!(disk::work_text(&repo, "main.txt").as_deref(), Some("main\n"));
    assert!(disk::is_clean(&repo));
    j.see_contains("Merge branch 'feature'", Region::Central);

    // step 6: delete the merged branch through the confirm dialog.
    j.branch_menu("feature", "Delete feature");
    j.see("Delete local branch feature?", Region::Anywhere);
    j.click("Delete", Region::Anywhere, Pick::Last);
    j.toast(true, "Delete branch succeeded");
    assert_eq!(disk::branch_oid(&repo, "feature"), None);
    j.absent("feature", Region::Sidebar);
    j.see("✔  main", Region::Sidebar);

    // step 7: another branch with its own change to a.txt; back on main.
    new_branch(&mut j, "other");
    write(&repo, "a.txt", "a from other\n");
    j.commit_all("Change a on other");
    j.branch_menu("main", "Checkout main");
    j.toast(true, "Checkout succeeded");
    assert_eq!(disk::work_text(&repo, "a.txt").as_deref(), Some("a\n"));

    // step 8: a local edit the checkout doesn't touch is carried over.
    write(&repo, "b.txt", "b edited\n");
    j.refresh();
    j.branch_menu("other", "Checkout other");
    j.toast(true, "Checkout succeeded");
    j.see("other", Region::Toolbar);
    assert_eq!(disk::head_branch(&repo).as_deref(), Some("other"));
    assert_eq!(disk::work_text(&repo, "a.txt").as_deref(), Some("a from other\n"));
    assert_eq!(disk::work_text(&repo, "b.txt").as_deref(), Some("b edited\n"));

    // step 9: an edit the checkout would overwrite stops it; nothing changes.
    write(&repo, "a.txt", "a edited here\n");
    j.refresh();
    j.branch_menu("main", "Checkout main");
    j.toast(false, "Checkout failed");
    j.see("other", Region::Toolbar);
    assert_eq!(disk::head_branch(&repo).as_deref(), Some("other"));
    assert_eq!(disk::work_text(&repo, "a.txt").as_deref(), Some("a edited here\n"));
    assert_eq!(disk::work_text(&repo, "b.txt").as_deref(), Some("b edited\n"));
    assert!(j.logged("a.txt"), "the activity log names the file in the way");
}
