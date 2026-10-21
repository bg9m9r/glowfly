//! Cherry-picking and reverting commits from the graph: the order, the refusals, the mainline
//! dialog for the merge commits among them, and selecting the WIP row (with its banner) when one stops.

use crate::effect::Effect;
use crate::msg::{Msg, SelectionMsg};
pub(super) use crate::state::pick::PickOp;
use crate::state::pick::{MainlineForm, label};
use crate::state::selection::{cherry_pick_blocker, in_graph_order};
use crate::state::{AppState, Modal, Selection};
use git2::{Oid, RepositoryState};
use glowfly_core::git_version::Feature;
use glowfly_core::pick_ops::{cherry_pick_cmd, revert_cmd};

/// Why a mainline the dialog sent doesn't fit the commits.
pub const MAINLINE_MISFIT: &str = "That parent number doesn't fit every selected commit.";

/// Cherry-picks (oldest first) or reverts (newest first) `oids` in one git command. Merge
/// commits among them without a `mainline` open the dialog that picks one; with it, `-m N` is
/// passed for every commit. The job runs as Effect::Pick, which skips the commits that change
/// nothing.
pub(super) fn run(st: &mut AppState, op: PickOp, oids: Vec<Oid>, mainline: Option<u32>, fx: &mut Vec<Effect>) {
    let Some(snap) = &st.snap else { return };
    let oids = in_graph_order(oids, snap, op == PickOp::CherryPick);
    if oids.is_empty() {
        return;
    }
    let blocked = match op {
        PickOp::CherryPick => cherry_pick_blocker(snap, &oids),
        PickOp::Revert => None,
    };
    if let Some(why) = blocked {
        st.log(false, why.text(st.settings.lang()));
        return;
    }
    match (MainlineForm::new(op, &oids, snap), mainline) {
        (Some(form), None) => {
            st.modal = Some(Modal::Mainline(form));
            return;
        }
        (Some(form), Some(n)) if form.fits(n) => {}
        (_, Some(_)) => {
            st.log(false, MAINLINE_MISFIT);
            return;
        }
        (None, None) => {}
    }
    let cmd = match op {
        PickOp::CherryPick => cherry_pick_cmd(&oids, mainline),
        PickOp::Revert => revert_cmd(&oids, mainline),
    };
    let mut spec = super::ops::JobSpec::new(label(op, oids.len()), vec![cmd]);
    let can_skip = st.git.has(Feature::SequencerSkip);
    spec.pick = Some(super::jobs::PickPlan { picking: op.core(), oids: oids.clone(), can_skip });
    let retry = || match (mainline, op) {
        (Some(mainline), _) => Msg::Selection(SelectionMsg::PickWithMainline { op, oids: oids.clone(), mainline }),
        (None, PickOp::CherryPick) => Msg::Selection(SelectionMsg::CherryPick(oids.clone())),
        (None, PickOp::Revert) => Msg::Selection(SelectionMsg::Revert(oids.clone())),
    };
    super::auto_stash::start_guarded(st, spec, retry, fx);
}

/// After the active tab's snapshot state was replaced (it was `before`): when a cherry-pick or
/// revert has just started (a multi-commit one stopped on a conflict, a single pick, or one run
/// outside Glowfly), the WIP row is selected so its banner shows.
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
