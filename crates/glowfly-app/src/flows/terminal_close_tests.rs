//! Headless tests for closing a terminal tab whose shell runs: asking the runner what the
//! shell is doing, closing at once when it is idle or unknown, and the confirmation while it
//! runs a program. No shell runs here.

use super::terminal_tests::{open_with_panel, run, terminal, workdir};
use super::update;
use crate::effect::Effect;
use crate::msg::Msg;
use crate::state::terminal::SessionStatus;
use crate::state::{AppState, Modal};
use crate::terminal::{AfterCheck, TerminalEffect as E, TerminalMsg as T};
use glowfly_core::terminal::ShellActivity;
use std::time::Instant;

/// The first tab's name once it runs pwsh: "pwsh \u{b7} <the fixture's folder>".
fn first_tab(st: &AppState) -> String {
    let dir = super::terminal_tests::workdir(st);
    format!("pwsh \u{b7} {}", dir.file_name().unwrap().to_string_lossy())
}

fn started(st: &mut AppState, id: u64) {
    run(st, T::Started { id, program: "pwsh".into() });
    assert_eq!(st.terminal.sessions.iter().find(|s| s.id == id).unwrap().status, SessionStatus::Running);
}

fn has(st: &AppState, id: u64) -> bool {
    st.terminal.index_of(id).is_some()
}

fn active(st: &AppState) -> Option<u64> {
    st.terminal.active_id(&workdir(st))
}

/// A second tab, started; returns its id.
fn second(st: &mut AppState) -> u64 {
    let fx = terminal(run(st, T::NewTab(None)));
    let [E::Spawn { id, .. }] = fx.as_slice() else { panic!("{fx:?}") };
    let id = *id;
    started(st, id);
    id
}

fn activity(st: &mut AppState, id: u64, activity: ShellActivity) -> Vec<Effect> {
    run(st, T::Activity { id, activity, then: AfterCheck::Close })
}

fn running(program: Option<&str>) -> ShellActivity {
    ShellActivity::Running { program: program.map(String::from) }
}

#[test]
fn a_running_shell_is_asked_about_first_and_closes_when_idle() {
    let (mut st, dir, one) = open_with_panel("term-close-idle");
    started(&mut st, one);
    let two = second(&mut st);
    let fx = terminal(run(&mut st, T::CloseTab(two)));
    assert!(matches!(&fx[..], [E::CheckActivity { id: c, then: AfterCheck::Close }] if *c == two), "{fx:?}");
    assert!(has(&st, two), "the tab stays until the answer");

    let fx = terminal(activity(&mut st, two, ShellActivity::Idle));
    assert!(matches!(&fx[..], [E::Kill(k), E::Close(c)] if *k == two && *c == two), "{fx:?}");
    assert!(!has(&st, two));
    assert_eq!(active(&st), Some(one), "the neighbour takes over");
    assert!(st.modal.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_unknown_answer_closes_without_asking() {
    let (mut st, dir, one) = open_with_panel("term-close-unknown");
    started(&mut st, one);
    run(&mut st, T::CloseTab(one));
    let fx = terminal(activity(&mut st, one, ShellActivity::Unknown));
    assert!(matches!(&fx[..], [E::Kill(_), E::Close(_)]), "{fx:?}");
    assert!(st.modal.is_none());
    assert!(st.terminal.sessions.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_running_program_asks_and_close_confirms() {
    let (mut st, dir, one) = open_with_panel("term-close-busy");
    started(&mut st, one);
    let two = second(&mut st);
    run(&mut st, T::CloseTab(one));
    let fx = terminal(activity(&mut st, one, running(Some("git"))));
    assert!(fx.is_empty(), "{fx:?}");
    assert!(has(&st, one));
    match &st.modal {
        Some(Modal::ConfirmDanger { title, body, button, then, .. }) => {
            assert_eq!(title, "Close terminal");
            assert_eq!(*body, format!("{} is running git. Close it and stop git?", first_tab(&st)));
            assert_eq!(button, "Close terminal", "the action, not a dismiss label");
            assert!(matches!(**then, Msg::Terminal(T::CloseTabNow(c)) if c == one));
        }
        _ => panic!("expected a confirmation"),
    }
    let fx = terminal(update(&mut st, Msg::ConfirmAccepted, Instant::now()));
    assert!(matches!(&fx[..], [E::Kill(k), E::Close(c)] if *k == one && *c == one), "{fx:?}");
    assert!(!has(&st, one) && has(&st, two));
    assert!(st.modal.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_unnamed_program_gets_the_generic_question() {
    let (mut st, dir, one) = open_with_panel("term-close-unnamed");
    started(&mut st, one);
    activity(&mut st, one, running(None));
    match &st.modal {
        Some(Modal::ConfirmDanger { body, .. }) => {
            assert_eq!(*body, format!("{} is running a program. Close it and stop it?", first_tab(&st)))
        }
        _ => panic!("expected a confirmation"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn late_answers_for_gone_or_ended_sessions() {
    let (mut st, dir, one) = open_with_panel("term-close-late");
    started(&mut st, one);
    let two = second(&mut st);
    assert!(activity(&mut st, 999, running(Some("git"))).is_empty(), "an unknown id is ignored");
    assert!(st.modal.is_none());

    // The shell ended while the runner looked: it closes without a dialog.
    run(&mut st, T::CloseTab(two));
    run(&mut st, T::Exited { id: two, code: Some(0) });
    let fx = terminal(activity(&mut st, two, running(Some("git"))));
    assert!(matches!(&fx[..], [E::Close(c)] if *c == two), "{fx:?}");
    assert!(!has(&st, two));
    assert!(st.modal.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn starting_and_ended_shells_close_at_once() {
    let (mut st, dir, one) = open_with_panel("term-close-now");
    // Still starting.
    let fx = terminal(run(&mut st, T::CloseTab(one)));
    assert!(fx.iter().all(|e| !matches!(e, E::CheckActivity { .. })), "{fx:?}");
    assert!(!has(&st, one));
    let _ = std::fs::remove_dir_all(&dir);

    let (mut st, dir, one) = open_with_panel("term-close-ended");
    run(&mut st, T::Exited { id: one, code: Some(1) });
    let fx = terminal(run(&mut st, T::CloseTab(one)));
    assert!(matches!(&fx[..], [E::Close(c)] if *c == one), "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn another_dialog_leaves_everything_as_it_was() {
    let (mut st, dir, one) = open_with_panel("term-close-modal");
    started(&mut st, one);
    let other = Modal::Confirm { title: "Other".into(), body: String::new(), button: "OK".into(), then: Box::new(Msg::Terminal(T::Hide)) };
    st.modal = Some(other);
    let fx = terminal(activity(&mut st, one, running(Some("git"))));
    assert!(fx.is_empty(), "{fx:?}");
    assert!(has(&st, one));
    assert!(matches!(&st.modal, Some(Modal::Confirm { title, .. }) if title == "Other"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn closing_the_last_tab_after_the_answer_hides_the_panel() {
    let (mut st, dir, one) = open_with_panel("term-close-last");
    started(&mut st, one);
    run(&mut st, T::CloseTab(one));
    let fx = activity(&mut st, one, ShellActivity::Idle);
    let saved = fx.iter().find_map(|e| match e {
        Effect::SaveSettings(s) => Some(s.terminal.panel_open),
        _ => None,
    });
    assert_eq!(saved, Some(false));
    assert!(!st.terminal.open);
    let _ = std::fs::remove_dir_all(&dir);
}
