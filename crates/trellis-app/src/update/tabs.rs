//! Tabs: opening repositories side by side, switching, reordering, closing and reopening, and
//! restoring the previous session. Switching parks the active tab's data and restores the
//! other's; reordering moves parked data with its tab. Where a tab opens and how a drop moves it
//! follow the worktree groups (`tab_groups.rs`).

use crate::effect::{Effect, LoadKind, UiEffect};
use crate::msg::TabsMsg;
use crate::state::conflict::ConflictView;
use crate::state::tabs::{MAX_CLOSED_TABS, Parked};
use crate::state::{AppState, Selection, Tab};
use std::path::PathBuf;
use trellis_core::repo::Snapshot;
use trellis_core::settings::DiffMode;

pub(super) fn handle(st: &mut AppState, msg: TabsMsg, fx: &mut Vec<Effect>) {
    if matches!(msg, TabsMsg::NewTab | TabsMsg::SelectTab(_) | TabsMsg::CycleTab(_) | TabsMsg::ReopenClosedTab) {
        super::dashboard::leave(st);
    }
    match msg {
        TabsMsg::NewTab => {
            st.tabs.push(Tab::empty());
            switch(st, st.tabs.len() - 1, fx);
        }
        TabsMsg::SelectTab(i) => switch(st, i, fx),
        TabsMsg::CycleTab(step) => {
            let n = st.tabs.len() as isize;
            switch(st, (st.active_tab as isize + step).rem_euclid(n) as usize, fx);
        }
        TabsMsg::CloseTab(i) => close(st, if i == usize::MAX { st.active_tab } else { i }, fx),
        TabsMsg::ReopenClosedTab => {
            if let Some(path) = st.closed_tabs.pop() {
                open(st, path, fx);
            }
        }
        TabsMsg::MoveTab { from, to } => super::tab_groups::moved(st, from, to, fx),
        TabsMsg::CloseOtherTabs(i) if i < st.tabs.len() => close_where(st, |j| j == i, i, fx),
        TabsMsg::CloseTabsToRight(i) if i < st.tabs.len() => close_where(st, |j| j <= i, i, fx),
        TabsMsg::RestoreTabs { paths, active } => {
            for path in paths {
                if !st.tabs.iter().any(|t| t.is(&path)) {
                    st.tabs.push(Tab::unloaded(path));
                }
            }
            // Drop the empty tab the app starts with.
            if st.tabs.len() > 1 && st.snap.is_none() && st.tabs[st.active_tab].path.is_none() {
                st.tabs.remove(st.active_tab);
                st.active_tab = usize::MAX; // nothing to park
            }
            let target = active.min(st.tabs.len() - 1);
            if st.active_tab == usize::MAX {
                activate(st, target, fx);
            } else {
                switch(st, target, fx);
            }
            super::tab_groups::restored(st, fx);
        }
        TabsMsg::GroupsRead(read) => super::tab_groups::read(st, read, fx),
        // Out of range: the tab is gone.
        TabsMsg::CloseOtherTabs(_) | TabsMsg::CloseTabsToRight(_) => {}
    }
}

/// Opens `path`: selects its tab if it is already open, uses the active tab if it is empty,
/// and otherwise opens a new tab. The dashboard, if open, closes to show it.
pub(super) fn open(st: &mut AppState, path: PathBuf, fx: &mut Vec<Effect>) {
    super::dashboard::leave(st);
    if let Some(i) = st.tab_showing(&path) {
        if i == st.active_tab {
            super::repo::refresh(st, fx);
        } else {
            switch(st, i, fx);
        }
        return;
    }
    if st.snap.is_some() || st.tabs[st.active_tab].path.is_some() {
        // Next to its worktree group when that is known already (`tab_groups::slot_for`).
        let at = super::tab_groups::slot_for(st, &path);
        super::tab_groups::insert_tab(st, at, Tab::empty());
        let back = st.active_tab;
        switch(st, at, fx);
        st.tab_return = Some(back);
    }
    st.tabs[st.active_tab] = Tab::unloaded(path.clone());
    super::repo::load(st, path, LoadKind::Open, fx);
}

/// Opening the active tab's repository failed: go back to the tab it was opened from, or leave
/// an empty tab.
pub(super) fn open_failed(st: &mut AppState, fx: &mut Vec<Effect>) {
    if st.snap.is_some() {
        return;
    }
    super::deep_link_open::open_failed(st);
    match st.tab_return.take() {
        Some(back) if back < st.tabs.len() && st.tabs.len() > 1 => {
            let failed = st.active_tab;
            st.tabs.remove(failed);
            st.active_tab = usize::MAX;
            activate(st, if back > failed { back - 1 } else { back }, fx);
        }
        _ => {
            st.tabs[st.active_tab] = Tab::empty();
            fx.push(save(st));
        }
    }
}

