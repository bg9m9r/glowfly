//! Closing tabs and reopening closed ones. A close that would lose work held only in memory (a
//! commit message draft, conflict choices not saved yet, an edited dialog, comments not posted;
//! `state/tab_losses.rs`) or end a
//! terminal running a program (asked about first, update/terminal_busy.rs) asks first; the
//! confirmation names its tabs by id, so it closes the right ones if they moved meanwhile.

use super::tabs::{activate, open, park, save};
use crate::effect::{BackgroundEffect, Effect};
use crate::msg::{CloseRequest, Msg, TabsMsg};
use crate::state::AppState;
use crate::state::dialog_guard::open_over;
use crate::state::tab_losses::close_confirm;
use crate::state::tabs::{ClosedTab, MAX_CLOSED_TABS, Tab, TabId};
use crate::state::terminal::busy::{BusyTerminal, ids_for_tabs};
use crate::terminal::BusyCheck;
use std::path::{Path, PathBuf};

/// A close asked for tab `i` (`usize::MAX`: the active tab), through the menus, the tab bar or
/// a key. Out of range: the tab is gone, nothing happens.
pub(super) fn asked(st: &mut AppState, i: usize, kind: fn(TabId) -> CloseRequest, fx: &mut Vec<Effect>) {
    let i = if i == usize::MAX { st.active_tab } else { i };
    if let Some(id) = st.tabs.get(i).map(|t| t.id) {
        request(st, kind(id), false, fx);
    }
}

/// Closes what `req` names, unless that loses work or ends a terminal running a program and
/// isn't `confirmed` yet: then the confirmation opens instead (over any other dialog). Running
/// shells of the closing tabs are asked about first (the answer comes back to `checked`).
pub(super) fn request(st: &mut AppState, req: CloseRequest, confirmed: bool, fx: &mut Vec<Effect>) {
    decide(st, req, confirmed, None, fx);
}

/// The shells' answer for a close: `busy` are its terminals running a program.
pub(super) fn checked(st: &mut AppState, req: CloseRequest, busy: Vec<BusyTerminal>, fx: &mut Vec<Effect>) {
    decide(st, req, false, Some(busy), fx);
}

fn decide(st: &mut AppState, req: CloseRequest, confirmed: bool, busy: Option<Vec<BusyTerminal>>, fx: &mut Vec<Effect>) {
    let (CloseRequest::One(id) | CloseRequest::Others(id) | CloseRequest::Right(id)) = req;
    let Some(i) = st.tabs.iter().position(|t| t.id == id) else { return };
    let n = st.tabs.len();
    let closing: Vec<usize> = match req {
        CloseRequest::One(_) => vec![i],
        CloseRequest::Others(_) => (0..n).filter(|&j| j != i).collect(),
        CloseRequest::Right(_) => (i + 1..n).collect(),
    };
    if closing.is_empty() {
        return;
    }
    if !confirmed {
        let busy = match busy {
            Some(busy) => busy,
            None => {
                let ids = ids_for_tabs(st, &closing);
                if !ids.is_empty() {
                    super::terminal_busy::check(ids, BusyCheck::CloseTabs(req), fx);
                    return;
                }
                Vec::new()
            }
        };
        if let Some(modal) = close_confirm(st, &closing, &busy, Msg::Tabs(TabsMsg::CloseConfirmed(req))) {
            // An edited form stays underneath, and comes back on Cancel.
            open_over(&mut st.modal, &mut st.dialog, modal);
            return;
        }
    }
    match req {
        CloseRequest::One(_) => close(st, i, fx),
        CloseRequest::Others(_) => close_where(st, |j| j == i, i, fx),
        CloseRequest::Right(_) => close_where(st, |j| j <= i, i, fx),
    }
}

fn close(st: &mut AppState, i: usize, fx: &mut Vec<Effect>) {
    if i >= st.tabs.len() {
        return;
    }
    if let Some(path) = st.tabs[i].path.clone() {
        // The repository pool lets go of its open handles (on Windows they keep packs and
        // folders from being deleted).
        fx.push(Effect::Background(BackgroundEffect::Forget(path.clone())));
        super::terminal::tab_closed(st, &path, &[i], fx);
        super::history_index::tab_closed(st, i, fx);
        remember_closed(st, ClosedTab { path, label: st.tabs[i].label.clone() });
    }
    st.tab_return = None;
    if i != st.active_tab {
        super::large_graph::free(fx, "tab", Box::new(st.tabs.remove(i)));
        if i < st.active_tab {
            st.active_tab -= 1;
        }
        fx.push(save(st));
        return;
    }
    super::partial_content::leave(st, fx);
    // An edited form kept under the confirmation goes with its tab.
    st.dialog.drop_under();
    // Discarded with the tab, on a worker (a large history is many allocations).
    super::large_graph::free(fx, "tab", Box::new(park(st)));
    if st.tabs.len() == 1 {
        st.tabs[0] = Tab::empty();
        activate(st, 0, fx);
    } else {
        st.tabs.remove(i);
        activate(st, i.min(st.tabs.len() - 1), fx);
    }
}

