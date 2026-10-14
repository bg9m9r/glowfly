//! Headless tests for the built-in terminal panel: one shell per repository, started when the
//! panel shows it; input, resizing, output, exit, restart and closing tabs. No shell runs here:
//! terminal effects are only inspected.

use super::tests::{drive, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::terminal::SessionStatus;
use crate::terminal::{TerminalEffect as E, TerminalMsg as T};
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::terminal::{KeyMods, TermColor, TermInput, TermKey};

fn run(st: &mut AppState, m: T) -> Vec<Effect> {
    update(st, Msg::Terminal(m), Instant::now())
}

/// Only the terminal effects of `fx`.
fn terminal(fx: Vec<Effect>) -> Vec<E> {
    fx.into_iter()
        .filter_map(|e| match e {
            Effect::Terminal(t) => Some(t),
            _ => None,
        })
        .collect()
}

fn workdir(st: &AppState) -> PathBuf {
    st.snap.as_ref().unwrap().workdir.clone()
}

/// A repository open with the panel shown; returns the session id.
fn open_with_panel(name: &str) -> (AppState, PathBuf, u64) {
    let dir = scratch_repo(name);
    let mut st = state();
    drive(&mut st, Msg::Open(dir.clone()));
    run(&mut st, T::Toggle);
    let id = st.terminal.sessions[0].id;
    (st, dir, id)
}

fn key(key: TermKey) -> TermInput {
    TermInput::Key { key, mods: KeyMods::default() }
}

#[test]
fn toggle_opens_the_panel_and_spawns_one_shell_per_repo() {
    let dir = scratch_repo("term-toggle");
    let mut st = state();
    drive(&mut st, Msg::Open(dir.clone()));
    let fx = run(&mut st, T::Toggle);
    assert!(st.terminal.open && st.terminal.focus_requested);
    assert_eq!(st.terminal.sessions.len(), 1);
    let s = &st.terminal.sessions[0];
    assert_eq!((s.workdir.clone(), s.status.clone()), (workdir(&st), SessionStatus::Starting));
    match &fx[..] {
        [Effect::Terminal(E::Spawn { cwd, rows: 24, cols: 80, .. })] => assert_eq!(*cwd, workdir(&st)),
        other => panic!("{other:?}"),
    }

    assert!(run(&mut st, T::Toggle).is_empty(), "hiding stops nothing");
    assert!(!st.terminal.open);
    assert_eq!(st.terminal.sessions.len(), 1);
    assert!(run(&mut st, T::Toggle).is_empty(), "the shell is still there");
    assert!(st.terminal.open);
    assert_eq!(st.terminal.sessions.len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn toggle_without_a_repository_does_nothing() {
    let mut st = state();
    assert!(run(&mut st, T::Toggle).is_empty());
    assert!(!st.terminal.open);
    assert!(st.terminal.sessions.is_empty());
}

#[test]
fn output_feeds_the_screen_and_marks_running() {
    let (mut st, dir, id) = open_with_panel("term-output");
    run(&mut st, T::Started { id, program: "pwsh".into() });
    assert!(run(&mut st, T::Output { id, bytes: b"hello\r\n\x1b[32mok".to_vec() }).is_empty());
    let s = &st.terminal.sessions[0];
    assert_eq!((s.status.clone(), s.program.as_str()), (SessionStatus::Running, "pwsh"));
    assert!(s.screen.text().starts_with("hello\nok"), "{:?}", s.screen.text());
    assert_eq!(s.screen.rows()[1].spans[0].style.fg, TermColor::Indexed(2));

    let before = s.screen.text();
    assert!(run(&mut st, T::Output { id: 99, bytes: b"zzz".to_vec() }).is_empty());
    assert_eq!(st.terminal.sessions[0].screen.text(), before, "unknown sessions are ignored");

    // A cursor position query is answered through the PTY.
    let fx = terminal(run(&mut st, T::Output { id, bytes: b"\x1b[6n".to_vec() }));
    assert!(matches!(&fx[..], [E::Write { bytes, .. }] if bytes == b"\x1b[2;3R"), "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn input_is_encoded_for_the_active_session() {
    let (mut st, dir, id) = open_with_panel("term-input");
    let write = |fx: Vec<Effect>| match terminal(fx).as_slice() {
        [E::Write { id: got, bytes }] if *got == id => bytes.clone(),
        other => panic!("{other:?}"),
    };
    assert_eq!(write(run(&mut st, T::Input(vec![TermInput::Text("ls".into()), key(TermKey::Enter)]))), b"ls\r");
    run(&mut st, T::Output { id, bytes: b"\x1b[?1h".to_vec() });
    assert_eq!(write(run(&mut st, T::Input(vec![key(TermKey::Up)]))), b"\x1bOA");
    let ctrl_c = TermInput::Key { key: TermKey::Char('c'), mods: KeyMods { ctrl: true, ..Default::default() } };
    assert_eq!(write(run(&mut st, T::Input(vec![ctrl_c]))), [0x03]);
    run(&mut st, T::Output { id, bytes: b"\x1b[?2004h".to_vec() });
    assert_eq!(write(run(&mut st, T::Input(vec![TermInput::Paste("a\nb".into())]))), b"\x1b[200~a\rb\x1b[201~");
    assert!(run(&mut st, T::Input(Vec::new())).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn resize_updates_screen_and_pty_once() {
    let (mut st, dir, id) = open_with_panel("term-resize");
    let fx = terminal(run(&mut st, T::Resize { rows: 30, cols: 100 }));
    assert!(matches!(&fx[..], [E::Resize { id: i, rows: 30, cols: 100 }] if *i == id), "{fx:?}");
    assert_eq!(st.terminal.sessions[0].screen.size(), (30, 100));
    assert_eq!(st.terminal.grid, Some((30, 100)));
    assert!(run(&mut st, T::Resize { rows: 30, cols: 100 }).is_empty());

    run(&mut st, T::Exited { id, code: Some(0) });
    let fx = terminal(run(&mut st, T::Restart));
    assert!(matches!(&fx[..], [E::Close(old), E::Spawn { rows: 30, cols: 100, .. }] if *old == id), "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn exit_kill_and_restart() {
    let (mut st, dir, id) = open_with_panel("term-exit");
    run(&mut st, T::Started { id, program: "pwsh".into() });
    assert!(matches!(&terminal(run(&mut st, T::Kill))[..], [E::Kill(i)] if *i == id));
    assert!(run(&mut st, T::Restart).is_empty(), "a running shell isn't restarted");

    let fx = terminal(run(&mut st, T::Exited { id, code: Some(1) }));
    assert!(matches!(&fx[..], [E::Close(i)] if *i == id));
    assert_eq!(st.terminal.sessions[0].status, SessionStatus::Exited(Some(1)));
    assert!(run(&mut st, T::Input(vec![TermInput::Text("x".into())])).is_empty(), "input after exit is dropped");
    assert!(run(&mut st, T::Kill).is_empty());
    // A late Started doesn't bring the session back to life.
    run(&mut st, T::Started { id, program: "pwsh".into() });
    assert_eq!(st.terminal.sessions[0].status, SessionStatus::Exited(Some(1)));

    st.terminal.focus_requested = false;
    let fx = terminal(run(&mut st, T::Restart));
    let new = st.terminal.sessions[0].id;
    assert_ne!(new, id);
    assert!(matches!(&fx[..], [E::Close(old), E::Spawn { id: n, .. }] if *old == id && *n == new), "{fx:?}");
    assert_eq!(st.terminal.sessions.len(), 1);
    assert_eq!(st.terminal.sessions[0].status, SessionStatus::Starting);
    assert!(st.terminal.focus_requested);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn spawn_failure_is_shown_and_logged() {
    let (mut st, dir, id) = open_with_panel("term-failed");
    run(&mut st, T::SpawnFailed { id, error: "boom".into() });
    assert_eq!(st.terminal.sessions[0].status, SessionStatus::Failed("boom".into()));
    assert!(st.log.last().is_some_and(|l| !l.ok && l.text.contains("boom")));
    // Sync doesn't start another one behind the user's back.
    run(&mut st, T::Toggle);
    assert!(terminal(run(&mut st, T::Toggle)).is_empty());
    assert_eq!(st.terminal.sessions.len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn switching_tabs_starts_the_other_repos_shell_and_closing_kills_it() {
    let (a, b) = (scratch_repo("term-tabs-a"), scratch_repo("term-tabs-b"));
    let mut st = state();
    drive(&mut st, Msg::Open(a.clone()));
    drive(&mut st, Msg::Open(b.clone()));
    drive(&mut st, Msg::SelectTab(0));
    run(&mut st, T::Toggle);
    let wa = workdir(&st);
    let id1 = st.terminal.sessions[0].id;

    let fx = terminal(update(&mut st, Msg::SelectTab(1), Instant::now()));
    let wb = workdir(&st);
    assert_ne!(wa, wb);
    let id2 = match &fx[..] {
        [E::Spawn { id, cwd, .. }] if *cwd == wb => *id,
        other => panic!("{other:?}"),
    };
    assert_eq!(st.terminal.sessions.len(), 2);
    assert!(terminal(update(&mut st, Msg::Refresh, Instant::now())).is_empty(), "one shell per repository");

    // The hidden tab's shell keeps running and its output is still parsed.
    run(&mut st, T::Output { id: id1, bytes: b"from a".to_vec() });
    assert_eq!(st.terminal.for_workdir(&wa).unwrap().screen.text(), "from a");

    let fx = terminal(update(&mut st, Msg::CloseTab(1), Instant::now()));
    assert!(matches!(&fx[..], [E::Kill(k), E::Close(c)] if *k == id2 && *c == id2), "{fx:?}");
    assert!(st.terminal.for_workdir(&wb).is_none());
    let s = st.terminal.for_workdir(&wa).expect("tab 1's session stays");
    assert_eq!(s.id, id1);
    assert_eq!(s.status, SessionStatus::Running);
    let _ = std::fs::remove_dir_all(&a);
    let _ = std::fs::remove_dir_all(&b);
}
