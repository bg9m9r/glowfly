//! Preview a drop before doing it: the dialog shows what the branch would look like, changes
//! nothing on disk, and its Drop… button goes on to the usual question and the drop itself.

use super::find::{Pick, Region};
use super::fixture::{self as disk, Fixture, commit_files};
use super::harness::Journey;

#[test]
fn preview_a_drop_then_apply_it() {
    let fx = Fixture::new("op-preview");
    let repo = fx.work().join("app");
    fx.init(&repo, &[("a.txt", "a\n")]);
    commit_files(&repo, &[("b.txt", "b\n")], "Add b");
    commit_files(&repo, &[("c.txt", "c\n")], "Add c");
    let tip = disk::head_oid(&repo);
    let mut j = Journey::start(fx, Some(repo.clone()), vec![repo.clone()]);

    // step 1: preview dropping "Add b" from its menu in the graph.
    j.right_click("Add b", Region::Central, Pick::Only);
    j.menu("Preview drop");
    j.see("Preview: drop 1 commit from main", Region::Anywhere);
    j.see("Dropping removes 1 commit.", Region::Anywhere);
    j.see("1 later commit is rewritten.", Region::Anywhere);
    j.see("1 commit no branch or tag will reach", Region::Anywhere);
    assert_eq!(disk::branch_oid(&repo, "main"), Some(tip), "previewing changes nothing");

    // step 2: apply it; the drop asks as it does from the menu, then runs.
    j.settle();
    j.click("Drop…", Region::Anywhere, Pick::Last);
    j.see("Drop commit", Region::Anywhere);
    j.settle();
    j.click("Drop commit", Region::Anywhere, Pick::Last);
    j.toast(true, "Drop commit");
    assert!(disk::work_text(&repo, "b.txt").is_none(), "b's change is gone");
    assert_eq!(disk::work_text(&repo, "c.txt").as_deref(), Some("c\n"), "c's change stays");
}
