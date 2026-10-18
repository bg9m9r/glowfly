//! Headless tests for what the terminal grid asks of its active tab (scrolling by rows and by
//! keyboard steps, copying, selecting all, clearing, pasting) and for the panel's remembered state and shell choice.

use super::terminal_tests::{open_with_panel, run, terminal};
use super::tests::scratch_repo;
use crate::effect::{Effect, UiEffect};
use crate::msg::{Msg, RepoMsg};
use crate::state::AppState;
use crate::state::terminal::Selection;
use crate::terminal::{ScrollStep, TerminalEffect as E, TerminalMsg as T};
use std::time::Instant;
use trellis_core::settings::{Settings, ShellChoice, ShellSetting};
use trellis_core::terminal::{TermInput, TermPoint};

/// The panel open on a running pwsh that has printed lines "1" to "60" (36 rows of history on
/// the 24-row screen).
fn with_history(name: &str) -> (AppState, std::path::PathBuf, u64) {
    let (mut st, dir, id) = open_with_panel(name);
    run(&mut st, T::Started { id, program: "pwsh".into() });
    let lines: Vec<String> = (1..=60).map(|i| i.to_string()).collect();
    run(&mut st, T::Output { id, bytes: lines.join("\r\n").into_bytes(), ack: Default::default() });
    (st, dir, id)
}

fn copied(fx: &[Effect]) -> Option<&str> {
    fx.iter().find_map(|e| match e {
        Effect::Ui(UiEffect::CopyText(t)) => Some(t.as_str()),
        _ => None,
    })
}

fn saves(fx: &[Effect]) -> usize {
    fx.iter().filter(|e| matches!(e, Effect::SaveSettings(_))).count()
}

/// Selects lines `a..=b` (numbered from the first history row) of session 0.
fn select(st: &mut AppState, a: u64, b: u64) {
    let s = &mut st.terminal.sessions[0];
    let first = s.screen.first_line();
    let anchor = TermPoint { line: first + a, col: 0 };
    let head = TermPoint { line: first + b, col: 80 };
    s.selection = Some(Selection { anchor, head, epoch: s.screen.epoch() });
}