/// A repository finished opening in the active tab.
pub(super) fn opened(st: &mut AppState) -> Effect {
    st.tab_return = None;
    if let (Some(snap), Some(tab)) = (&st.snap, st.tabs.get_mut(st.active_tab)) {
        tab.path = Some(snap.workdir.clone());
        tab.real = Some(snap.real_workdir.clone());
        tab.folder = snap.folder;
        tab.name = snap.name.clone();
    }
    super::tab_groups::loaded(st);
    save(st)
}

/// The repository `snap` just loaded for the active tab is already open in another tab, under
/// another name for the same folder (a link, a junction, a short 8.3 name, another case on a
/// volume that ignores case): closes the active tab
/// and selects that one instead. Returns whether it did.
pub(super) fn open_elsewhere(st: &mut AppState, snap: &Snapshot, fx: &mut Vec<Effect>) -> bool {
    let here = st.active_tab;
    let Some(there) = (0..st.tabs.len()).find(|&j| j != here && st.tabs[j].shows(snap)) else { return false };
    tracing::debug!(path = %snap.workdir.display(), "already open in another tab");
    st.tab_return = None;
    park(st); // discarded with the tab
    st.tabs.remove(here);
    st.active_tab = usize::MAX;
    activate(st, if there > here { there - 1 } else { there }, fx);
    true
}

fn switch(st: &mut AppState, i: usize, fx: &mut Vec<Effect>) {
    if i >= st.tabs.len() || i == st.active_tab {
        return;
    }
    st.tab_return = None;
    let parked = park(st);
    st.tabs[st.active_tab].parked = Some(Box::new(parked));
    activate(st, i, fx);
}

/// Makes tab `i` active, assuming the previous tab's data has been parked or discarded.
fn activate(st: &mut AppState, i: usize, fx: &mut Vec<Effect>) {
    st.active_tab = i;
    let parked = st.tabs[i].parked.take().map(|p| *p).unwrap_or_default();
    unpark(st, parked);
    super::search_paths::resume(st, fx);
    super::watch::sync(st, fx);
    super::terminal::sync(st, fx);
    // Results still in flight for the previous tab are now stale.
    st.load_gen += 1;
    st.loading = false;
    st.refresh = Default::default();
    let title = match &st.tabs[i].path {
        Some(_) => format!("{} - Trellis", st.tabs[i].name),
        None => "Trellis".to_owned(),
    };
    fx.push(Effect::Ui(UiEffect::SetTitle(title)));
    if st.snap.is_some() {
        super::repo::refresh(st, fx);
        super::conflict::reload_pending(st, fx);
        super::conflict_files::want(st, fx);
        super::history::reload_pending(st, fx);
        super::blame::reload_pending(st, fx);
        super::pull_request::reload_pending(st, fx);
        super::issue::reload_pending(st, fx);
        super::rebase::reload_pending(st, fx);
        super::conflict_check::want(st, fx);
    } else if let Some(path) = st.tabs[i].path.clone() {
        super::repo::load(st, path, LoadKind::Open, fx);
    }
    fx.push(save(st));
}

fn close(st: &mut AppState, i: usize, fx: &mut Vec<Effect>) {
    if i >= st.tabs.len() {
        return;
    }
    if let Some(path) = st.tabs[i].path.clone() {
        super::terminal::tab_closed(st, &path, &[i], fx);
        remember_closed(st, path);
    }
    st.tab_return = None;
    if i != st.active_tab {
        st.tabs.remove(i);
        if i < st.active_tab {
            st.active_tab -= 1;
        }
        fx.push(save(st));
        return;
    }
    park(st); // discarded with the tab
    if st.tabs.len() == 1 {
        st.tabs[0] = Tab::empty();
        activate(st, 0, fx);
    } else {
        st.tabs.remove(i);
        activate(st, i.min(st.tabs.len() - 1), fx);
    }
}

