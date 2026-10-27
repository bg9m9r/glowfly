//! Selecting several commits: Ctrl- and Shift-clicks in the graph, select all, the comparison of
//! a pair or a line of commits (and its direction), comparing a remote branch with the current
//! branch, and keeping the selection across refreshes.
//! Comparing with the working directory is in `compare_wip.rs`; cherry-pick and revert in
//! `pick.rs`.

use crate::effect::SelectionEffect;
use crate::effect::{Effect, StackRequest};
use crate::msg::SelectionMsg;
use crate::state::graph_rows::GraphRows;
use crate::state::pick::PickOp;
use crate::state::remote_branch;
use crate::state::selection::{self as sel, MultiSelection};
use crate::state::{AppState, Selection};
use glowfly_core::repo::{DiffTarget, Snapshot, StackedCommit};
use std::time::Instant;

pub(super) fn handle(st: &mut AppState, msg: SelectionMsg, now: Instant, fx: &mut Vec<Effect>) {
    let Some(snap) = &st.snap else { return };
    match msg {
        SelectionMsg::ToggleCommit(oid) => {
            let new = sel::toggled(&st.session.selection, oid, snap);
            apply(st, new, fx);
        }
        SelectionMsg::SelectCommitRange(oid) => {
            let shown = GraphRows::new(snap, &st.session.search, st.view.show_search).shown();
            let new = sel::ranged(&st.session.selection, oid, snap, shown);
            apply(st, new, fx);
        }
        SelectionMsg::SelectAll => select_all(st, fx),
        SelectionMsg::CollapseSelection => match &st.session.selection {
            Selection::Commits(m) => {
                let focus = m.focus();
                super::repo::select_commit(st, focus, false, fx);
            }
            Selection::VsWorkdir(w) => {
                let oid = w.oid;
                super::repo::select_commit(st, oid, false, fx);
            }
            Selection::None | Selection::Wip | Selection::Commit(_) => {}
        },
        SelectionMsg::SwapCompare => swap(st, fx),
        SelectionMsg::CompareLoaded { from, to, result } => match &mut st.session.selection {
            Selection::Commits(m) if m.compare_ends() == Some((from, to)) => {
                m.changes = Some(result.clone());
                if let Err(e) = result {
                    st.log(false, e);
                }
            }
            _ => tracing::debug!("dropped a stale comparison of {from} and {to}"),
        },
        SelectionMsg::ToggleWip => super::compare_wip::toggle(st, fx),
        SelectionMsg::CompareWithWorkdir(oid) => super::compare_wip::compare(st, oid, fx),
        SelectionMsg::CompareWithHead { oid, name } => compare_with_head(st, oid, &name, fx),
        SelectionMsg::WorkdirCompareLoaded { oid, generation, result } => super::compare_wip::loaded(st, oid, generation, result),
        SelectionMsg::CherryPick(oids) => super::pick::run(st, PickOp::CherryPick, oids, None, fx),
        SelectionMsg::Revert(oids) => super::pick::run(st, PickOp::Revert, oids, None, fx),
        SelectionMsg::PickWithMainline { op, oids, mainline } => super::pick::run(st, op, oids, Some(mainline), fx),
        SelectionMsg::StackedLoaded { oids, result } => stacked_loaded(st, oids, result),
        SelectionMsg::BackToFocus => super::selection_focus::back(st, now, fx),
        SelectionMsg::RetryCompare => retry_compare(st, fx),
    }
}

/// Try again after a comparison's file list failed: asks for it once more.
fn retry_compare(st: &mut AppState, fx: &mut Vec<Effect>) {
    match &mut st.session.selection {
        Selection::Commits(m) => {
            m.changes = None;
            m.stacked = None;
            request_changes(st, fx);
        }
        Selection::VsWorkdir(w) => {
            w.changes = None;
            super::compare_wip::request(st, fx);
        }
        Selection::None | Selection::Wip | Selection::Commit(_) => {}
    }
}

/// Selects every commit row the graph shows (only the matches while the search filters), with
/// the commit in focus as the anchor (else the newest).
fn select_all(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(snap) = &st.snap else { return };
    let oids: Vec<_> = match GraphRows::new(snap, &st.session.search, st.view.show_search).shown() {
        Some(rows) => rows.iter().filter_map(|&i| snap.commits.get(i)).map(|c| c.oid).collect(),
        None => snap.commits.iter().map(|c| c.oid).collect(),
    };
    let Some(&newest) = oids.first() else { return };
    let focus = super::search::selected_oid(&st.session.selection).filter(|o| oids.contains(o));
    let new = MultiSelection::build(oids, focus.unwrap_or(newest), snap);
    apply(st, new, fx);
}

/// Selects HEAD's commit and `oid` (the tip of `name`) as a pair, unless the menu's reasons
/// against it hold now.
fn compare_with_head(st: &mut AppState, oid: git2::Oid, name: &str, fx: &mut Vec<Effect>) {
    let Some(snap) = &st.snap else { return };
    if let Some(why) = remote_branch::compare_blocked(snap, oid, name) {
        st.log(false, why);
        return;
    }
    let Some(head) = snap.head_oid else { return };
    let new = MultiSelection::build([head, oid], oid, snap);
    apply(st, new, fx);
}

