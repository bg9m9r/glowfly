//! The journal's Lost commits tab (`JournalMsg::Tab`, `LostMsg`): switching tabs, finding the
//! lost commits only while their tab shows (a refresh while it is hidden marks the list stale
//! instead of running `git fsck`), the selected commit's history, and its actions through the
//! pure rules in state/journal_lost.rs. Loads and previews carry a generation from
//! `RepoSession::journal`, so a result for a view that was closed or parked is dropped.

use crate::effect::Effect;
use crate::journal::{JournalEffect, LostMsg};
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::journal_lost::{self as rules, JournalTab, LostPreviewState, LostView};
use git2::Oid;
use glowfly_core::journal::{LostCommit, LostKind};
use std::time::Instant;

/// Shows `tab`, finding the lost commits when their tab shows without a fresh list.
pub(super) fn show_tab(st: &mut AppState, tab: JournalTab, fx: &mut Vec<Effect>) {
    let Some(view) = st.session.journal.view.as_mut() else { return };
    view.tab = tab;
    let needs = view.lost.as_ref().is_none_or(|l| l.stale && !l.loading);
    if tab == JournalTab::Lost && needs {
        load(st, fx);
    }
}

pub(super) fn handle(st: &mut AppState, msg: LostMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        LostMsg::Loaded { generation, result } => loaded(st, generation, result.map(|l| *l), fx),
        LostMsg::Select(oid) => select(st, oid, fx),
        LostMsg::PreviewLoaded { generation, oid, result } => {
            let Some(p) = lost_mut(st).and_then(|l| l.preview.as_mut()) else { return };
            if p.generation == generation && p.oid == oid {
                p.result = Some(result);
            }
        }
        LostMsg::Restore(oid) => restore(st, oid, now),
        LostMsg::CreateBranch(oid) => {
            let Some(c) = commit(st, oid).filter(|c| c.kind == LostKind::Commit) else { return };
            super::branches::handle(st, rules::create_branch(&c), fx);
        }
        LostMsg::Apply(oid) => {
            let (Some(c), Some(snap)) = (stash(st, oid), st.snap.as_ref()) else { return };
            let msg = rules::apply(snap, &c, st.settings.lang());
            dispatch(st, msg, now, fx);
        }
        LostMsg::Store(oid) => {
            let Some(c) = stash(st, oid) else { return };
            let msg = rules::store(&c, st.settings.lang());
            dispatch(st, msg, now, fx);
        }
    }
}

fn lost_mut(st: &mut AppState) -> Option<&mut LostView> {
    st.session.journal.view.as_mut().and_then(|v| v.lost.as_mut())
}

/// The listed lost commit `oid`.
fn commit(st: &AppState, oid: Oid) -> Option<LostCommit> {
    st.session.journal.view.as_ref()?.lost.as_ref()?.commit(oid).cloned()
}

/// The listed dropped stash `oid`.
fn stash(st: &AppState, oid: Oid) -> Option<LostCommit> {
    commit(st, oid).filter(|c| matches!(c.kind, LostKind::Stash { .. }))
}

/// Runs an action's message: a confirmation opens, a job starts.
fn dispatch(st: &mut AppState, msg: Msg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        Msg::Confirm { title, body, button, then } => st.modal = Some(crate::state::Modal::Confirm { title, body, button, then }),
        Msg::Job(job) => super::ops::job(st, job, now, fx),
        // Apply on top of uncommitted changes reads the stash's files before it asks.
        Msg::Stash(m) => super::stash::handle(st, m, fx),
        // Apply refused over conflicts.
        Msg::Logged { ok, text } => st.log(ok, text),
        // The actions only ever give one of those.
        _ => {}
    }
}

/// Finds the lost commits again (`git fsck`, off the UI thread).
fn load(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(workdir) = st.workdir() else { return };
    if st.session.journal.view.is_none() {
        return;
    }
    st.session.journal.generation += 1;
    let generation = st.session.journal.generation;
    let Some(view) = st.session.journal.view.as_mut() else { return };
    let lost = view.lost.get_or_insert_with(LostView::default);
    lost.generation = generation;
    lost.loading = true;
    lost.stale = false;
    fx.push(Effect::Journal(JournalEffect::Lost { workdir, generation }));
}

fn loaded(st: &mut AppState, generation: u64, result: Result<glowfly_core::journal::LostCommits, String>, fx: &mut Vec<Effect>) {
    let Some(view) = st.session.journal.view.as_mut() else { return };
    let showing = view.tab == JournalTab::Lost;
    let Some(lost) = view.lost.as_mut().filter(|l| l.generation == generation) else { return };
    lost.loading = false;
    lost.result = Some(result);
    // Refs changed while `git fsck` ran, so this result may already be out of date: find them
    // again while the tab shows, else `stale` stays set for when it does.
    let again = lost.stale && showing;
    // The selection survives a reload while its commit is still lost; its history is read again.
    match lost.selected.take() {
        Some(oid) if lost.commit(oid).is_some() => select(st, oid, fx),
        _ => lost.preview = None,
    }
    if again {
        load(st, fx);
    }
}

/// Selects lost commit `oid` and reads its history.
fn select(st: &mut AppState, oid: Oid, fx: &mut Vec<Effect>) {
    let Some(workdir) = st.workdir() else { return };
    let Some(c) = commit(st, oid) else { return };
    st.session.journal.generation += 1;
    let generation = st.session.journal.generation;
    let Some(lost) = lost_mut(st) else { return };
    lost.selected = Some(oid);
    lost.preview = Some(LostPreviewState { oid, generation, result: None });
    fx.push(Effect::Journal(JournalEffect::LostPreview { workdir, oid, from: rules::history_from(&c), generation }));
}

/// Restore… on lost commit `oid`: the confirmation of a detached checkout, or why it can't.
fn restore(st: &mut AppState, oid: Oid, now: Instant) {
    let Some(c) = commit(st, oid).filter(|c| c.kind == LostKind::Commit) else { return };
    let Some(snap) = &st.snap else { return };
    let t = st.settings.lang();
    match rules::restore_plan(snap, &c, st.job.is_some(), t) {
        Err(why) => super::rebase::refuse(st, why, now),
        Ok(plan) => {
            let msg = rules::restore_confirm(t, &plan, snap);
            if let Msg::Confirm { title, body, button, then } = msg {
                st.modal = Some(crate::state::Modal::Confirm { title, body, button, then });
            }
        }
    }
}

/// After a refresh: finds the lost commits again while their tab shows, else marks them stale.
pub(super) fn refreshed(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(view) = st.session.journal.view.as_mut() else { return };
    let loading = view.lost.as_ref().is_some_and(|l| l.loading);
    if view.tab == JournalTab::Lost && !loading {
        load(st, fx);
    } else if let Some(l) = view.lost.as_mut() {
        l.stale = true;
    }
}

/// On a tab's return: asks again for a load or history that was in flight when it was parked.
/// A load is asked for only while the Lost tab shows; on the States tab it is marked stale.
pub(super) fn reload_pending(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(view) = st.session.journal.view.as_mut() else { return };
    let showing = view.tab == JournalTab::Lost;
    let Some(lost) = view.lost.as_mut() else { return };
    let pending = lost.preview.as_ref().filter(|p| p.result.is_none()).map(|p| p.oid);
    if lost.loading && showing {
        load(st, fx);
    } else if lost.loading {
        lost.loading = false;
        lost.stale = true;
    } else if let Some(oid) = pending {
        select(st, oid, fx);
    }
}
