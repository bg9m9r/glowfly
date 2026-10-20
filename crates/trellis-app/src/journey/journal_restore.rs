//! A hard reset throws commits away; the operation journal shows the state before it, and
//! Restore puts the branch back. Undo then takes the restore back again.

use super::find::{Pick, Region};
use super::fixture::{self as disk, Fixture, commit_files};
use super::harness::Journey;

#[test]
fn restore_a_branch_from_the_journal_after_a_hard_reset() {
    let fx = Fixture::new("journal-restore");
    let repo = fx.work().join("app");
    fx.init(&repo, &[("a.txt", "a\n")]);
    let base = disk::head_oid(&repo);
    commit_files(&repo, &[("b.txt", "b\n")], "Add b");
    commit_files(&repo, &[("c.txt", "c\n")], "Add c");
    let tip = disk::head_oid(&repo);
    let base_summary = disk::summary(&repo, base);
    let mut j = Journey::start(fx, Some(repo.clone()), vec![repo.clone()]);

    // step 1: hard reset main back to the first commit from its menu in the graph.
    j.right_click(&base_summary, Region::Central, Pick::Only);
    j.hover("Reset main to this commit", Region::Anywhere, Pick::Only);
    j.menu("Hard: reset files too…");
    j.see("Hard reset", Region::Anywhere);
    // The dialog's first frame is egui's sizing pass, drawn disabled.
    j.settle();
    j.click("Reset", Region::Anywhere, Pick::Last);
    j.toast(true, "Reset (hard) succeeded");
    assert_eq!(disk::branch_oid(&repo, "main"), Some(base));
    assert!(disk::work_text(&repo, "c.txt").is_none());

    // step 2: open the journal from the palette; the state before the reset is listed.
    j.run_command("journal", "Open operation journal");
    let before = format!("Before resetting main to {}", &base.to_string()[..7]);
    j.see(&before, Region::Central);

    // step 3: select it and restore it.
    j.click(&before, Region::Central, Pick::Only);
    j.see("History at this state", Region::Central);
    j.click("Restore…", Region::Central, Pick::Only);
    j.see("Restore main?", Region::Anywhere);
    j.settle();
    j.click("Restore", Region::Anywhere, Pick::Last);
    j.toast(true, "Restore");
    assert_eq!(disk::branch_oid(&repo, "main"), Some(tip), "main is back at its old tip");
    assert_eq!(disk::work_text(&repo, "c.txt").as_deref(), Some("c\n"));
    assert!(disk::is_clean(&repo));

    // step 4: the restore is undoable.
    j.enabled("Undo the last action", Region::Toolbar);
    j.click("Undo the last action", Region::Toolbar, Pick::Only);
    j.toast(true, "Undo");
    assert_eq!(disk::branch_oid(&repo, "main"), Some(base));
}