/// Swap direction: the selection's comparison (of commits, or of a commit and the working tree)
/// runs the other way. The list is read again, and a diff of the old direction closes.
fn swap(st: &mut AppState, fx: &mut Vec<Effect>) {
    if matches!(st.session.selection, Selection::VsWorkdir(_)) {
        return super::compare_wip::swap(st, fx);
    }
    let Selection::Commits(m) = &mut st.session.selection else { return };
    let Some(old) = m.compare_ends() else { return };
    m.swapped = !m.swapped;
    m.changes = None;
    if let Some((DiffTarget::Between(a, b, _), _)) = &st.session.diff.view
        && (*a, *b) == old
    {
        super::repo::close_diff(st);
    }
    request_changes(st, fx);
}

/// A stacked list arrived: kept while the selection still shows those commits.
fn stacked_loaded(st: &mut AppState, oids: Vec<git2::Oid>, result: Result<Vec<StackedCommit>, String>) {
    match &mut st.session.selection {
        Selection::Commits(m) if m.stack_oids() == Some(&oids[..]) => {
            if let Err(e) = &result {
                let e = e.clone();
                m.stacked = Some(result);
                st.log(false, e);
            } else {
                m.stacked = Some(result);
            }
        }
        _ => tracing::debug!("dropped a stale stacked list of {} commits", oids.len()),
    }
}

/// Makes `new` the selection, closing whatever showed the old one.
pub(super) fn apply(st: &mut AppState, new: Selection, fx: &mut Vec<Effect>) {
    if new == st.session.selection {
        return;
    }
    match new {
        Selection::Commit(o) => super::repo::select_commit(st, o, false, fx),
        Selection::Commits(_) | Selection::VsWorkdir(_) => {
            let vs_workdir = matches!(new, Selection::VsWorkdir(_));
            st.session.selection = new;
            st.session.detail = None;
            super::repo::close_diff(st);
            super::conflict::close(st);
            super::history::close(st);
            super::blame::close(st);
            super::pull_request::close(st);
            super::issue::close(st);
            if vs_workdir {
                super::compare_wip::request(st, fx);
            } else {
                request_changes(st, fx);
            }
        }
        Selection::Wip => {
            super::repo::close_diff(st);
            super::repo::select_wip(st, fx);
        }
        Selection::None => {
            st.session.selection = new;
            st.session.detail = None;
            super::repo::close_diff(st);
        }
    }
}

/// Asks for what the panel lists and hasn't received yet: the comparison's file list, or the
/// stacked list of each commit's own changes.
fn request_changes(st: &mut AppState, fx: &mut Vec<Effect>) {
    let (Selection::Commits(m), Some(workdir)) = (&st.session.selection, st.workdir()) else { return };
    if let Some(e) = changes_effect(m, workdir) {
        fx.push(e);
    }
}

fn changes_effect(m: &MultiSelection, workdir: std::path::PathBuf) -> Option<Effect> {
    match (m.compare_ends(), m.stack_oids()) {
        (Some((from, to)), _) => m.changes.is_none().then_some(Effect::Selection(SelectionEffect::Compare { workdir, from, to })),
        (None, Some(stack)) => {
            m.stacked.is_none().then(|| Effect::Selection(SelectionEffect::Stacked(StackRequest { workdir, oids: stack.to_vec() })))
        }
        (None, None) => None,
    }
}

/// After a refresh: `snap` is the new snapshot, not stored yet. Commits that are gone leave the
/// selection, and the cherry-pick check is redone (HEAD may have moved).
pub(super) fn refreshed(st: &mut AppState, snap: &Snapshot, fx: &mut Vec<Effect>) {
    let Selection::Commits(m) = &st.session.selection else { return };
    let old_ends = m.compare_ends();
    let old_stack: Vec<_> = m.stack_oids().unwrap_or_default().to_vec();
    let new = sel::refreshed(m, snap);
    // A stacked commit's diff closes when the commit leaves the list.
    let new_stack = match &new {
        Selection::Commits(n) => n.stack_oids().unwrap_or_default(),
        _ => &[],
    };
    if let Some((DiffTarget::Commit(o, _), _)) = &st.session.diff.view
        && old_stack.contains(o)
        && !new_stack.contains(o)
        && matches!(new, Selection::Commits(_))
    {
        super::repo::close_diff(st);
    }
    let new_ends = match &new {
        Selection::Commits(n) => n.compare_ends(),
        _ => None,
    };
    if new_ends != old_ends
        && let (Some((from, to)), Some((DiffTarget::Between(a, b, _), _))) = (old_ends, &st.session.diff.view)
        && (*a, *b) == (from, to)
    {
        super::repo::close_diff(st);
    }
    match new {
        Selection::Commits(_) => {
            st.session.selection = new;
            // A list dropped in flight (a tab switch) is asked for again here.
            if let Selection::Commits(m) = &st.session.selection
                && let Some(e) = changes_effect(m, snap.workdir.clone())
            {
                fx.push(e);
            }
        }
        Selection::Commit(o) => {
            st.session.selection = Selection::Commit(o);
            st.session.detail = None;
            fx.push(Effect::LoadDetail { workdir: snap.workdir.clone(), oid: o });
            super::line_counts::want_commit(st, o, fx);
        }
        Selection::None | Selection::Wip | Selection::VsWorkdir(_) => {
            st.session.selection = Selection::None;
            st.session.detail = None;
        }
    }
}
