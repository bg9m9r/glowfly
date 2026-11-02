//! Checking out a pull request's source branch from the PULL REQUESTS rows
//! (`BranchMsg::CheckoutPullRequest`): its local branch, or a new one tracking the remote of the
//! repository it is on (the code host's, or a fork's remote; `ForgeState::source_remote`), through
//! the auto-stash guard as its own operation (`AutoStashFor::PullRequestCheckout`). A local
//! branch another worktree has opens that worktree instead (update/branch_elsewhere.rs).

use super::branch_elsewhere::{Go, Route, route, with_prune};
use super::jobs::JobSpec;
use crate::effect::Effect;
use crate::msg::{BranchMsg, Msg};
use crate::state::{AppState, branches};
use glowfly_core::forge::{self, PullRequest};
use glowfly_core::repo::BranchInfo;
use glowfly_core::stash_ops::AutoStashFor;

/// Checks out the source branch of the listed pull request `id`, unless it is gone from the
/// list, already checked out or its name is refused; opens the worktree that has it instead.
pub(super) fn checkout(st: &mut AppState, id: u64, go: Go, fx: &mut Vec<Effect>) {
    let t = st.settings.lang();
    let (Some(snap), Some(f)) = (st.snap.as_ref(), st.session.forge.as_ref()) else { return };
    let Some(pr) = f.prs.iter().find(|p| p.id == id) else {
        st.log(false, t.tr("pr_checkout.gone"));
        return;
    };
    let (label, source) = (f.pr_label(id), pr.source.clone());
    // The source is on the remote of its repository, never a same-named branch of another.
    let Some(remote) = f.source_remote(pr) else {
        st.log(false, t.trf("pr_checkout.fork_not_here", &[("pr", &label)]));
        return;
    };
    let mut how = Route::Here;
    if let Some(b) = snap.locals.iter().find(|b| b.name == source) {
        if remote != f.remote && !is_fork_branch(st, b, pr, &remote) {
            st.log(false, t.trf("pr_checkout.name_taken", &[("branch", &source), ("remote", &remote), ("pr", &label)]));
            return;
        }
        if b.is_head {
            st.log(false, t.trf("toolbar.branch_menu.current_blocked", &[("branch", &source)]));
            return;
        }
        how = route(st, &source, go, || BranchMsg::CheckoutPullRequest { id }, fx);
        if how == Route::Done {
            return;
        }
    }
    let Some(snap) = st.snap.as_ref() else { return };
    let cmd = match branches::pr_checkout(snap, &remote, &source) {
        Ok(cmd) => cmd,
        Err(why) => {
            st.log(false, why);
            return;
        }
    };
    let op = AutoStashFor::PullRequestCheckout { pr: label, branch: source };
    let again = move || BranchMsg::CheckoutPullRequest { id };
    let retry = move || Msg::Branch(if go.prune { BranchMsg::PruneThen(Box::new(again())) } else { again() });
    let mut spec = JobSpec::new("Checkout".into(), with_prune(&how, vec![cmd]));
    if how == Route::PruneFirst {
        // Two commands: `start_job` classifies only one, so name the kind for undo.
        spec.record = Some(glowfly_core::undo::UndoKind::Checkout);
    }
    super::auto_stash::start_guarded_as(st, spec, op, retry, fx);
}

/// Whether local branch `b` is the fork's branch of the same name: it tracks it or pushes to it
/// (`branch.<name>.pushRemote`, `remote.pushDefault`), through `remote` or another remote of the
/// fork's repository.
fn is_fork_branch(st: &AppState, b: &BranchInfo, pr: &PullRequest, remote: &str) -> bool {
    let (Some(snap), Some(host)) = (st.snap.as_ref(), st.session.forge.as_ref().and_then(|f| f.host.as_ref())) else { return false };
    let is_fork = |r: &str| {
        let url = snap.remote_urls.iter().find(|(n, _)| n == r).map(|(_, u)| u.as_str());
        r == remote || url.and_then(|u| forge::parse_remote(u, &st.settings.connections)).is_some_and(|t| pr.head_repo.is(&t, host))
    };
    let tracks = b.tracking.as_ref().is_some_and(|t| t.merge == b.name && is_fork(&t.remote));
    tracks || snap.push_remotes.for_branch(&b.name).is_some_and(|(_, r)| is_fork(r))
}