#[test]
fn scroll_and_typing_snap_back() {
    let (mut st, dir, _) = with_history("term-view-scroll");
    assert_eq!(st.terminal.sessions[0].screen.history_len(), 36);
    run(&mut st, T::Scroll(5));
    assert_eq!(st.terminal.sessions[0].screen.view(), 5);
    run(&mut st, T::Scroll(-2));
    assert_eq!(st.terminal.sessions[0].screen.view(), 3);
    select(&mut st, 0, 1);
    assert!(run(&mut st, T::Input(Vec::new())).is_empty());
    assert_eq!(st.terminal.sessions[0].screen.view(), 3, "no bytes, no jump");
    run(&mut st, T::Input(vec![TermInput::Text("x".into())]));
    let s = &st.terminal.sessions[0];
    assert_eq!((s.screen.view(), s.selection), (0, None));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn keyboard_steps_page_through_the_history() {
    let (mut st, dir, id) = with_history("term-view-page");
    let view = |st: &AppState| st.terminal.sessions[0].screen.view();
    run(&mut st, T::ScrollPage(ScrollStep::PageUp));
    assert_eq!(view(&st), 24, "one screen of rows");
    run(&mut st, T::ScrollPage(ScrollStep::PageUp));
    assert_eq!(view(&st), 36, "clamped at the start of the history");
    run(&mut st, T::ScrollPage(ScrollStep::PageDown));
    assert_eq!(view(&st), 12);
    run(&mut st, T::ScrollPage(ScrollStep::PageDown));
    assert_eq!(view(&st), 0, "clamped at the live screen");
    run(&mut st, T::ScrollPage(ScrollStep::Top));
    assert_eq!(view(&st), st.terminal.sessions[0].screen.history_len());
    run(&mut st, T::ScrollPage(ScrollStep::Bottom));
    assert_eq!(view(&st), 0);

    // The alternate screen has no history to move through.
    run(&mut st, T::Output { id, bytes: b"\x1b[?1049hless".to_vec(), ack: Default::default() });
    for step in [ScrollStep::PageUp, ScrollStep::Top, ScrollStep::PageDown, ScrollStep::Bottom] {
        run(&mut st, T::ScrollPage(step));
        assert_eq!(view(&st), 0, "{step:?}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn select_all_copies_with_copy_on_select() {
    let (mut st, dir, _) = with_history("term-view-select-copy");
    assert_eq!(copied(&run(&mut st, T::SelectAll)), None, "off by default");
    st.settings.terminal.copy_on_select = true;
    let fx = run(&mut st, T::SelectAll);
    let all: Vec<String> = (1..=60).map(|i| i.to_string()).collect();
    assert_eq!(copied(&fx), Some(all.join("\n").as_str()));
    assert!(st.terminal.sessions[0].selection.is_some(), "it stays selected");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn copy_select_all_and_stale_selections() {
    let (mut st, dir, _) = with_history("term-view-copy");
    select(&mut st, 1, 3);
    st.terminal.focus_requested = false;
    let fx = run(&mut st, T::CopySelection { keep: true });
    assert_eq!(copied(&fx), Some("2\n3\n4"));
    assert!(st.terminal.sessions[0].selection.is_some() && st.terminal.focus_requested);
    let fx = run(&mut st, T::CopySelection { keep: false });
    assert_eq!(copied(&fx), Some("2\n3\n4"));
    assert!(st.terminal.sessions[0].selection.is_none());

    run(&mut st, T::SelectAll);
    let s = &st.terminal.sessions[0];
    let sel = s.selection.expect("selected");
    assert_eq!(sel.anchor, TermPoint { line: s.screen.first_line(), col: 0 });
    let fx = run(&mut st, T::CopySelection { keep: true });
    let all: Vec<String> = (1..=60).map(|i| i.to_string()).collect();
    assert_eq!(copied(&fx), Some(all.join("\n").as_str()));

    // A resize renumbers the lines: the old selection copies nothing.
    run(&mut st, T::Resize { rows: 20, cols: 70 });
    select(&mut st, 0, 2);
    st.terminal.sessions[0].selection.as_mut().unwrap().epoch -= 1;
    assert_eq!(copied(&run(&mut st, T::CopySelection { keep: false })), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn clear_per_shell() {
    let (mut st, dir, id) = with_history("term-view-clear");
    let fx = terminal(run(&mut st, T::Clear));
    assert!(matches!(&fx[..], [E::Write { id: i, bytes }] if *i == id && bytes == &[0x0c]), "{fx:?}");
    assert_eq!(st.terminal.sessions[0].screen.history_len(), 0);
    assert!(st.terminal.focus_requested);

    run(&mut st, T::Output { id, bytes: b"\r\nC:\\demo>".to_vec(), ack: Default::default() });
    st.terminal.sessions[0].program = "cmd".into();
    assert!(terminal(run(&mut st, T::Clear)).is_empty(), "cmd doesn't redraw on Ctrl+L");
    assert_eq!(st.terminal.sessions[0].screen.text(), "C:\\demo>");

    run(&mut st, T::Output { id, bytes: b"\x1b[?1049hvim".to_vec(), ack: Default::default() });
    let before = st.terminal.sessions[0].screen.text();
    assert!(run(&mut st, T::Clear).is_empty(), "nothing on the alternate screen");
    assert_eq!(st.terminal.sessions[0].screen.text(), before);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn paste_asks_the_window_for_the_clipboard() {
    let (mut st, dir, id) = with_history("term-view-paste");
    st.terminal.focus_requested = false;
    let fx = run(&mut st, T::Paste);
    assert!(matches!(&fx[..], [Effect::Ui(UiEffect::RequestPaste)]), "{fx:?}");
    assert!(st.terminal.focus_requested);
    run(&mut st, T::Exited { id, code: Some(0) });
    assert!(run(&mut st, T::Paste).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_panel_state_is_saved_when_it_changes() {
    let (mut st, dir, _) = open_with_panel("term-view-remember");
    assert!(st.settings.terminal.panel_open, "Toggle saved it open");
    assert_eq!(saves(&run(&mut st, T::Toggle)), 1);
    assert!(!st.settings.terminal.panel_open);
    assert_eq!(saves(&run(&mut st, T::Hide)), 0, "already hidden");

    assert_eq!(saves(&run(&mut st, T::PanelHeight(301.4))), 1);
    assert_eq!(st.settings.terminal.panel_height, Some(301.0));
    assert!(run(&mut st, T::PanelHeight(301.0)).is_empty());
    run(&mut st, T::PanelHeight(40.0));
    assert_eq!(st.settings.terminal.panel_height, Some(90.0));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_remembered_panel_starts_its_shell_with_the_chosen_shell() {
    let mut settings = Settings::default();
    settings.terminal.panel_open = true;
    let shell = ShellSetting { choice: ShellChoice::Custom, program: "nu".into(), args: "-l".into() };
    settings.terminal.shell = shell.clone();
    let mut st = AppState::new(settings, Vec::new(), Instant::now());
    assert!(st.terminal.open);
    let dir = scratch_repo("term-view-restored");
    let fx = super::update(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())), Instant::now());
    super::tests::finish(&mut st, fx);
    assert_eq!(st.terminal.sessions.len(), 1, "one shell for the restored panel");
    assert!(!st.terminal.focus_requested, "the keyboard stays where it was");
    // A new tab carries the chosen shell to the runner.
    let fx = terminal(run(&mut st, T::NewTab));
    assert!(matches!(&fx[..], [E::Spawn { shell: s, .. }] if *s == shell), "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
