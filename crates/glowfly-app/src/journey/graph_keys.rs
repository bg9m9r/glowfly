//! The graph keeps its keys: after a click on a button elsewhere (Stage all), Down still moves
//! the graph's selection; and after the file list's pane key, the graph's pane key gives the
//! graph its keys back.

use super::find::{Pick, Region};
use super::fixture::{Fixture, commit_files};
use super::harness::Journey;
use crate::state::Selection;
use eframe::egui::{Key, Modifiers};

#[test]
fn down_moves_the_selection_after_a_click_on_stage_all() {
    let fx = Fixture::new("graph-keys");
    let repo = fx.work().join("app");
    fx.init(&repo, &[("a.txt", "a\n")]);
    commit_files(&repo, &[("b.txt", "b\n")], "Add b");
    std::fs::write(repo.join("c.txt"), "c\n").unwrap();
    let mut j = Journey::start(fx, Some(repo.clone()), vec![repo.clone()]);

    j.refresh();
    j.mouse_click("Uncommitted changes", Region::Central, Pick::Only);
    j.mouse_click("Stage all", Region::Details, Pick::Only);
    assert_eq!(j.state().session.selection, Selection::Wip);
    j.press(Key::ArrowDown);
    let head = j.state().snap.as_ref().and_then(|s| s.head_oid).expect("a HEAD");
    assert_eq!(j.state().session.selection, Selection::Commit(head), "Down moved from the WIP row to HEAD");

    // The details panel's file list takes the focus; the graph's pane key gives the graph its
    // keys back.
    let pane = if cfg!(target_os = "macos") { Modifiers::CTRL } else { Modifiers::ALT };
    j.press_with(pane, Key::Num3);
    j.press_with(pane, Key::Num2);
    j.press(Key::ArrowUp);
    assert_eq!(j.state().session.selection, Selection::Wip, "Up went back to the WIP row");
}
