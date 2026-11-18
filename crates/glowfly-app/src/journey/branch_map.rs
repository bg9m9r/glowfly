//! Two branches that change the same line of a file: the Branch map, opened from the command
//! palette, lists both, shows what they share when one is selected, and Esc goes back to the
//! graph. Nothing in the repository changes.

use super::find::{Pick, Region};
use super::fixture::{self as disk, Fixture, commit_files};
use super::harness::Journey;
use eframe::egui::Key;

fn file(line: &str) -> String {
    (0..20).map(|i| if i == 5 { format!("{line}\n") } else { format!("line {i}\n") }).collect()
}

#[test]
fn open_the_branch_map_and_see_what_two_branches_share() {
    let fx = Fixture::new("branch-map");
    let repo = fx.work().join("app");
    fx.init(&repo, &[("f.txt", &file("base"))]);
    for name in ["one", "two"] {
        disk::git(&repo, &["checkout", "-q", "-b", name, "main"]);
        commit_files(&repo, &[("f.txt", &file(name))], name);
    }
    disk::git(&repo, &["checkout", "-q", "main"]);
    let tips = (disk::branch_oid(&repo, "main"), disk::branch_oid(&repo, "one"), disk::branch_oid(&repo, "two"));
    let mut j = Journey::start(fx, Some(repo.clone()), vec![repo.clone()]);

    // step 1: the palette opens the map, with a row for each branch and its numbers.
    j.run_command("branch map", "Open branch map");
    j.see("Branch map", Region::Central);
    j.see_contains("main, checked out, the main branch: last commit", Region::Central);
    j.see_contains("one: 0 behind, 1 ahead, 1 file changed", Region::Central);
    assert!(!j.state().graph_shown());

    // step 2: selecting a branch lists what it shares with the other one, and that merging
    // them would conflict.
    j.click_containing("two: ", Region::Central, Pick::Only);
    j.see("Conflicts in 1 file", Region::Central);
    j.see("f.txt", Region::Central);

    // step 3: Esc goes back to the graph; the repository is as it was.
    j.press(Key::Escape);
    assert!(j.state().session.branch_map.view.is_none() && j.state().graph_shown());
    assert_eq!((disk::branch_oid(&repo, "main"), disk::branch_oid(&repo, "one"), disk::branch_oid(&repo, "two")), tips);
    assert!(disk::is_clean(&repo));
}