/// Puts `tab` on top of the "Reopen closed tab" stack.
fn remember_closed(st: &mut AppState, tab: ClosedTab) {
    st.closed_tabs.retain(|c| c.path != tab.path);
    st.closed_tabs.push(tab);
    if st.closed_tabs.len() > MAX_CLOSED_TABS {
        st.closed_tabs.remove(0);
    }
}

/// Closes the tabs at `idx` (never the active one) without asking, as Close tab would once
/// confirmed: the caller has asked already (removing a worktree).
pub(super) fn close_tabs(st: &mut AppState, idx: &[usize], fx: &mut Vec<Effect>) {
    let active = st.active_tab;
    close_where(st, |j| j == active || !idx.contains(&j), active, fx);
}

/// Closes every tab for which `keep` is false. `survivor` (the tab the request came from) is
/// always kept, and becomes active if the active tab closes.
fn close_where(st: &mut AppState, keep: impl Fn(usize) -> bool, survivor: usize, fx: &mut Vec<Effect>) {
    let closing: Vec<usize> = (0..st.tabs.len()).filter(|&j| !keep(j)).collect();
    if closing.is_empty() {
        return;
    }
    // Right to left, so the leftmost closed tab is reopened first and the tabs come back in
    // their original order.
    for &j in closing.iter().rev() {
        if let Some(path) = st.tabs[j].path.clone() {
            fx.push(Effect::Background(BackgroundEffect::Forget(path.clone())));
            super::terminal::tab_closed(st, &path, &closing, fx);
            super::history_index::tab_closed(st, j, fx);
            remember_closed(st, ClosedTab { path, label: st.tabs[j].label.clone() });
        }
    }
    st.tab_return = None;
    let before = |i: usize| closing.iter().filter(|&&j| j < i).count();
    let new_survivor = survivor - before(survivor);
    if closing.contains(&st.active_tab) {
        super::partial_content::leave(st, fx);
        st.dialog.drop_under();
        park(st); // discarded with the tab
        for &j in closing.iter().rev() {
            st.tabs.remove(j);
        }
        activate(st, new_survivor, fx);
    } else {
        st.active_tab -= before(st.active_tab);
        for &j in closing.iter().rev() {
            st.tabs.remove(j);
        }
        fx.push(save(st));
    }
}

/// Reopens the most recently closed tab (Ctrl+Shift+T).
pub(super) fn reopen_last(st: &mut AppState, fx: &mut Vec<Effect>) {
    if let Some(closed) = st.closed_tabs.pop() {
        reopened(st, closed, fx);
    }
}

/// Reopens `path` chosen from the recently closed list (the tab list, the palette), taking it
/// off the list.
pub(super) fn reopen(st: &mut AppState, path: PathBuf, fx: &mut Vec<Effect>) {
    let closed = match st.closed_tabs.iter().position(|c| c.path == path) {
        Some(k) => st.closed_tabs.remove(k),
        None => ClosedTab::from(path),
    };
    reopened(st, closed, fx);
}

fn reopened(st: &mut AppState, closed: ClosedTab, fx: &mut Vec<Effect>) {
    open(st, closed.path.clone(), fx);
    if let Some(label) = closed.label {
        restore_label(st, &closed.path, label, fx);
    }
}

/// Gives the tab just reopened for `path` the label it had, unless it has one already.
fn restore_label(st: &mut AppState, path: &Path, label: String, fx: &mut Vec<Effect>) {
    let i = st.active_tab;
    let Some(tab) = st.tabs.get_mut(i).filter(|t| t.is(path) && t.label.is_none()) else { return };
    tab.label = Some(label);
    fx.push(save(st));
    fx.push(Effect::Ui(crate::effect::UiEffect::SetTitle(super::tabs::title(st, i))));
}
