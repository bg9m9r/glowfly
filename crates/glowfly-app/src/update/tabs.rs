//! Tabs: opening repositories side by side, switching, reordering and restoring the previous
//! session. Switching parks the active tab's data (its per-tab state whole, as `RepoSession`) and
//! restores the other's, and swaps their activity logs (`Tab::log`, outside `Parked`); reordering
//! moves parked data and logs with its tab. Where a tab opens and how a drop or a keyboard move moves it follow
//! the worktree groups (`tab_groups.rs`). Closing and reopening are in `tabs_close.rs`, labels in
//! `tab_label.rs`.

use crate::effect::{Effect, LoadKind, UiEffect};
use crate::msg::{CloseRequest, TabsMsg};
use crate::state::tabs::Parked;
use crate::state::{AppState, Selection, Tab};
use glowfly_core::repo::Snapshot;
use glowfly_core::settings::DiffMode;
use std::path::PathBuf;

pub(super) fn handle(st: &mut AppState, msg: TabsMsg, fx: &mut Vec<Effect>) {
    if matches!(msg, TabsMsg::NewTab | TabsMsg::SelectTab(_) | TabsMsg::CycleTab(_) | TabsMsg::ReopenClosedTab | TabsMsg::ReopenClosed(_)) {
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
        TabsMsg::CloseTab(i) => super::tabs_close::asked(st, i, CloseRequest::One, fx),
        TabsMsg::CloseOtherTabs(i) => super::tabs_close::asked(st, i, CloseRequest::Others, fx),
        TabsMsg::CloseTabsToRight(i) => super::tabs_close::asked(st, i, CloseRequest::Right, fx),
        TabsMsg::CloseConfirmed(req) => super::tabs_close::request(st, req, true, fx),
        TabsMsg::ReopenClosedTab => super::tabs_close::reopen_last(st, fx),
        TabsMsg::ReopenClosed(path) => super::tabs_close::reopen(st, path, fx),
        TabsMsg::MoveTab { from, to } => super::tab_groups::moved(st, from, to, fx),
        TabsMsg::MoveActiveTab(d) => {
            let to = st.active_tab as isize + d.signum();
            if (0..st.tabs.len() as isize).contains(&to) && d != 0 {
                super::tab_groups::moved(st, st.active_tab, to as usize, fx);
            }
        }
        TabsMsg::RenameTabDialog(i) => super::tab_label::dialog(st, if i == usize::MAX { st.active_tab } else { i }),
        TabsMsg::SetTabLabel { tab, label } => super::tab_label::set(st, tab, label, fx),
        TabsMsg::RestoreTabs { paths, labels, active } => {
            for (k, path) in paths.into_iter().enumerate() {
                if !st.tabs.iter().any(|t| t.is(&path)) {
                    let label = labels.get(k).cloned().flatten();
                    st.tabs.push(Tab { label, ..Tab::unloaded(path) });
                }
            }
            // Drop the empty tab the app starts with; its lines (settings or theme problems)
            // go to the restored tab.
            let mut carried = None;
            if st.tabs.len() > 1 && st.snap.is_none() && st.tabs[st.active_tab].path.is_none() {
                carried = Some(std::mem::take(&mut st.log));
                st.tabs.remove(st.active_tab);
                st.active_tab = usize::MAX; // nothing to park
            }
            let target = active.min(st.tabs.len() - 1);
            if st.active_tab == usize::MAX {
                activate(st, target, fx);
                super::logs_tabs::carry(st, carried.unwrap_or_default());
            } else {
                switch(st, target, fx);
            }
            super::tab_groups::restored(st, fx);
        }
        TabsMsg::GroupsRead(read) => super::tab_groups::read(st, read, fx),
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

/// Open all favorites: adds an unloaded tab for each of `paths` not shown by a tab and not
/// missing, in order (a worktree next to its group when that is known), and switches to the
/// first new one; the others load when selected. With nothing new, switches to the tab of the
/// first.
pub(super) fn open_all(st: &mut AppState, paths: Vec<PathBuf>, fx: &mut Vec<Effect>) {
    super::dashboard::leave(st);
    let mut first: Option<usize> = None;
    for p in &paths {
        if st.tab_showing(p).is_some() || crate::state::repo_lists::is_missing(st, p) {
            continue;
        }
        let at = super::tab_groups::slot_for(st, p);
        super::tab_groups::insert_tab(st, at, Tab::unloaded(p.clone()));
        first = Some(first.map_or(at, |f| if f >= at { f + 1 } else { f }));
    }
    match first {
        // Switching saves the session.
        Some(i) => switch(st, i, fx),
        None => {
            if let Some(i) = paths.first().and_then(|p| st.tab_showing(p)) {
                switch(st, i, fx);
            }
        }
    }
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
            // The failure stays readable in the tab the open came from.
            let carried = std::mem::take(&mut st.log);
            st.tabs.remove(failed);
            st.active_tab = usize::MAX;
            activate(st, if back > failed { back - 1 } else { back }, fx);
            super::logs_tabs::carry(st, carried);
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
    // Earlier entries held for this repository, now that its path is known.
    super::logs_tabs::claim(st);
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
    super::partial_content::leave(st, fx);
    park(st); // discarded with the tab
    let carried = std::mem::take(&mut st.log);
    st.tabs.remove(here);
    st.active_tab = usize::MAX;
    activate(st, if there > here { there - 1 } else { there }, fx);
    super::logs_tabs::carry(st, carried);
    true
}

fn switch(st: &mut AppState, i: usize, fx: &mut Vec<Effect>) {
    if i >= st.tabs.len() || i == st.active_tab {
        return;
    }
    st.tab_return = None;
    super::partial_content::leave(st, fx);
    let parked = park(st);
    st.tabs[st.active_tab].parked = Some(Box::new(parked));
    st.tabs[st.active_tab].log = std::mem::take(&mut st.log);
    activate(st, i, fx);
}

/// The window title while tab `i` is active.
pub(super) fn title(st: &AppState, i: usize) -> String {
    match st.tabs.get(i) {
        Some(tab) if tab.path.is_some() => format!("{} - Glowfly", tab.shown_name()),
        _ => "Glowfly".to_owned(),
    }
}

/// Makes tab `i` active, assuming the previous tab's data and log have been parked or
/// discarded: `log` becomes tab `i`'s.
pub(super) fn activate(st: &mut AppState, i: usize, fx: &mut Vec<Effect>) {
    st.active_tab = i;
    st.log = std::mem::take(&mut st.tabs[i].log);
    let parked = st.tabs[i].parked.take().map(|p| *p).unwrap_or_default();
    unpark(st, parked);
    // The preset or a pin may have changed while the tab was parked.
    super::branch_colour::sync(st);
    super::logs_tabs::claim(st);
    super::search_paths::resume(st, fx);
    super::search_index::resume(st, fx);
    super::watch::sync(st, fx);
    super::terminal::sync(st, fx);
    // Results still in flight for the previous tab are now stale.
    st.load_gen += 1;
    st.loading = false;
    st.refresh = Default::default();
    fx.push(Effect::Ui(UiEffect::SetTitle(title(st, i))));
    if st.snap.is_some() {
        super::repo::refresh(st, fx);
        super::conflict::reload_pending(st, fx);
        super::conflict_files::want(st, fx);
        super::history::reload_pending(st, fx);
        super::blame::reload_pending(st, fx);
        super::pull_request::reload_pending(st, fx);
        super::issue::reload_pending(st, fx);
        super::rebase::reload_pending(st, fx);
        super::journal::reload_pending(st, fx);
        super::range_diff::reload_pending(st, fx);
        super::diff_preview::reload_pending(st, fx);
        super::line_counts::reload_pending(st, fx);
        super::conflict_check::want(st, fx);
    } else if let Some(path) = st.tabs[i].path.clone() {
        super::repo::load(st, path, LoadKind::Open, fx);
    }
    fx.push(save(st));
}

/// Forgets what was read from the repository the tab showed before (commit statistics, line
/// counts, signatures, conflict checks) and the folders collapsed in its file lists, when the
/// tab opens another repository.
pub(super) fn forget_repo_reads(st: &mut AppState) {
    st.session.reads.clear();
    st.view.file_folds.clear();
}

pub(super) fn park(st: &mut AppState) -> Parked {
    // Not kept: another tab may show the same path, and its diff reloads on return anyway.
    st.view.diff_lines.clear();
    // The parked tab's diff results are dropped, so a pending step can't land.
    st.view.diff_landing = None;
    st.view.diff_opening = None;
    st.view.stage_move_on = None;
    st.view.diff_cursor = None;
    super::stack_prs::parked(st);
    Parked {
        session: st.session.park(),
        snap: st.snap.take(),
        selection: std::mem::take(&mut st.selection),
        issue_form: super::issue_create::park(st),
        undo: std::mem::take(&mut st.undo),
        forge: st.forge.take(),
        search: std::mem::take(&mut st.view.search),
        commit_search: std::mem::take(&mut st.search),
        ref_filter: std::mem::take(&mut st.view.ref_filter),
        graph_scroll: st.view.graph_scroll.0,
        diff_top_row: st.view.diff_top_row,
        diff_layout: st.settings.diff_layout,
        diff_mode: st.settings.diff_mode,
        diff_expanded: std::mem::take(&mut st.view.diff_expanded),
        file_folds: std::mem::take(&mut st.view.file_folds),
    }
}

fn unpark(st: &mut AppState, p: Parked) {
    st.snap = p.snap;
    // Every counter moves past the ones the previous tab left (`RepoSession::resume_after`):
    // results in flight from before the park, or for another tab, are dropped, and `activate`
    // asks again for what was loading. The open diff's results were dropped while parked too, so
    // its hunks stay off until the refresh that follows the switch reloads it.
    let mut session = p.session;
    session.resume_after(&mut st.session);
    st.session = session;
    // After the snapshot and `gitflow_resume` are back.
    super::gitflow::snapshot_changed(st);
    st.selection = if st.snap.is_some() { p.selection } else { Selection::None };
    st.view.file_folds = p.file_folds;
    // Before the rows below are counted: they count these folds.
    st.view.diff_expanded = p.diff_expanded;
    let (mode, layout) = (st.settings.diff_mode, st.settings.diff_layout);
    let ex = &st.view.diff_expanded;
    let top = match st.session.diff.view.as_mut() {
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
    st.view.diff_scroll_to_row = st.session.diff.view.is_some().then_some(top);
    st.view.diff_top_row = top;
    st.view.conflict_scroll_to = st.session.conflict.view.as_ref().filter(|c| c.file.is_some()).map(|c| c.current);
    st.view.history_scroll_to = st.session.history.view.as_ref().and_then(|h| h.selected);
    super::issue_create::unpark(st, p.issue_form);
    st.undo = p.undo;
    st.forge = p.forge;
    // Replies that were in flight for this tab were dropped while it was parked.
    if let Some(f) = st.forge.as_mut() {
        f.replies_dropped();
    }
    st.view.search = p.search;
    // The matches belong to the parked snapshot, restored above.
    st.search = p.commit_search;
    st.view.ref_filter = p.ref_filter;
    st.view.scroll_offset = Some(p.graph_scroll);
    st.view.scroll_to_row = None;
    st.view.highlight = None;
    st.view.ref_rows.reset();
    st.view.hover_tip = None;
    st.view.hover_tip_next = None;
}

/// The session to restore on the next start: tabs with a repository, and the active one.
pub(super) fn save(st: &AppState) -> Effect {
    let with_path: Vec<(usize, PathBuf)> = st.tabs.iter().enumerate().filter_map(|(i, t)| t.path.clone().map(|p| (i, p))).collect();
    let active = with_path.iter().position(|(i, _)| *i >= st.active_tab).unwrap_or(0);
    let labels = with_path.iter().map(|(i, _)| st.tabs[*i].label.clone()).collect();
    Effect::SaveTabs { paths: with_path.into_iter().map(|(_, p)| p).collect(), labels, active }
}
