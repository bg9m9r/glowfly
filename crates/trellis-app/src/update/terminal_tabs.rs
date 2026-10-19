//! Terminal tabs: several shells per repository, numbered "Terminal N" with the lowest free
//! number. New tab, switching, closing (asking first while the shell runs a program; the
//! neighbour takes over; the last one hides the panel), Restart in place, and Kill all after a
//! confirmation.

use super::terminal::{active_index, remember_open, spawn, sync};
use crate::effect::Effect;
use crate::msg::Msg;
use crate::state::terminal::SessionStatus;
use crate::state::{AppState, Modal};
use crate::terminal::{TerminalEffect as E, TerminalMsg};
use std::path::PathBuf;
use trellis_core::terminal::ShellActivity;

fn workdir(st: &AppState) -> Option<PathBuf> {
    st.snap.as_ref().map(|s| s.workdir.clone())
}

/// A new shell in the active repository, which becomes its active tab. A hidden panel opens
/// first; if the repository had no shell, the one that opening starts is the new tab.
pub(super) fn new_tab(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(workdir) = workdir(st) else { return };
    st.terminal.focus_requested = true;
    if !st.terminal.open {
        st.terminal.open = true;
        remember_open(st, fx);
        if st.terminal.sessions_for(&workdir).next().is_none() {
            sync(st, fx);
            return;
        }
    }
    let number = st.terminal.next_number(&workdir);
    spawn(st, workdir, number, None, fx);
}

/// Makes session `id` of the active repository its active tab.
pub(super) fn select_tab(st: &mut AppState, id: u64) {
    let Some(workdir) = workdir(st) else { return };
    if st.terminal.sessions_for(&workdir).any(|s| s.id == id) {
        st.terminal.active.insert(workdir, id);
        st.terminal.focus_requested = true;
    }
}

/// Closes session `id`'s tab, first asking the runner whether its shell is running a program
/// when it runs at all (the answer comes back to `activity`).
pub(super) fn close_tab(st: &mut AppState, id: u64, fx: &mut Vec<Effect>) {
    let Some(i) = st.terminal.index_of(id) else { return };
    if st.terminal.sessions[i].status == SessionStatus::Running {
        fx.push(Effect::Terminal(E::CheckActivity(id)));
    } else {
        close_now(st, id, fx);
    }
}

/// What session `id`'s shell was doing when its tab was asked to close: a running program
/// asks before closing (unless another dialog is open, when the click is dropped); anything
/// else closes it.
pub(super) fn activity(st: &mut AppState, id: u64, activity: ShellActivity, fx: &mut Vec<Effect>) {
    let Some(i) = st.terminal.index_of(id) else { return };
    let s = &st.terminal.sessions[i];
    let (running, n) = (s.status == SessionStatus::Running, s.number);
    let ShellActivity::Running { program } = activity else { return close_now(st, id, fx) };
    if !running {
        return close_now(st, id, fx);
    }
    if st.modal.is_some() {
        return;
    }
    let body = match program {
        Some(p) => format!("Terminal {n} is running {p}. Close it and stop {p}?"),
        None => format!("Terminal {n} is running a program. Close it and stop it?"),
    };
    st.modal = Some(Modal::danger("Close terminal", body, "Close terminal", Msg::Terminal(TerminalMsg::CloseTabNow(id))));
}

/// Stops session `id`'s shell if it runs and removes its tab; the tab to its right becomes
/// active, else the one to its left. Closing the last one hides the panel.
pub(super) fn close_now(st: &mut AppState, id: u64, fx: &mut Vec<Effect>) {
    let Some(i) = st.terminal.index_of(id) else { return };
    let old = st.terminal.sessions.remove(i);
    let was_active = st.terminal.active.get(&old.workdir) == Some(&id);
    if old.alive() {
        fx.push(Effect::Terminal(E::Kill(id)));
    }
    fx.push(Effect::Terminal(E::Close(id)));
    let workdir = old.workdir;
    if !was_active && st.terminal.sessions_for(&workdir).next().is_some() {
        // Another tab was active and stays so.
        return;
    }
    let right = st.terminal.sessions[i..].iter().find(|s| s.workdir == workdir);
    let left = st.terminal.sessions[..i].iter().rev().find(|s| s.workdir == workdir);
    match right.or(left).map(|s| s.id) {
        Some(next) => {
            st.terminal.active.insert(workdir, next);
            st.terminal.focus_requested = true;
        }
        None => {
            st.terminal.active.remove(&workdir);
            if workdir_is_active(st, &workdir) {
                st.terminal.open = false;
                remember_open(st, fx);
            }
        }
    }
}

fn workdir_is_active(st: &AppState, workdir: &std::path::Path) -> bool {
    st.snap.as_ref().is_some_and(|s| s.workdir == workdir)
}

/// Starts a new shell in the active tab's place after its shell ended: same number, same
/// position, still active.
pub(super) fn restart(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(i) = active_index(st) else { return };
    if st.terminal.sessions[i].alive() {
        return;
    }
    let old = st.terminal.sessions.remove(i);
    fx.push(Effect::Terminal(E::Close(old.id)));
    spawn(st, old.workdir, old.number, Some(i), fx);
    st.terminal.focus_requested = true;
}

/// The shells of the active repository that still run.
fn alive_ids(st: &AppState) -> Vec<u64> {
    let Some(workdir) = st.snap.as_ref().map(|s| &s.workdir) else { return Vec::new() };
    st.terminal.sessions_for(workdir).filter(|s| s.alive()).map(|s| s.id).collect()
}

/// Asks before Kill all; only offered while two or more shells run.
pub(super) fn ask_kill_all(st: &mut AppState) {
    let n = alive_ids(st).len();
    if n < 2 {
        return;
    }
    st.modal = Some(Modal::danger(
        "Kill all terminals",
        format!("Stop {n} terminals? Everything running in them stops too."),
        "Kill all",
        Msg::Terminal(TerminalMsg::KillAll),
    ));
}

/// Stops every running shell of the active repository; their tabs stay, with Restart.
pub(super) fn kill_all(st: &mut AppState, fx: &mut Vec<Effect>) {
    for id in alive_ids(st) {
        fx.push(Effect::Terminal(E::Kill(id)));
    }
}
