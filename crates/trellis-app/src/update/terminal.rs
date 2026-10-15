//! The built-in terminal panel: showing and hiding it, one shell per repository (started the
//! first time the panel shows that repository), keyboard input, resizing, and the shells'
//! output and exit. Workdirs are compared as plain paths: no filesystem access here.

use crate::effect::Effect;
use crate::state::AppState;
use crate::state::terminal::{DEFAULT_SIZE, SessionStatus, TermSession};
use crate::terminal::{TerminalEffect as E, TerminalMsg};
use std::path::{Path, PathBuf};
use trellis_core::terminal::{TermScreen, encode};

pub(super) fn handle(st: &mut AppState, m: TerminalMsg, fx: &mut Vec<Effect>) {
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
        }
        TerminalMsg::Hide => st.terminal.open = false,
        TerminalMsg::Kill => {
            if let Some(s) = active_mut(st).filter(|s| s.alive()) {
                fx.push(Effect::Terminal(E::Kill(s.id)));
            }
        }
        TerminalMsg::Restart => {
            let Some(i) = active_index(st) else { return };
            if st.terminal.sessions[i].alive() {
                return;
            }
            let old = st.terminal.sessions.remove(i);
            fx.push(Effect::Terminal(E::Close(old.id)));
            spawn(st, old.workdir, fx);
            st.terminal.focus_requested = true;
        }
        TerminalMsg::Input(inputs) => {
            // Input after the shell has ended goes nowhere.
            let Some(s) = active_mut(st).filter(|s| s.alive()) else { return };
            let modes = s.screen.modes();
            let bytes: Vec<u8> = inputs.iter().flat_map(|i| encode(i, modes)).collect();
            if !bytes.is_empty() {
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
            if s.alive() {
                fx.push(Effect::Terminal(E::Resize { id: s.id, rows, cols }));
            }
        }
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
    }
}

/// Starts a shell for the active repository if the panel is open and it has none yet. A shell
/// that ended (or failed to start) counts: it is restarted only when the user asks.
pub(super) fn sync(st: &mut AppState, fx: &mut Vec<Effect>) {
    if !st.terminal.open {
        return;
    }
    let Some(workdir) = st.snap.as_ref().map(|s| s.workdir.clone()) else { return };
    if st.terminal.for_workdir(&workdir).is_none() {
        spawn(st, workdir, fx);
    }
}

/// The tab for `path` is closing, along with the tabs at `closing` (indices into `st.tabs`,
/// it among them): stop its shell and forget the session, unless a tab that stays open shows
/// the same working directory and so still uses it.
pub(super) fn tab_closed(st: &mut AppState, path: &Path, closing: &[usize], fx: &mut Vec<Effect>) {
    let shared = st.tabs.iter().enumerate().any(|(j, t)| !closing.contains(&j) && t.path.as_deref() == Some(path));
    if shared {
        return;
    }
    let ids: Vec<u64> = st.terminal.sessions.iter().filter(|s| s.workdir == path).map(|s| s.id).collect();
    st.terminal.sessions.retain(|s| s.workdir != path);
    for id in ids {
        // Kill reaches the session's thread before Close drops the runner's handle on it.
        fx.push(Effect::Terminal(E::Kill(id)));
        fx.push(Effect::Terminal(E::Close(id)));
    }
}

fn spawn(st: &mut AppState, workdir: PathBuf, fx: &mut Vec<Effect>) {
    let id = st.terminal.next_id;
    st.terminal.next_id += 1;
    let (rows, cols) = st.terminal.grid.unwrap_or(DEFAULT_SIZE);
    st.terminal.sessions.push(TermSession {
        id,
        workdir: workdir.clone(),
        program: String::new(),
        screen: TermScreen::new(rows, cols),
        status: SessionStatus::Starting,
    });
    fx.push(Effect::Terminal(E::Spawn { id, cwd: workdir, rows, cols }));
}

fn active_index(st: &AppState) -> Option<usize> {
    let workdir = &st.snap.as_ref()?.workdir;
    st.terminal.sessions.iter().position(|s| &s.workdir == workdir)
}

fn active_mut(st: &mut AppState) -> Option<&mut TermSession> {
    let i = active_index(st)?;
    st.terminal.sessions.get_mut(i)
}

fn session_mut(st: &mut AppState, id: u64) -> Option<&mut TermSession> {
    let i = st.terminal.index_of(id)?;
    st.terminal.sessions.get_mut(i)
}
