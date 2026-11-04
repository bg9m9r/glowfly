//! The graph keeps its keys (issue #441): after Tab gave a button (Stage all) or a WIP file list
//! the keyboard focus, Esc gives the graph its keys back and Down moves its selection; that Esc
//! does nothing else (two selected commits stay selected); Tab reaches the graph itself, whose
//! arrows then move it; and after the file list's pane key, the graph's pane key gives the graph
//! its keys back. A WIP row that opened selected on its own (unlit) is where the first Down,
//! Up or Home lands (issue #439), and a click lights it.

use super::fixture::{Fixture, commit_files};
use super::harness::Journey;
use crate::state::Selection;
use eframe::egui::{Key, Modifiers};
use git2::Oid;

/// A repository with two commits and an unstaged file, as it opens: the WIP selected (its
/// staging panel shows) but not chosen, so its row isn't lit.
fn opened(name: &str) -> (Journey, Oid) {
    let fx = Fixture::new(name);
    let repo = fx.work().join("app");
    fx.init(&repo, &[("a.txt", "a\n")]);
    commit_files(&repo, &[("b.txt", "b\n")], "Add b");
    std::fs::write(repo.join("c.txt"), "c\n").unwrap();
    let mut j = Journey::start(fx, Some(repo.clone()), vec![repo.clone()]);
    j.refresh();
    assert_eq!(j.state().session.selection, Selection::Wip);
    assert!(!j.state().session.wip_drawn_selected(), "opening doesn't light the WIP row");
    let head = j.state().snap.as_ref().and_then(|s| s.head_oid).expect("a HEAD");
    (j, head)
}

/// `opened`, then the WIP row clicked: chosen, and lit.
fn start(name: &str) -> (Journey, Oid) {
    let (mut j, head) = opened(name);
    j.select_wip();
    assert_eq!(j.state().session.selection, Selection::Wip);
    assert!(j.state().session.wip_drawn_selected(), "a click lights the WIP row");
    (j, head)
}

#[test]
fn the_first_down_up_or_home_lands_on_the_unlit_wip_row() {
    for (key, name) in [(Key::ArrowDown, "down"), (Key::ArrowUp, "up"), (Key::Home, "home")] {
        let (mut j, head) = opened(&format!("graph-keys-unlit-{name}"));
        j.press(key);
        assert_eq!(j.state().session.selection, Selection::Wip, "{name}");
        assert!(j.state().session.wip_drawn_selected(), "{name} lit the WIP row");
        j.press(Key::ArrowDown);
        assert_eq!(j.state().session.selection, Selection::Commit(head), "{name}, then Down: HEAD");
    }
}

#[test]
fn down_moves_the_graph_after_tab_to_stage_all_and_esc() {
    let (mut j, head) = start("graph-keys-stage-all");
    j.tab_to("Stage all");
    j.press(Key::Escape);
    assert_eq!(j.state().session.selection, Selection::Wip, "Esc only gave the graph its keys");
    j.press(Key::ArrowDown);
    assert_eq!(j.state().session.selection, Selection::Commit(head), "Down moved from the WIP row to HEAD");
}

#[test]
fn down_moves_the_graph_after_right_into_a_wip_list_and_esc() {
    let (mut j, head) = start("graph-keys-wip-list");
    j.tab_to("Unstaged files (1)");
    j.press(Key::ArrowRight);
    assert_eq!(j.focused_name().as_deref(), Some("Staged files (0)"), "Right went to Staged files");
    j.press(Key::Escape);
    j.press(Key::ArrowDown);
    assert_eq!(j.state().session.selection, Selection::Commit(head), "Down moved from the WIP row to HEAD");
}

#[test]
fn tab_reaches_the_graph_and_its_arrows_move_it() {
    let (mut j, head) = start("graph-keys-tab");
    j.tab_to("Commit graph");
    j.press(Key::ArrowDown);
    assert_eq!(j.state().session.selection, Selection::Commit(head));
    assert_eq!(j.focused_name().as_deref(), Some("Commit graph"), "the arrow kept the focus on the graph");
    j.press(Key::ArrowUp);
    assert_eq!(j.state().session.selection, Selection::Wip);
}

#[test]
fn the_graphs_pane_key_gives_it_its_keys_back() {
    let (mut j, head) = start("graph-keys-pane");
    j.press(Key::ArrowDown);
    assert_eq!(j.state().session.selection, Selection::Commit(head));
    // The details panel's file list takes the focus; the graph's pane key gives it back.
    let pane = if cfg!(target_os = "macos") { Modifiers::CTRL } else { Modifiers::ALT };
    j.press_with(pane, Key::Num3);
    j.press_with(pane, Key::Num2);
    assert_eq!(j.focused_name().as_deref(), Some("Commit graph"));
    j.press(Key::ArrowUp);
    assert_eq!(j.state().session.selection, Selection::Wip, "Up went back to the WIP row");
}

#[test]
fn esc_on_a_focused_button_only_gives_the_graph_its_keys() {
    let (mut j, _head) = start("graph-keys-esc-only");
    j.press(Key::ArrowDown);
    j.press_with(Modifiers::SHIFT, Key::ArrowDown);
    assert!(matches!(&j.state().session.selection, Selection::Commits(m) if m.oids.len() == 2), "Shift+Down selected two commits");
    j.press(Key::Tab);
    let on = j.focused_name();
    assert!(on.is_some() && on.as_deref() != Some("Commit graph"), "Tab focused a control: {on:?}");
    j.press(Key::Escape);
    assert!(
        matches!(&j.state().session.selection, Selection::Commits(m) if m.oids.len() == 2),
        "the Esc left the two commits selected: {:?}",
        j.state().session.selection
    );
    assert_eq!(j.focused_name().as_deref(), Some("Commit graph"));
    j.press(Key::ArrowUp);
    assert_eq!(j.state().session.selection, Selection::Wip, "the graph's Up, from HEAD");
}
