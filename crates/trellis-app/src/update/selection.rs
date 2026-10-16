//! Selecting several commits: Ctrl- and Shift-clicks in the graph, the two-commit comparison,
//! keeping the selection across refreshes, and cherry-picking the selected commits.

use crate::effect::Effect;
use crate::msg::SelectionMsg;
use crate::state::graph_rows::GraphRows;
use crate::state::selection as sel;
use crate::state::{AppState, Selection};
use git2::Oid;
use trellis_core::gitcli::args;
use trellis_core::repo::{DiffTarget, Snapshot};

pub(super) fn handle(st: &mut AppState, msg: SelectionMsg, fx: &mut Vec<Effect>) {
    let Some(snap) = &st.snap else { return };
    match msg {
        SelectionMsg::ToggleCommit(oid) => {
            let new = sel::toggled(&st.selection, oid, snap);
            apply(st, new, fx);
        }
        SelectionMsg::SelectCommitRange(oid) => {
            let shown = GraphRows::new(snap, &st.search, st.view.show_search).shown();
            let new = sel::ranged(&st.selection, oid, snap, shown);
            apply(st, new, fx);
        }
        SelectionMsg::CollapseSelection => {
            if let Selection::Commits(m) = &st.selection {
                let focus = m.focus();
                super::repo::select_commit(st, focus, false, fx);
            }
        }
        SelectionMsg::CherryPick(oids) => cherry_pick(st, oids, fx),
        SelectionMsg::CompareLoaded { from, to, result } => match &mut st.selection {
            Selection::Commits(m) if m.pair() == Some((from, to)) => {
                m.changes = Some(result.clone());
                if let Err(e) = result {
                    st.log(false, e);
                }
            }
            _ => tracing::debug!("dropped a stale comparison of {from} and {to}"),
        },
    }
}

/// Makes `new` the selection, closing whatever showed the old one.
fn apply(st: &mut AppState, new: Selection, fx: &mut Vec<Effect>) {
    if new == st.selection {
        return;
    }
    match new {
        Selection::Commit(o) => super::repo::select_commit(st, o, false, fx),
        Selection::Commits(_) => {
            st.selection = new;
            st.detail = None;
            super::repo::close_diff(st);
            super::conflict::close(st);
            super::history::close(st);
            super::blame::close(st);
            request_compare(st, fx);
        }
        Selection::None | Selection::Wip => {
            st.selection = new;
            st.detail = None;
            super::repo::close_diff(st);
        }
    }
}

/// With two commits selected and their file list not received yet, asks for it.
fn request_compare(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Selection::Commits(m) = &st.selection else { return };
    if let (Some((from, to)), None, Some(workdir)) = (m.pair(), &m.changes, st.workdir()) {
        fx.push(Effect::LoadCompare { workdir, from, to });
    }
}

/// After a refresh: `snap` is the new snapshot, not stored yet. Commits that are gone leave the
/// selection, and the cherry-pick check is redone (HEAD may have moved).
pub(super) fn refreshed(st: &mut AppState, snap: &Snapshot, fx: &mut Vec<Effect>) {
    let Selection::Commits(m) = &st.selection else { return };
    let old_pair = m.pair();
    let new = sel::refreshed(m, snap);
    let new_pair = match &new {
        Selection::Commits(n) => n.pair(),
        _ => None,
    };
    if new_pair != old_pair
        && let (Some((from, to)), Some((DiffTarget::Between(a, b, _), _))) = (old_pair, &st.diff)
        && (*a, *b) == (from, to)
    {
        super::repo::close_diff(st);
    }
    match new {
        Selection::Commits(_) => {
            st.selection = new;
            // A list dropped in flight (a tab switch) is asked for again here.
            if let Selection::Commits(m) = &st.selection
                && let Some((from, to)) = m.pair()
                && m.changes.is_none()
            {
                fx.push(Effect::LoadCompare { workdir: snap.workdir.clone(), from, to });
            }
        }
        Selection::Commit(o) => {
            st.selection = Selection::Commit(o);
            st.detail = None;
            fx.push(Effect::LoadDetail { workdir: snap.workdir.clone(), oid: o });
        }
        Selection::None | Selection::Wip => {
            st.selection = Selection::None;
            st.detail = None;
        }
    }
}

/// Cherry-picks `oids` onto HEAD in one git command, oldest first.
fn cherry_pick(st: &mut AppState, oids: Vec<Oid>, fx: &mut Vec<Effect>) {
    let Some(snap) = &st.snap else { return };
    let oids = sel::in_graph_order(oids, snap, true);
    if oids.is_empty() {
        return;
    }
    if let Some(why) = sel::cherry_pick_blocker(snap, &oids) {
        st.log(false, why);
        return;
    }
    let n = oids.len();
    let label = if n == 1 { "Cherry-pick".to_owned() } else { format!("Cherry-pick {n} commits") };
    let mut cmd = args(&["cherry-pick"]);
    cmd.extend(oids.iter().map(Oid::to_string));
    super::ops::start_job(st, super::ops::JobSpec::new(label, vec![cmd]), fx);
}
