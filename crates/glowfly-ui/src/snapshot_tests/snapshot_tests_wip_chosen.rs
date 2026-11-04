//! The WIP row chosen or not (issue #439): a repository opens with the WIP selected (its staging
//! panel shows) but the row unlit, so the first Down lands on it; a click chooses it, and the
//! chosen row has the selection fill and a `text` label, dark and light.

use super::snapshot_tests::{harness, remove_fixture, render, with_repo};
use super::snapshot_tests_select::{click, row_y};
use crate::msg::{Msg, RepoMsg};
use crate::state::{AppState, Selection};
use eframe::egui::{Key, PointerButton, pos2};
use glowfly_core::settings::ThemeChoice;

/// The selections the frames after a key or click asked for.
fn selects(msgs: &[Msg]) -> Vec<String> {
    msgs.iter()
        .filter_map(|m| match m {
            Msg::Repo(RepoMsg::SelectWip) => Some("wip".to_owned()),
            Msg::Repo(RepoMsg::SelectCommit(o)) => Some(o.to_string()),
            _ => None,
        })
        .collect()
}

fn down(st: AppState) -> Vec<String> {
    let mut h = harness(st, true);
    h.run_steps(4);
    h.state_mut().1.clear();
    h.press_key(Key::ArrowDown);
    h.run_steps(4);
    selects(&h.state().1)
}

#[test]
fn graph_wip_chosen() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "wip-chosen");
    st.session.wip_lit = true;
    render("graph_wip_chosen_dark", st, true);
    let (mut st, _) = with_repo(ThemeChoice::Light, "wip-chosen");
    st.session.wip_lit = true;
    render("graph_wip_chosen_light", st, false);
    remove_fixture(&dir);
}

#[test]
fn down_from_an_unlit_wip_lands_on_it_and_from_a_chosen_one_on_head() {
    let (st, dir) = with_repo(ThemeChoice::Dark, "wip-chosen-down");
    assert_eq!(st.session.selection, Selection::Wip);
    assert!(!st.session.wip_drawn_selected());
    let head = st.snap.as_ref().unwrap().head_oid.unwrap();
    assert_eq!(down(st), ["wip"], "the first Down chooses the WIP row");
    let (mut st, _) = with_repo(ThemeChoice::Dark, "wip-chosen-down");
    st.session.wip_lit = true;
    assert_eq!(down(st), [head.to_string()], "from the chosen WIP row, Down goes to HEAD");
    remove_fixture(&dir);
}

#[test]
fn a_click_on_the_unlit_wip_row_chooses_it() {
    let (st, dir) = with_repo(ThemeChoice::Dark, "wip-chosen-click");
    let mut h = harness(st, true);
    h.run();
    h.state_mut().1.clear();
    click(&mut h, pos2(700.0, row_y(0)), PointerButton::Primary);
    assert_eq!(selects(&h.state().1), ["wip"]);
    drop(h);
    remove_fixture(&dir);
}
