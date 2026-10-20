//! Headless tests for the terminal header's state: renaming a tab in place (the draft, the
//! cleaned name, going back to the automatic name) and maximising the panel (only while it
//! shows, cleared when it hides, and covering the graph).

use super::terminal_tests::{open_with_panel, run, workdir};
use crate::state::AppState;
use crate::state::terminal::{SessionStatus, TabRename, TermSession, maximized_shown};
use crate::state::terminal_names::{auto_name, display_name};
use crate::terminal::TerminalMsg as T;
use trellis_core::settings::ShellSetting;
use trellis_core::terminal::TermScreen;

/// The repository's shell reported as pwsh, as the runner does once it runs.
fn started(st: &mut AppState, id: u64) {
    run(st, T::Started { id, program: "pwsh".into() });
}

fn session(st: &AppState, id: u64) -> &TermSession {
    st.terminal.sessions.iter().find(|s| s.id == id).expect("the session")
}

fn folder(st: &AppState) -> String {
    workdir(st).file_name().unwrap().to_string_lossy().into_owned()
}

#[test]
fn start_rename_fills_the_draft_and_selects_the_tab() {
    let (mut st, dir, one) = open_with_panel("term-rename-start");
    started(&mut st, one);
    run(&mut st, T::NewTab(None));
    st.terminal.focus_requested = false;
    run(&mut st, T::StartRename(one));
    let want = format!("pwsh \u{b7} {}", folder(&st));
    assert_eq!(st.terminal.renaming, Some(TabRename { id: one, text: want, focus: true }));
    assert_eq!(st.terminal.active_id(&workdir(&st)), Some(one));
    assert!(!st.terminal.focus_requested, "the field takes the keyboard, not the shell");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rename_sets_a_cleaned_name_and_gives_the_shell_the_keyboard() {
    let (mut st, dir, one) = open_with_panel("term-rename-set");
    started(&mut st, one);
    run(&mut st, T::StartRename(one));
    st.terminal.focus_requested = false;
    run(&mut st, T::Rename { id: one, name: "  build  ".into() });
    assert_eq!(session(&st, one).name.as_deref(), Some("build"));
    assert_eq!(display_name(session(&st, one)), "build");
    assert!(st.terminal.renaming.is_none());
    assert!(st.terminal.focus_requested);

    run(&mut st, T::StartRename(one));
    run(&mut st, T::Rename { id: one, name: "x".repeat(100) });
    assert_eq!(session(&st, one).name.as_ref().map(|n| n.len()), Some(64));

    run(&mut st, T::StartRename(one));
    run(&mut st, T::Rename { id: one, name: "   ".into() });
    assert_eq!(session(&st, one).name, None, "blank goes back to the automatic name");

    run(&mut st, T::StartRename(one));
    run(&mut st, T::Rename { id: one, name: "build".into() });
    let auto = auto_name(session(&st, one));
    run(&mut st, T::StartRename(one));
    run(&mut st, T::Rename { id: one, name: auto });
    assert_eq!(session(&st, one).name, None, "the automatic name is not stored");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_stale_rename_is_ignored_and_cancel_clears_the_draft() {
    let (mut st, dir, one) = open_with_panel("term-rename-stale");
    started(&mut st, one);
    run(&mut st, T::NewTab(None));
    let two = st.terminal.sessions[1].id;
    run(&mut st, T::StartRename(two));
    run(&mut st, T::Rename { id: one, name: "other".into() });
    assert_eq!(session(&st, one).name, None, "not the draft's tab");
    assert!(st.terminal.renaming.is_some());
    st.terminal.focus_requested = false;
    run(&mut st, T::CancelRename);
    assert!(st.terminal.renaming.is_none());
    assert!(st.terminal.focus_requested);
    assert_eq!(session(&st, two).name, None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn another_repositorys_tab_is_not_renamed() {
    let (mut st, dir, _) = open_with_panel("term-rename-other");
    let other = trellis_core::paths::temp_dir().join("trellis-elsewhere");
    st.terminal.sessions.push(TermSession {
        id: 99,
        number: 1,
        workdir: other,
        program: "bash".into(),
        shell: ShellSetting::default(),
        name: None,
        screen: TermScreen::new(24, 80),
        status: SessionStatus::Running,
        selection: None,
    });
    run(&mut st, T::StartRename(99));
    assert!(st.terminal.renaming.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn maximise_toggles_only_while_the_panel_shows() {
    let (mut st, dir, _) = open_with_panel("term-max-toggle");
    run(&mut st, T::ToggleMaximize);
    assert!(st.terminal.maximized && maximized_shown(&st));
    assert!(st.terminal.focus_requested);
    run(&mut st, T::ToggleMaximize);
    assert!(!st.terminal.maximized);
    run(&mut st, T::Hide);
    run(&mut st, T::ToggleMaximize);
    assert!(!st.terminal.maximized, "a hidden panel isn't maximised");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn hiding_clears_maximise() {
    let (mut st, dir, one) = open_with_panel("term-max-hide");
    run(&mut st, T::ToggleMaximize);
    run(&mut st, T::Hide);
    assert!(!st.terminal.maximized, "Hide");
    run(&mut st, T::Toggle);
    run(&mut st, T::ToggleMaximize);
    run(&mut st, T::Toggle);
    assert!(!st.terminal.open && !st.terminal.maximized, "Toggle that hides");
    run(&mut st, T::Toggle);
    run(&mut st, T::ToggleMaximize);
    run(&mut st, T::Exited { id: one, code: Some(0) });
    run(&mut st, T::CloseTab(one));
    assert!(!st.terminal.open && !st.terminal.maximized, "closing the last tab");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_maximised_panel_covers_the_graph_only_on_a_repository_view() {
    let (mut st, dir, _) = open_with_panel("term-max-graph");
    assert!(st.graph_shown());
    run(&mut st, T::ToggleMaximize);
    assert!(!st.graph_shown(), "the graph isn't drawn under a maximised panel");
    st.dashboard.open = true;
    assert!(!maximized_shown(&st), "the dashboard shows instead");
    st.dashboard.open = false;
    assert!(maximized_shown(&st));
    let snap = st.snap.take();
    assert!(!maximized_shown(&st), "no repository");
    st.snap = snap;
    run(&mut st, T::ToggleMaximize);
    assert!(st.graph_shown(), "restored");
    let _ = std::fs::remove_dir_all(&dir);
}
