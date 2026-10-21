//! A branch whose tip was amended: its menu compares it with its version before, commit by
//! commit, and Esc goes back to the graph. Nothing in the repository changes.

use super::find::{Pick, Region};
use super::fixture::{self as disk, Fixture, commit_files};
use super::harness::Journey;
use crate::state::range_diff::Phase;
use eframe::egui::Key;
use trellis_core::range_diff::{Outcome, Status};

#[test]
fn compare_an_amended_branch_with_its_version_before() {
    let fx = Fixture::new("range-diff");
    let repo = fx.work().join("app");
    fx.init(&repo, &[("a.txt", "a\n")]);
    disk::git(&repo, &["checkout", "-q", "-b", "topic"]);
    commit_files(&repo, &[("f.txt", "1\n2\n3\n4\n5\n6\n7\n8\n")], "Add the list");
    disk::write(&repo, "f.txt", "1\n2\n3\nfour, edited\n5\n6\n7\n8\n");
    disk::git(&repo, &["commit", "-q", "-a", "--amend", "--no-edit"]);
    disk::git(&repo, &["checkout", "-q", "main"]);
    let tip = disk::branch_oid(&repo, "topic");
    let mut j = Journey::start(fx, Some(repo.clone()), vec![repo.clone()]);

    // step 1: the branch's menu in the sidebar compares it with its version before.
    j.right_click("topic", Region::Sidebar, Pick::Only);
    j.menu("Compare with before last rebase");
    j.see("Range-diff", Region::Central);
    j.see("Changed: Add the list", Region::Central);
    let view = j.state().range_diff.as_ref().expect("the view is open");
    let Phase::Ready(outcome) = &view.phase else { panic!("not ready: {:?}", view.phase) };
    let Outcome::Compared(c) = outcome.as_ref() else { panic!("{outcome:?}") };
    let pair = &c.pairs[view.selected.expect("the changed pair is selected")];
    assert_eq!(pair.status, Status::Changed);
    let lines: Vec<&str> = pair.interdiff.iter().flat_map(|d| &d.hunks).flat_map(|h| &h.lines).map(|l| l.text.as_str()).collect();
    assert!(lines.contains(&"+four, edited"), "the edited line is in the interdiff: {lines:?}");

    // step 2: Esc goes back to the graph; the repository is as it was.
    j.press(Key::Escape);
    assert!(j.state().range_diff.is_none() && j.state().graph_shown());
    assert_eq!(disk::branch_oid(&repo, "topic"), tip);
    assert!(disk::is_clean(&repo));
}
