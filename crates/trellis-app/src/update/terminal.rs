//! The built-in terminal panel: showing and hiding it (remembered in the settings), a first
//! shell per repository (started the first time the panel shows that repository), keyboard
//! input, resizing, and the shells' output and exit. Terminal tabs are in terminal_tabs.rs, and
//! scrolling, the selection and the grid's menu in terminal_view.rs. Workdirs are compared as
//! plain paths: no filesystem access here.

use crate::effect::Effect;
use crate::state::AppState;
use crate::state::terminal::{DEFAULT_SIZE, SessionStatus, TermSession};
use crate::terminal::{AfterCheck, TerminalEffect as E, TerminalMsg};
use std::path::{Path, PathBuf};
use trellis_core::terminal::{TermScreen, encode};

pub(super) fn handle(st: &mut AppState, m: TerminalMsg, fx: &mut Vec<Effect>) {
    use super::{terminal_tabs as tabs, terminal_view as view};
    match m {
        TerminalMsg::Toggle => {
            if st.snap.is_none() {
                return;
            }
            if st.terminal.open {
                // Hiding never stops a shell.
                st.terminal.open = false;
            } else {
                st.terminal.open = true;
                st.terminal.focus_requested = true;
                sync(st, fx);
            }
            remember_open(st, fx);
        }
        TerminalMsg::Hide => {
            st.terminal.open = false;
            remember_open(st, fx);
        }
        TerminalMsg::Kill => tabs::kill(st, fx),
        TerminalMsg::KillNow(id) => tabs::kill_now(st, id, fx),
        TerminalMsg::Restart => tabs::restart(st, fx),
        TerminalMsg::Input(inputs) => {
            // Input after the shell has ended goes nowhere.
            let Some(s) = active_mut(st).filter(|s| s.alive()) else { return };
            let modes = s.screen.modes();
            let bytes: Vec<u8> = inputs.iter().flat_map(|i| encode(i, modes)).collect();
            if !bytes.is_empty() {
                s.screen.scroll_to_bottom();
                s.selection = None;
                fx.push(Effect::Terminal(E::Write { id: s.id, bytes }));
            }
        }
        TerminalMsg::Resize { rows, cols } => {
            st.terminal.grid = Some((rows, cols));
            let Some(s) = active_mut(st) else { return };
            if s.screen.size() == (rows, cols) {
                return;
            }
            // An ended session's last screen follows the panel too, without a PTY to resize.
            s.screen.resize(rows, cols);
            s.selection = None;
            if s.alive() {
                fx.push(Effect::Terminal(E::Resize { id: s.id, rows, cols }));
            }
        }
        TerminalMsg::NewTab => tabs::new_tab(st, fx),
        TerminalMsg::SelectTab(id) => tabs::select_tab(st, id),
        TerminalMsg::CloseTab(id) => tabs::close_tab(st, id, fx),
        TerminalMsg::CloseTabNow(id) => tabs::close_now(st, id, fx),
        TerminalMsg::AskKillAll => tabs::ask_kill_all(st),
        TerminalMsg::KillAll => tabs::kill_all(st, fx),
        TerminalMsg::Scroll(rows) => view::scroll(st, rows),
        TerminalMsg::ScrollPage(step) => view::scroll_page(st, step),
        TerminalMsg::CopySelection { keep } => view::copy(st, keep, fx),
        TerminalMsg::SelectAll => view::select_all(st, fx),
        TerminalMsg::Clear => view::clear(st, fx),
        TerminalMsg::Paste => view::paste(st, fx),
        TerminalMsg::PanelHeight(h) => view::panel_height(st, h, fx),
        TerminalMsg::Started { id, program } => {
            if let Some(s) = session_mut(st, id) {
                s.program = program;
                // The exit can be reported first when the shell ends at once.
                if s.status == SessionStatus::Starting {
                    s.status = SessionStatus::Running;
                }
            }
        }
        TerminalMsg::Output { id, bytes, ack } => {
            // Fed whichever tab the session belongs to, and even after its exit was reported.
            let Some(s) = session_mut(st, id) else { return };
            let reply = s.screen.feed(&bytes);
            // Handled: the session may send what has come since.
            drop(ack);
            if s.status == SessionStatus::Starting {
                s.status = SessionStatus::Running;
            }
            if !reply.is_empty() && s.alive() {
                fx.push(Effect::Terminal(E::Write { id, bytes: reply }));
            }
        }
        TerminalMsg::Exited { id, code } => {
            if let Some(s) = session_mut(st, id) {
                s.status = SessionStatus::Exited(code);
            }
            fx.push(Effect::Terminal(E::Close(id)));
        }
        TerminalMsg::SpawnFailed { id, error } => {
            let Some(s) = session_mut(st, id) else { return };
            s.status = SessionStatus::Failed(error.clone());
            fx.push(Effect::Terminal(E::Close(id)));
            st.log(false, format!("Could not start the terminal: {error}"));
        }
        TerminalMsg::Activity { id, activity, then } => match then {
            AfterCheck::Close => tabs::activity(st, id, activity, fx),
            AfterCheck::Kill => tabs::kill_activity(st, id, activity, fx),
        },
    }
}

