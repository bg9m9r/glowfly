//! Cherry-picking and reverting commits from the graph: the order, the refusals, the mainline
//! dialog for one merge commit, and selecting the WIP row (with its banner) when one stops.

use crate::effect::Effect;
use crate::msg::{Msg, SelectionMsg};
pub(super) use crate::state::pick::PickOp;
use crate::state::pick::{MainlineForm, label};
use crate::state::selection::{cherry_pick_blocker, in_graph_order, revert_blocker};
use crate::state::{AppState, Modal, Selection};
use git2::{Oid, RepositoryState};
use trellis_core::pick_ops::{cherry_pick_cmd, revert_cmd};

/// Why a mainline the dialog sent doesn't fit the commit.
pub const NOT_A_PARENT: &str = "That parent isn't one of the merge's parents.";

/// Cherry-picks (oldest first) or reverts (newest first) `oids` in one git command. One merge
/// commit without a `mainline` opens the dialog that picks one; with it, `-m N` is passed.
pub(super) fn run(st: &mut AppState, op: PickOp, oids: Vec<Oid>, mainline: Option<u32>, fx: &mut Vec<Effect>) {
    let Some(snap) = &st.snap else { return };
    let oids = in_graph_order(oids, snap, op == PickOp::CherryPick);
    if oids.is_empty() {
        return;
    }
    let blocked = match op {
        PickOp::CherryPick => cherry_pick_blocker(snap, &oids),
        PickOp::Revert => revert_blocker(snap, &oids),
    };
    if let Some(why) = blocked {
        st.log(false, why.text(st.settings.lang()));
        return;
    }
    let parents = |o: &Oid| snap.row_of.get(o).map_or(0, |&r| snap.commits[r].parents.len());
    match (mainline, oids.as_slice()) {
        (None, [one]) if parents(one) > 1 => {
            st.modal = MainlineForm::new(op, *one, snap).map(Modal::Mainline);
            return;
        }
        (Some(n), [one]) if parents(one) > 1 && n >= 1 && n as usize <= parents(one) => {}
        (Some(_), _) => {
            st.log(false, NOT_A_PARENT);
            return;
        }
        (None, _) => {}
    }
    let cmd = match op {
        PickOp::CherryPick => cherry_pick_cmd(&oids, mainline),
        PickOp::Revert => revert_cmd(&oids, mainline),
    };
    let spec = super::ops::JobSpec::new(label(op, oids.len()), vec![cmd]);
    let retry = || match (mainline, op) {
        (Some(mainline), _) => Msg::Selection(SelectionMsg::PickMerge { op, oid: oids[0], mainline }),
        (None, PickOp::CherryPick) => Msg::Selection(SelectionMsg::CherryPick(oids.clone())),
        (None, PickOp::Revert) => Msg::Selection(SelectionMsg::Revert(oids.clone())),
    };
    super::auto_stash::start_guarded(st, spec, retry, fx);
}

/// After the active tab's snapshot state was replaced (it was `before`): when a cherry-pick or
/// revert has just started (a multi-commit one stopped on a conflict, a single pick, or one run
/// outside Trellis), the WIP row is selected so its banner shows.
pub(super) fn state_changed(st: &mut AppState, before: Option<RepositoryState>) {
    use RepositoryState as R;
    let Some(now) = st.snap.as_ref().map(|s| s.state) else { return };
    let started = matches!(now, R::CherryPick | R::CherryPickSequence | R::Revert | R::RevertSequence);
    if before != Some(R::Clean) || !started {
        return;
    }
    st.selection = Selection::Wip;
    st.detail = None;
    if st.diff.as_ref().is_some_and(|(t, _)| !t.follows_workdir()) {
        super::repo::close_diff(st);
    }
}
