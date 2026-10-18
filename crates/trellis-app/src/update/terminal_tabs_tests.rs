//! Headless tests for terminal tabs: new tabs and their numbers, switching, closing (the
//! neighbour takes over, the last one hides the panel), Restart in place, Kill all and its
//! confirmation, and closing the repository's tab. No shell runs here.

use super::terminal_tests::{open_with_panel, run, terminal, workdir};
use super::tests::{drive, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::msg::{Msg, RepoMsg, TabsMsg};
use crate::state::terminal::SessionStatus;
use crate::state::{AppState, Modal};
use crate::terminal::{TerminalEffect as E, TerminalMsg as T};
use std::time::Instant;
use trellis_core::terminal::TermInput;

/// The id of the one Spawn in `fx`.
fn spawned(fx: Vec<Effect>) -> u64 {
    match terminal(fx).as_slice() {
        [E::Spawn { id, .. }] => *id,
        other => panic!("{other:?}"),
    }
}

/// (number, id) of the active repository's sessions in tab order.
fn tabs(st: &AppState) -> Vec<(u32, u64)> {
    st.terminal.sessions_for(&workdir(st)).map(|s| (s.number, s.id)).collect()
}

fn active(st: &AppState) -> Option<u64> {
    st.terminal.active_id(&workdir(st))
}

fn saved_open(fx: &[Effect]) -> Option<bool> {
    fx.iter().find_map(|e| match e {
        Effect::SaveSettings(s) => Some(s.terminal.panel_open),
        _ => None,
    })
}

#[test]
fn a_new_tab_is_numbered_and_takes_the_input() {
    let (mut st, dir, first) = open_with_panel("term-tab-new");
    let second = spawned(run(&mut st, T::NewTab));
    assert_eq!(tabs(&st), [(1, first), (2, second)]);
    assert_eq!(active(&st), Some(second));
    assert!(st.terminal.focus_requested);
    let fx = terminal(run(&mut st, T::Input(vec![TermInput::Text("x".into())])));
    assert!(matches!(&fx[..], [E::Write { id, .. }] if *id == second), "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn new_tab_on_a_hidden_panel_starts_one_shell() {
    let dir = scratch_repo("term-tab-hidden");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let fx = run(&mut st, T::NewTab);
    assert_eq!(saved_open(&fx), Some(true), "the open panel is remembered");
    spawned(fx);
    assert!(st.terminal.open);
    assert_eq!(st.terminal.sessions.len(), 1);
    assert_eq!(st.terminal.sessions[0].number, 1);

    // Hidden again with its shell: showing it adds the new tab.
    run(&mut st, T::Hide);
    spawned(run(&mut st, T::NewTab));
    assert_eq!(st.terminal.sessions.len(), 2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn select_tab_switches_and_numbers_reuse_the_lowest_free() {
    let (mut st, dir, one) = open_with_panel("term-tab-select");
    let two = spawned(run(&mut st, T::NewTab));
    let three = spawned(run(&mut st, T::NewTab));
    st.terminal.focus_requested = false;
    run(&mut st, T::SelectTab(one));
    assert_eq!(active(&st), Some(one));
    assert!(st.terminal.focus_requested);
    run(&mut st, T::SelectTab(999));
    assert_eq!(active(&st), Some(one), "an unknown id is ignored");

    run(&mut st, T::CloseTab(two));
    let four = spawned(run(&mut st, T::NewTab));
    assert_eq!(tabs(&st), [(1, one), (3, three), (2, four)]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn closing_a_tab_stops_it_and_activates_a_neighbour() {
    let (mut st, dir, one) = open_with_panel("term-tab-close");
    let two = spawned(run(&mut st, T::NewTab));
    let three = spawned(run(&mut st, T::NewTab));
    run(&mut st, T::Started { id: two, program: "pwsh".into() });
    run(&mut st, T::SelectTab(two));
    let fx = terminal(run(&mut st, T::CloseTab(two)));
    assert!(matches!(&fx[..], [E::Kill(k), E::Close(c)] if *k == two && *c == two), "{fx:?}");
    assert_eq!(active(&st), Some(three), "the right neighbour");
    run(&mut st, T::Exited { id: three, code: Some(0) });
    let fx = terminal(run(&mut st, T::CloseTab(three)));
    assert!(matches!(&fx[..], [E::Close(c)] if *c == three), "an ended shell isn't killed: {fx:?}");
    assert_eq!(active(&st), Some(one), "else the left one");

    // The last one hides the panel, and showing it again starts a fresh shell.
    let fx = run(&mut st, T::CloseTab(one));
    assert_eq!(saved_open(&fx), Some(false));
    assert!(!st.terminal.open);
    assert!(st.terminal.sessions.is_empty() && st.terminal.active.is_empty());
    let fresh = spawned(run(&mut st, T::Toggle));
    assert_eq!(tabs(&st), [(1, fresh)]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn restart_keeps_the_tabs_number_and_place() {
    let (mut st, dir, one) = open_with_panel("term-tab-restart");
    let two = spawned(run(&mut st, T::NewTab));
    run(&mut st, T::SelectTab(one));
    run(&mut st, T::Exited { id: one, code: Some(1) });
    let fx = terminal(run(&mut st, T::Restart));
    let new = match &fx[..] {
        [E::Close(old), E::Spawn { id, .. }] if *old == one => *id,
        other => panic!("{other:?}"),
    };
    assert_ne!(new, one);
    assert_eq!(tabs(&st), [(1, new), (2, two)]);
    assert_eq!(active(&st), Some(new));
    assert_eq!(st.terminal.sessions[0].status, SessionStatus::Starting);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn kill_all_asks_first_and_stops_only_this_repository() {
    let (a, b) = (scratch_repo("term-tab-kill-a"), scratch_repo("term-tab-kill-b"));
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(a.clone())));
    drive(&mut st, Msg::Repo(RepoMsg::Open(b.clone())));
    let other = spawned(run(&mut st, T::Toggle));
    drive(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)));
    let one = st.terminal.sessions.iter().find(|s| s.id != other).expect("tab 1's shell").id;

    run(&mut st, T::AskKillAll);
    assert!(st.modal.is_none(), "one shell: nothing to ask");
    let two = spawned(run(&mut st, T::NewTab));
    run(&mut st, T::AskKillAll);
    match st.modal.take() {
        Some(Modal::Confirm { title, body, button, then }) => {
            assert_eq!((title.as_str(), button.as_str()), ("Kill all terminals", "Kill all"));
            assert!(body.contains("2 terminals"), "{body}");
            assert!(matches!(*then, Msg::Terminal(T::KillAll)));
        }
        _ => panic!("no confirmation"),
    }
    let fx = terminal(run(&mut st, T::KillAll));
    let killed: Vec<u64> = fx
        .iter()
        .map(|e| match e {
            E::Kill(id) => *id,
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(killed, [one, two], "the other repository's shell is untouched");
    assert_eq!(st.terminal.sessions.len(), 3, "the tabs stay");
    let _ = std::fs::remove_dir_all(&a);
    let _ = std::fs::remove_dir_all(&b);
}

#[test]
fn closing_the_repository_stops_all_its_terminals() {
    let (mut st, dir, one) = open_with_panel("term-tab-repo");
    let two = spawned(run(&mut st, T::NewTab));
    let path = workdir(&st);
    let fx = terminal(update(&mut st, Msg::Tabs(TabsMsg::CloseTab(0)), Instant::now()));
    let want = [E::Kill(one), E::Close(one), E::Kill(two), E::Close(two)];
    assert_eq!(format!("{fx:?}"), format!("{want:?}"));
    assert!(st.terminal.sessions.is_empty());
    assert!(!st.terminal.active.contains_key(&path));
    let _ = std::fs::remove_dir_all(&dir);
}