/// Saves whether the panel is open, when that changed.
pub(super) fn remember_open(st: &mut AppState, fx: &mut Vec<Effect>) {
    if st.settings.terminal.panel_open != st.terminal.open {
        st.settings.terminal.panel_open = st.terminal.open;
        fx.push(super::prefs::save_settings(st));
    }
}

/// Starts a shell for the active repository if the panel is open and it has none yet. A shell
/// that ended (or failed to start) counts: it is restarted only when the user asks.
pub(super) fn sync(st: &mut AppState, fx: &mut Vec<Effect>) {
    if !st.terminal.open {
        return;
    }
    let Some(workdir) = st.snap.as_ref().map(|s| s.workdir.clone()) else { return };
    if st.terminal.sessions_for(&workdir).next().is_none() {
        spawn(st, workdir, 1, None, fx);
    }
}

/// The tab for `path` is closing, along with the tabs at `closing` (indices into `st.tabs`,
/// it among them): stop its shells and forget their sessions, unless a tab that stays open
/// shows the same working directory and so still uses them.
pub(super) fn tab_closed(st: &mut AppState, path: &Path, closing: &[usize], fx: &mut Vec<Effect>) {
    let shared = st.tabs.iter().enumerate().any(|(j, t)| !closing.contains(&j) && t.path.as_deref() == Some(path));
    if shared {
        return;
    }
    let ids: Vec<u64> = st.terminal.sessions_for(path).map(|s| s.id).collect();
    st.terminal.sessions.retain(|s| s.workdir != path);
    st.terminal.active.remove(path);
    for id in ids {
        // Kill reaches the session's thread before Close drops the runner's handle on it.
        fx.push(Effect::Terminal(E::Kill(id)));
        fx.push(Effect::Terminal(E::Close(id)));
    }
}

/// Starts a shell numbered `number` in `workdir`, at `index` in the sessions (else at the end),
/// and makes it the repository's active tab. Returns its id.
pub(super) fn spawn(st: &mut AppState, workdir: PathBuf, number: u32, index: Option<usize>, fx: &mut Vec<Effect>) -> u64 {
    let id = st.terminal.next_id;
    st.terminal.next_id += 1;
    let (rows, cols) = st.terminal.grid.unwrap_or(DEFAULT_SIZE);
    let session = TermSession {
        id,
        number,
        workdir: workdir.clone(),
        program: String::new(),
        screen: TermScreen::new(rows, cols),
        status: SessionStatus::Starting,
        selection: None,
    };
    match index {
        Some(i) if i <= st.terminal.sessions.len() => st.terminal.sessions.insert(i, session),
        _ => st.terminal.sessions.push(session),
    }
    st.terminal.active.insert(workdir.clone(), id);
    let shell = st.settings.terminal.shell.clone();
    fx.push(Effect::Terminal(E::Spawn { id, cwd: workdir, rows, cols, shell }));
    id
}

/// The active repository's active session, as an index into the sessions.
pub(super) fn active_index(st: &AppState) -> Option<usize> {
    let id = st.terminal.active_id(&st.snap.as_ref()?.workdir)?;
    st.terminal.index_of(id)
}

pub(super) fn active_mut(st: &mut AppState) -> Option<&mut TermSession> {
    let i = active_index(st)?;
    st.terminal.sessions.get_mut(i)
}

fn session_mut(st: &mut AppState, id: u64) -> Option<&mut TermSession> {
    let i = st.terminal.index_of(id)?;
    st.terminal.sessions.get_mut(i)
}