/// Puts `path` on top of the "Reopen closed tab" stack.
fn remember_closed(st: &mut AppState, path: PathBuf) {
    st.closed_tabs.retain(|p| p != &path);
    st.closed_tabs.push(path);
    if st.closed_tabs.len() > MAX_CLOSED_TABS {
        st.closed_tabs.remove(0);
    }
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
            super::terminal::tab_closed(st, &path, &closing, fx);
            remember_closed(st, path);
        }
    }
    st.tab_return = None;
    let before = |i: usize| closing.iter().filter(|&&j| j < i).count();
    let new_survivor = survivor - before(survivor);
    if closing.contains(&st.active_tab) {
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

fn park(st: &mut AppState) -> Parked {
    st.stats_pending.clear();
    // Not kept: another tab may show the same path, and its diff reloads on return anyway.
    st.view.diff_lines.clear();
    // The parked tab's diff results are dropped, so a pending step can't land.
    st.view.diff_landing = None;
    st.view.diff_cursor = None;
    Parked {
        snap: st.snap.take(),
        selection: std::mem::take(&mut st.selection),
        detail: st.detail.take(),
        diff: st.diff.take(),
        // A save in flight reports to this tab only if it is still active, so don't wait for it.
        conflict: st.conflict.take().map(|c| ConflictView { saving: false, ..c }),
        conflict_files: std::mem::take(&mut st.conflict_files),
        history: st.history.take(),
        blame: st.blame.take(),
        pull_request: st.pull_request.take(),
        issue: st.issue.take(),
        issue_form: super::issue_create::park(st),
        rebase: st.rebase.take(),
        gitflow_panel: std::mem::take(&mut st.gitflow_panel),
        draft: std::mem::take(&mut st.draft),
        undo: std::mem::take(&mut st.undo),
        forge: st.forge.take(),
        stats: std::mem::take(&mut st.stats),
        signatures: st.signatures.parked(),
        conflict_checks: st.conflict_checks.parked(),
        search: std::mem::take(&mut st.view.search),
        commit_search: std::mem::take(&mut st.search),
        ref_filter: std::mem::take(&mut st.view.ref_filter),
        graph_scroll: st.view.graph_scroll.0,
        diff_top_row: st.view.diff_top_row,
        diff_layout: st.settings.diff_layout,
        diff_mode: st.settings.diff_mode,
        diff_expanded: std::mem::take(&mut st.view.diff_expanded),
    }
}

fn unpark(st: &mut AppState, p: Parked) {
    st.snap = p.snap;
    super::gitflow::snapshot_changed(st);
    st.selection = if st.snap.is_some() { p.selection } else { Selection::None };
    st.detail = p.detail;
    st.diff = p.diff;
    // Diff results for this tab were dropped while it was parked: its hunks stay disabled
    // until the refresh that follows the switch reloads the diff.
    st.diff_gen += 1;
    // Before the rows below are counted: they count these folds.
    st.view.diff_expanded = p.diff_expanded;
    let (mode, layout) = (st.settings.diff_mode, st.settings.diff_layout);
    let ex = &st.view.diff_expanded;
    let top = match st.diff.as_mut() {
        // The view mode changed while parked: open on the hunk that was at the top.
        Some((_, d)) if p.diff_mode != mode => {
            let h = super::diff::hunk_at_top(d, p.diff_mode, p.diff_layout, p.diff_top_row, ex);
            if mode == DiffMode::Hunks {
                std::sync::Arc::make_mut(d).whole = None;
            }
            super::diff::opening_row(d, &st.settings, h, ex)
        }
        Some((_, d)) => super::diff::same_change_row(d, mode, p.diff_layout, layout, p.diff_top_row, ex),
        None => 0,
    };
    st.view.diff_scroll_to_row = st.diff.is_some().then_some(top);
    st.view.diff_top_row = top;
    st.conflict = p.conflict;
    // Summary reads for this tab were dropped while it was parked; `activate` reads again.
    st.conflict_files = p.conflict_files;
    // Conflict results for this tab were dropped while it was parked.
    st.conflict_gen += 1;
    st.view.conflict_scroll_to = st.conflict.as_ref().filter(|c| c.file.is_some()).map(|c| c.current);
    st.history = p.history;
    // History results for this tab were dropped while it was parked.
    st.history_gen += 1;
    st.view.history_scroll_to = st.history.as_ref().and_then(|h| h.selected);
    st.blame = p.blame;
    // Blame results for this tab were dropped while it was parked.
    st.blame_gen += 1;
    st.pull_request = p.pull_request;
    // File lists for this tab were dropped while it was parked.
    st.pull_request_gen += 1;
    st.issue = p.issue;
    super::issue_create::unpark(st, p.issue_form);
    // Details for this tab were dropped while it was parked.
    st.issue_gen += 1;
    st.rebase = p.rebase;
    // Rebase list results for this tab were dropped while it was parked.
    st.rebase_gen += 1;
    st.gitflow_panel = p.gitflow_panel;
    st.draft = p.draft;
    st.undo = p.undo;
    st.forge = p.forge;
    // Replies that were in flight for this tab were dropped while it was parked.
    if let Some(f) = st.forge.as_mut() {
        f.replies_dropped();
    }
    st.stats = p.stats;
    st.signatures = p.signatures;
    st.conflict_checks = p.conflict_checks;
    st.view.search = p.search;
    // The matches belong to the parked snapshot, restored above.
    st.search = p.commit_search;
    st.view.ref_filter = p.ref_filter;
    st.view.scroll_offset = Some(p.graph_scroll);
    st.view.scroll_to_row = None;
    st.view.highlight = None;
    st.view.base_set = None;
    st.view.hover_tip = None;
    st.view.hover_tip_next = None;
}

/// The session to restore on the next start: tabs with a repository, and the active one.
pub(super) fn save(st: &AppState) -> Effect {
    let with_path: Vec<(usize, PathBuf)> = st.tabs.iter().enumerate().filter_map(|(i, t)| t.path.clone().map(|p| (i, p))).collect();
    let active = with_path.iter().position(|(i, _)| *i >= st.active_tab).unwrap_or(0);
    Effect::SaveTabs { paths: with_path.into_iter().map(|(_, p)| p).collect(), active }
}
