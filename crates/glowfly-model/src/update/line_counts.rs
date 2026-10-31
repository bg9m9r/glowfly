//! Line counts: the graph's per-commit totals, per-file counts of the selected commit and of
//! the working tree.

use crate::effect::{Effect, LineCountsEffect};
use crate::state::{AppState, Selection};
use git2::Oid;
use glowfly_core::repo::{FileLineCounts, WipLineCounts};
use std::sync::Arc;

/// The graph shows these commits and needs their totals (`RepoMsg::WantStats`).
pub(super) fn want_totals(st: &mut AppState, oids: Vec<Oid>, fx: &mut Vec<Effect>) {
    let Some(workdir) = st.workdir() else { return };
    let missing: Vec<Oid> =
        oids.into_iter().filter(|o| !st.session.reads.stats.contains_key(o) && !st.session.reads.stats_pending.contains(o)).collect();
    if !missing.is_empty() {
        st.session.reads.stats_pending.extend(missing.iter().copied());
        fx.push(Effect::Stats { workdir, oids: missing });
    }
}

/// Totals asked for but not worked out (`RepoMsg::StatsDropped`): they can be asked again.
pub(super) fn totals_dropped(st: &mut AppState, oids: Vec<Oid>) {
    for oid in oids {
        st.session.reads.stats_pending.remove(&oid);
    }
}

pub(super) fn totals_loaded(st: &mut AppState, batch: Vec<(Oid, usize, usize, usize)>) {
    for (oid, files, adds, dels) in batch {
        st.session.reads.stats_pending.remove(&oid);
        st.session.reads.stats.insert(oid, (files, adds, dels));
    }
}

/// Asks for commit `oid`'s per-file counts unless they are known or on their way.
pub(super) fn want_commit(st: &mut AppState, oid: Oid, fx: &mut Vec<Effect>) {
    if let Some(workdir) = st.workdir()
        && st.session.reads.line_counts.wants_commit(oid)
    {
        st.session.reads.line_counts.start_commit(oid);
        fx.push(Effect::LineCounts(LineCountsEffect::Commit { workdir, oid }));
    }
}

pub(super) fn commit_loaded(st: &mut AppState, oid: Oid, result: Result<Arc<FileLineCounts>, String>) {
    match result {
        Ok(counts) => st.session.reads.line_counts.finish_commit(oid, counts),
        Err(e) => {
            // No toast: the rows simply show no counts.
            st.session.reads.line_counts.commit_failed(oid);
            tracing::debug!("line counts of {oid}: {e}");
        }
    }
}

/// The working tree's status was read: count its files again (none to count when it is clean).
pub fn status_changed(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(snap) = st.snap.as_ref() else { return };
    if !snap.is_dirty() {
        st.session.reads.line_counts.clear_wip();
        return;
    }
    let workdir = snap.workdir.clone();
    if let Some(generation) = st.session.reads.line_counts.start_wip() {
        fx.push(Effect::LineCounts(LineCountsEffect::Wip { workdir, generation }));
    }
}

pub(super) fn wip_loaded(st: &mut AppState, generation: u64, result: Result<Box<WipLineCounts>, String>, fx: &mut Vec<Effect>) {
    if let Err(e) = &result {
        tracing::debug!("line counts of the working tree: {e}");
    }
    if st.session.reads.line_counts.finish_wip(generation, result.ok().map(|b| *b)) {
        status_changed(st, fx);
    }
}

/// On return to a tab: the selected commit's counts were dropped in flight. (The working tree's
/// follow the status read the activation sends.)
pub(super) fn reload_pending(st: &mut AppState, fx: &mut Vec<Effect>) {
    if let Selection::Commit(oid) = st.session.selection {
        want_commit(st, oid, fx);
    }
}
