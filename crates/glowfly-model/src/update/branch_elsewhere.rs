//! A checkout of a local branch another worktree has: that worktree opens instead (its tab, or a
//! new one), noted in the activity log, also when it is rebasing or bisecting the branch there
//! (that tab is where it is continued). One whose folder is missing is refused (once: clicked
//! again, no second line and the toast restarts) with a toast offering Prune and check out (`BranchMsg::PruneThen`), which runs the checkout again with
//! `git worktree prune` first.

use crate::effect::Effect;
use crate::msg::{BranchMsg, Msg};
use crate::state::worktree_branches::used_elsewhere;
use crate::state::{AppState, ToastAction};
use glowfly_core::native_path::shown;
use glowfly_core::repo::BranchUse;
use std::time::Instant;

/// How a checkout was asked for: when, and whether to prune a missing worktree first.
#[derive(Clone, Copy)]
pub(super) struct Go {
    pub now: Instant,
    pub prune: bool,
}

impl Go {
    pub fn new(now: Instant) -> Self {
        Self { now, prune: false }
    }
}

/// What the checkout does after `route`.
#[derive(Debug, PartialEq)]
pub(super) enum Route {
    /// No other worktree has the branch: check it out.
    Here,
    /// A missing worktree has it and pruning was asked for: check it out after `git worktree
    /// prune`.
    PruneFirst,
    /// Handled: the worktree opened, or the refusal was shown.
    Done,
}

/// Routes a checkout of local branch `name` around another worktree having it. `again` is the
/// checkout message, for the toast's Prune and check out.
pub(super) fn route(st: &mut AppState, name: &str, go: Go, again: impl FnOnce() -> BranchMsg, fx: &mut Vec<Effect>) -> Route {
    let t = st.settings.lang();
    let Some((wt, by)) = st.snap.as_ref().and_then(|s| used_elsewhere(s, name)) else { return Route::Here };
    let (path, missing) = (wt.path.clone(), wt.missing);
    let shown_path = shown(&path);
    let args = [("branch", &name as &dyn std::fmt::Display), ("path", &shown_path)];
    if missing {
        if go.prune {
            return Route::PruneFirst;
        }
        // Clicked again: no second line, and the toast restarts rather than stacking.
        let line = t.trf("worktree_branch.missing.log", &args);
        if st.log.last().is_none_or(|l| l.text != line) {
            st.log_quiet(false, line);
        }
        let msg = Box::new(Msg::Branch(BranchMsg::PruneThen(Box::new(again()))));
        let action = ToastAction { label: t.tr("worktree_branch.missing.button").to_owned(), msg, workdir: st.workdir() };
        st.toast_action_once(true, t.trf("worktree_branch.missing.toast", &args), action, go.now);
        return Route::Done;
    }
    let note = match by {
        BranchUse::Rebasing => t.trf("worktree_branch.opened.rebasing", &args),
        BranchUse::Bisecting => t.trf("worktree_branch.opened.bisecting", &args),
        BranchUse::CheckedOut => t.trf("worktree_branch.opened.checked_out", &args),
    };
    st.log(true, note);
    super::tabs::open(st, path, fx);
    Route::Done
}

/// `cmds` with `git worktree prune` first when `route` said `PruneFirst`.
pub(super) fn with_prune(route: &Route, mut cmds: Vec<Vec<String>>) -> Vec<Vec<String>> {
    if *route == Route::PruneFirst {
        cmds.insert(0, glowfly_core::worktree_ops::prune_cmd());
    }
    cmds
}
