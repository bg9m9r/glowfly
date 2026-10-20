//! Headless tests for Kill in the terminal panel: asking the runner what the shell is doing,
//! killing at once when it is idle, unknown or still starting, and the confirmation while it
//! runs a program. No shell runs here.

use super::terminal_tests::{open_with_panel, run, terminal};
use super::update;
use crate::msg::Msg;
use crate::state::terminal::SessionStatus;
use crate::state::{AppState, Modal};
use crate::terminal::{AfterCheck, TerminalEffect as E, TerminalMsg as T};
use std::time::Instant;
use trellis_core::terminal::ShellActivity;

/// The first tab's name once it runs pwsh: "pwsh \u{b7} <the fixture's folder>".
fn first_tab(st: &AppState) -> String {
    let dir = super::terminal_tests::workdir(st);
    format!("pwsh \u{b7} {}", dir.file_name().unwrap().to_string_lossy())
}

fn started(st: &mut AppState, id: u64) {
    run(st, T::Started { id, program: "pwsh".into() });
}

fn answer(st: &mut AppState, id: u64, activity: ShellActivity) -> Vec<E> {
    terminal(run(st, T::Activity { id, activity, then: AfterCheck::Kill }))
}

fn running(program: Option<&str>) -> ShellActivity {
    ShellActivity::Running { program: program.map(String::from) }
}

#[test]
fn kill_asks_the_runner_and_kills_an_idle_or_unknown_shell() {
    for (name, activity) in [("term-kill-idle", ShellActivity::Idle), ("term-kill-unknown", ShellActivity::Unknown)] {
        let (mut st, dir, id) = open_with_panel(name);
        started(&mut st, id);
        let fx = terminal(run(&mut st, T::Kill));
        assert!(matches!(&fx[..], [E::CheckActivity { id: c, then: AfterCheck::Kill }] if *c == id), "{fx:?}");
        let fx = answer(&mut st, id, activity);
        assert!(matches!(&fx[..], [E::Kill(k)] if *k == id), "{fx:?}");
        assert!(st.modal.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn a_running_program_asks_and_kill_confirms() {
    let (mut st, dir, id) = open_with_panel("term-kill-busy");
    started(&mut st, id);
    run(&mut st, T::Kill);
    assert!(answer(&mut st, id, running(Some("npm"))).is_empty());
    match &st.modal {
        Some(Modal::ConfirmDanger { title, body, button, then, .. }) => {
            assert_eq!(title, "Kill terminal");
            assert_eq!(*body, format!("{} is running npm. Kill the shell and stop npm?", first_tab(&st)));
            assert_eq!(button, "Kill");
            assert!(matches!(**then, Msg::Terminal(T::KillNow(k)) if k == id));
        }
        _ => panic!("expected a confirmation"),
    }
    let fx = terminal(update(&mut st, Msg::ConfirmAccepted, Instant::now()));
    assert!(matches!(&fx[..], [E::Kill(k)] if *k == id), "{fx:?}");
    assert!(st.modal.is_none());
    assert_eq!(st.terminal.sessions.len(), 1, "the tab stays, for Restart");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_unnamed_program_gets_the_generic_question() {
    let (mut st, dir, id) = open_with_panel("term-kill-unnamed");
    started(&mut st, id);
    answer(&mut st, id, running(None));
    match &st.modal {
        Some(Modal::ConfirmDanger { body, .. }) => {
            assert_eq!(*body, format!("{} is running a program. Kill the shell and stop it?", first_tab(&st)))
        }
        _ => panic!("expected a confirmation"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn another_dialog_drops_the_click() {
    let (mut st, dir, id) = open_with_panel("term-kill-modal");
    started(&mut st, id);
    let other = Modal::Confirm { title: "Other".into(), body: String::new(), button: "OK".into(), then: Box::new(Msg::Terminal(T::Hide)) };
    st.modal = Some(other);
    assert!(answer(&mut st, id, running(Some("npm"))).is_empty());
    assert!(matches!(&st.modal, Some(Modal::Confirm { title, .. }) if title == "Other"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ended_and_starting_shells() {
    let (mut st, dir, id) = open_with_panel("term-kill-ended");
    started(&mut st, id);
    run(&mut st, T::Kill);
    // The shell exited while the runner looked: nothing to kill or ask.
    run(&mut st, T::Exited { id, code: Some(0) });
    assert!(answer(&mut st, id, running(Some("npm"))).is_empty());
    assert!(answer(&mut st, id, ShellActivity::Idle).is_empty());
    assert!(st.modal.is_none());
    assert!(terminal(run(&mut st, T::KillNow(id))).is_empty(), "KillNow on an ended shell does nothing");
    let _ = std::fs::remove_dir_all(&dir);

    // Still starting: killed at once, without asking.
    let (mut st, dir, id) = open_with_panel("term-kill-starting");
    assert_eq!(st.terminal.sessions[0].status, SessionStatus::Starting);
    let fx = terminal(run(&mut st, T::Kill));
    assert!(matches!(&fx[..], [E::Kill(k)] if *k == id), "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
