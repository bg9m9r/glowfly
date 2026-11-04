//! Headless tests for when the WIP row is drawn selected (`RepoSession::wip_lit`): opening a
//! dirty repository selects the WIP (its staging panel shows) without lighting the row, and a
//! refresh that falls back to the WIP leaves it unlit; choosing it (a click or the keyboard
//! send `SelectWip`, Show in progress, Ctrl-click, Go to commit message) lights it.

use super::selection_tests::refresh;
use super::{clean_tree, drive, git, scratch_repo, state};
use crate::keymap::KeyAction;
use crate::msg::{Msg, RepoMsg, SelectionMsg};
use crate::state::Selection;

#[test]
fn opening_a_clean_repository_selects_nothing() {
    let dir = scratch_repo("wip-lit-clean");
    clean_tree(&dir);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    assert_eq!(st.session.selection, Selection::None);
    assert!(!st.session.wip_drawn_selected());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn opening_a_dirty_repository_keeps_the_staging_panel_but_leaves_the_row_unlit() {
    let dir = scratch_repo("wip-lit-open");
    let mut st = state();
    // Left lit by the tab's last repository.
    st.session.wip_lit = true;
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    assert_eq!(st.session.selection, Selection::Wip, "the staging panel and commit box show");
    assert!(!st.session.wip_lit && !st.session.wip_drawn_selected(), "but the row isn't drawn selected");

    // Show in progress, from the unlit WIP, lights it.
    drive(&mut st, Msg::Repo(RepoMsg::ShowInProgress));
    assert!(st.session.wip_drawn_selected());

    // Choosing it (a click, Home, the arrows landing on it) lights it.
    st.session.wip_lit = false;
    drive(&mut st, Msg::Repo(RepoMsg::SelectWip));
    assert_eq!(st.session.selection, Selection::Wip);
    assert!(st.session.wip_drawn_selected());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_refresh_that_falls_back_to_the_wip_leaves_it_unlit() {
    let dir = scratch_repo("wip-lit-refresh");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Repo(RepoMsg::SelectWip));
    let head = st.snap.as_ref().unwrap().head_oid.unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::SelectCommit(head)));
    drive(&mut st, Msg::Selection(SelectionMsg::ToggleWip));
    assert!(matches!(st.session.selection, Selection::VsWorkdir(_)));
    assert!(st.session.wip_drawn_selected(), "a comparison draws the WIP row selected");
    // The compared commit goes away (its message is amended): the WIP stays, unlit.
    git(&dir, &["commit", "-q", "--amend", "-m", "first, amended"]);
    refresh(&mut st);
    assert_eq!(st.session.selection, Selection::Wip);
    assert!(!st.session.wip_drawn_selected());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ctrl_click_on_the_unlit_wip_lights_it() {
    let dir = scratch_repo("wip-lit-ctrl");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    assert!(st.session.selection == Selection::Wip && !st.session.wip_drawn_selected());
    drive(&mut st, Msg::Selection(SelectionMsg::ToggleWip));
    assert_eq!(st.session.selection, Selection::Wip);
    assert!(st.session.wip_drawn_selected(), "Ctrl-click chooses it as a plain click does");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn go_to_commit_message_on_the_unlit_wip_lights_it() {
    let dir = scratch_repo("wip-lit-focus");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    assert!(st.session.selection == Selection::Wip && !st.session.wip_drawn_selected());
    drive(&mut st, Msg::Shortcut(KeyAction::FocusMessage));
    assert_eq!(st.session.selection, Selection::Wip);
    assert!(st.view.focus_message);
    assert!(st.session.wip_drawn_selected(), "going to the commit message chooses the WIP");
    let _ = std::fs::remove_dir_all(&dir);
}
