//! Per-tab activity logs: earlier entries handed to their tabs, lines carried from a discarded
//! tab, and the Details toast that announces a failure (state/logs.rs keeps the lines).

use crate::effect::Effect;
use crate::logs::{LogKind, LogsMsg};
use crate::msg::{Msg, TabsMsg};
use crate::state::logs::{LOG_LIMIT, LogEntry, keep_newest};
use crate::state::tabs::TabId;
use crate::state::{AppState, ToastAction};
use std::time::Instant;
use trellis_core::logs::ActivityEntry;

/// Hands the earlier sessions' entries to the tabs they were logged in: a tab showing their
/// repository, the active tab for those logged without one, and `unclaimed` for a repository no
/// tab shows yet.
pub(super) fn distribute(st: &mut AppState, entries: Vec<ActivityEntry>) {
    let mut per_tab: Vec<Vec<ActivityEntry>> = st.tabs.iter().map(|_| Vec::new()).collect();
    let mut here = Vec::new();
    for e in entries {
        let Some(repo) = e.repo.as_deref() else {
            here.push(e);
            continue;
        };
        match st.tabs.iter().position(|t| t.is(repo)) {
            Some(i) if i == st.active_tab => here.push(e),
            Some(i) => per_tab[i].push(e),
            None => st.logs.unclaimed.push(e),
        }
    }
    prepend(&mut st.log, here);
    for (tab, earlier) in st.tabs.iter_mut().zip(per_tab) {
        prepend(&mut tab.log, earlier);
    }
}

/// Gives the active tab the earlier entries held for its repository. Run when a tab becomes
/// active and when a repository finishes opening in it (its path may only now be known).
pub(super) fn claim(st: &mut AppState) {
    if st.logs.unclaimed.is_empty() {
        return;
    }
    let Some(tab) = st.tabs.get(st.active_tab) else { return };
    let (mine, rest): (Vec<_>, Vec<_>) =
        std::mem::take(&mut st.logs.unclaimed).into_iter().partition(|e| e.repo.as_deref().is_some_and(|r| tab.is(r)));
    st.logs.unclaimed = rest;
    prepend(&mut st.log, mine);
}

/// Adds the lines of a tab that was discarded in favour of the active one after its own, so an
/// error is never lost.
pub(super) fn carry(st: &mut AppState, mut lines: Vec<LogEntry>) {
    st.log.append(&mut lines);
    keep_newest(&mut st.log);
}

/// Puts `earlier` in front of `log`, as many of the newest as fit under `LOG_LIMIT`: this
/// session's lines are always kept.
fn prepend(log: &mut Vec<LogEntry>, earlier: Vec<ActivityEntry>) {
    if earlier.is_empty() {
        return;
    }
    let room = LOG_LIMIT.saturating_sub(log.len());
    let skip = earlier.len().saturating_sub(room);
    let mut all: Vec<LogEntry> = earlier.into_iter().skip(skip).map(LogEntry::earlier).collect();
    all.append(log);
    *log = all;
}

/// A failure toast's Details: selects `tab` if it is still open and not active, then opens the
/// panel on the activity log.
pub(super) fn details(st: &mut AppState, tab: TabId, fx: &mut Vec<Effect>) {
    if let Some(i) = st.tabs.iter().position(|t| t.id == tab)
        && i != st.active_tab
    {
        super::tabs::handle(st, TabsMsg::SelectTab(i), fx);
    }
    super::logs::show(st, LogKind::Activity, fx);
}

/// Announces the first failure logged loudly in this `update`: a failure toast pushed after
/// `since` gets a Details button (beside its fix, when it has one), else a new failure toast with Details shows the line's
/// headline. Nothing new is shown while the panel already shows that tab's activity log. Run at
/// the end of `update`, which has the time.
pub(super) fn announce(st: &mut AppState, since: u64, now: Instant) {
    let Some(a) = st.logs.announce.take() else { return };
    let msg = match a.tab {
        Some(tab) => LogsMsg::Details(tab),
        None => LogsMsg::Show(LogKind::Activity),
    };
    let label = st.settings.lang().tr("logs.toast.details.button").to_owned();
    // Runs from any tab: Details switches to the line's tab itself.
    let action = ToastAction { label, msg: Box::new(Msg::Logs(msg)), workdir: None };
    if let Some(t) = st.toasts.iter_mut().rev().find(|t| t.id > since && !t.ok) {
        if t.action.is_none() {
            t.action = Some(action);
        } else if t.details.is_none() {
            t.details = Some(action);
        }
        return;
    }
    let active = st.tabs.get(st.active_tab).map(|t| t.id);
    if st.view.show_log && st.logs.tab == LogKind::Activity && a.tab == active {
        return;
    }
    st.toast_action(false, a.text, action, now);
}

#[cfg(test)]
#[path = "logs_tabs_tests.rs"]
pub(super) mod tests;
