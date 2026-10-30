//! The operation journal (`JournalMsg`): opening it in the central area, reading it and the
//! selected state's preview, and Restore through the pure plan in state/journal_restore.rs.
//! The Lost commits tab is update/journal_lost.rs.
//! Every load and preview carries a generation from `RepoSession::journal`, so a result for a
//! view that was closed, reopened or parked meanwhile is dropped.

use crate::effect::Effect;
use crate::journal::{JournalEffect, JournalMsg};
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::journal::{JournalView, PreviewState, against};
use crate::state::journal_restore;
use glowfly_core::journal::RowKey;
use std::time::Instant;

pub(super) fn handle(st: &mut AppState, msg: JournalMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        JournalMsg::Open => open(st, fx),
        JournalMsg::Close => close(st),
        JournalMsg::Toggle if st.session.journal.view.is_some() => close(st),
        JournalMsg::Toggle => open(st, fx),
        JournalMsg::Refresh => refreshed(st, fx),
        JournalMsg::Loaded { generation, result } => loaded(st, generation, result.map(|t| *t), fx),
        JournalMsg::Select(key) => select(st, key, fx),
        JournalMsg::PreviewLoaded { generation, key, result } => {
            let Some(p) = st.session.journal.view.as_mut().and_then(|v| v.preview.as_mut()) else { return };
            if p.generation == generation && p.key == key {
                p.result = Some(result);
            }
        }
        JournalMsg::Restore(key) => restore(st, &key, now),
        JournalMsg::CreateBranch(key) => {
            let (Some(view), Some(snap)) = (&st.session.journal.view, &st.snap) else { return };
            let Some(row) = view.row(&key) else { return };
            let msg = journal_restore::create_branch(row, snap);
            super::branches::handle(st, msg, now, fx);
        }
        JournalMsg::Tab(tab) => super::journal_lost::show_tab(st, tab, fx),
        JournalMsg::Lost(m) => super::journal_lost::handle(st, m, now, fx),
    }
}

/// Opens the journal in place of the graph, closing the views it replaces, and reads it.
fn open(st: &mut AppState, fx: &mut Vec<Effect>) {
    if st.snap.is_none() || st.session.rebase.view.as_ref().is_some_and(|p| p.starting) {
        return;
    }
    super::repo::close_diff(st);
    super::conflict::close(st);
    super::history::close(st);
    super::blame::close(st);
    super::rebase::close(st);
    super::pull_request::close(st);
    super::issue::close(st);
    super::range_diff::close(st);
    st.session.gitflow_panel = false;
    if st.session.journal.view.is_none() {
        st.session.journal.view = Some(JournalView::default());
    }
    load(st, fx);
}

/// Back to the graph.
pub(super) fn close(st: &mut AppState) {
    st.session.journal.view = None;
}

/// Reads the journal again when it is open (a full refresh, Refresh, Try again), and the lost
/// commits while their tab shows.
pub(super) fn refreshed(st: &mut AppState, fx: &mut Vec<Effect>) {
    if st.session.journal.view.is_some() {
        load(st, fx);
        super::journal_lost::refreshed(st, fx);
    }
}

fn load(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(workdir) = st.workdir() else { return };
    st.session.journal.generation += 1;
    let generation = st.session.journal.generation;
    let Some(view) = st.session.journal.view.as_mut() else { return };
    view.generation = generation;
    view.loading = true;
    fx.push(Effect::Journal(JournalEffect::Load { workdir, generation }));
}

fn loaded(st: &mut AppState, generation: u64, result: Result<glowfly_core::journal::Timeline, String>, fx: &mut Vec<Effect>) {
    let Some(view) = st.session.journal.view.as_mut().filter(|v| v.generation == generation) else { return };
    view.loading = false;
    view.timeline = Some(result);
    // The selection survives a reload by its key; its preview is read again against the tips now.
    match view.selected.take() {
        Some(key) if view.row(&key).is_some() => select(st, key, fx),
        _ => view.preview = None,
    }
}

/// Selects row `key` and reads its preview, unless its commit is gone.
fn select(st: &mut AppState, key: RowKey, fx: &mut Vec<Effect>) {
    let (Some(workdir), Some(snap)) = (st.workdir(), st.snap.as_ref()) else { return };
    let Some(view) = st.session.journal.view.as_ref() else { return };
    let Some(row) = view.row(&key) else { return };
    let gone = row.summary.is_none();
    let (oid, tip) = (row.oid, against(snap, &row.subject));
    let t = st.settings.lang();
    st.session.journal.generation += 1;
    let generation = st.session.journal.generation;
    let Some(view) = st.session.journal.view.as_mut() else { return };
    view.selected = Some(key.clone());
    let result = gone.then(|| Err(t.tr("journal.gone").to_owned()));
    if result.is_none() {
        fx.push(Effect::Journal(JournalEffect::Preview { workdir, key: key.clone(), oid, against: tip, generation }));
    }
    view.preview = Some(PreviewState { key, generation, result });
}

/// Restore… on row `key`: the confirmation, or why it can't (log and toast).
fn restore(st: &mut AppState, key: &RowKey, now: Instant) {
    let t = st.settings.lang();
    let (Some(view), Some(snap)) = (&st.session.journal.view, &st.snap) else { return };
    let Some(row) = view.row(key) else { return };
    match journal_restore::plan(snap, row, st.job.is_some(), t) {
        Err(why) => super::rebase::refuse(st, why, now),
        Ok(plan) => {
            let dirty = crate::state::auto_stash::blocks(snap);
            let msg = journal_restore::confirm(t, &plan, view.preview_of(key), dirty, snap.head_branch.as_deref());
            if let Msg::Confirm { title, body, button, then } = msg {
                st.modal = Some(crate::state::Modal::Confirm { title, body, button, then });
            }
        }
    }
}

/// On a tab's return: asks again for a load or preview that was in flight when it was parked.
pub(super) fn reload_pending(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(view) = &st.session.journal.view else { return };
    let (loading, pending) = (view.loading, view.preview.as_ref().filter(|p| p.result.is_none()).map(|p| p.key.clone()));
    if loading {
        load(st, fx);
    } else if let Some(key) = pending {
        select(st, key, fx);
    }
    super::journal_lost::reload_pending(st, fx);
}
