//! Checking out a pull request's source branch from the PULL REQUESTS rows
//! (`BranchMsg::CheckoutPullRequest`): its local branch, or a new one tracking the remote of the
//! repository it is on (the code host's, or a fork's remote; `ForgeState::source_remote`), through
//! the auto-stash guard as its own operation (`AutoStashFor::PullRequestCheckout`).

use super::jobs::JobSpec;
use crate::effect::Effect;
use crate::msg::{BranchMsg, Msg};
use crate::state::{AppState, branches, worktree_branches};
use glowfly_core::forge::{self, PullRequest};
use glowfly_core::repo::BranchInfo;
use glowfly_core::stash_ops::AutoStashFor;

/// Checks out the source branch of the listed pull request `id`, unless it is gone from the
/// list, already checked out, checked out in another worktree, or its name is refused.
pub(super) fn checkout(st: &mut AppState, id: u64, fx: &mut Vec<Effect>) {
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
    if let Some(b) = snap.locals.iter().find(|b| b.name == source) {
        if remote != f.remote && !is_fork_branch(st, b, pr, &remote) {
            st.log(false, t.trf("pr_checkout.name_taken", &[("branch", &source), ("remote", &remote), ("pr", &label)]));
            return;
        }
        let why = if b.is_head {
            Some(t.trf("toolbar.branch_menu.current_blocked", &[("branch", &source)]))
        } else {
            worktree_branches::checkout_blocked(t, snap, &source)
        };
        if let Some(why) = why {
            st.log(false, why);
            return;
        }
    }
    let cmd = match branches::pr_checkout(snap, &remote, &source) {
        Ok(cmd) => cmd,
        Err(why) => {
            st.log(false, why);
            return;
        }
    };
    let op = AutoStashFor::PullRequestCheckout { pr: label, branch: source };
    let retry = move || Msg::Branch(BranchMsg::CheckoutPullRequest { id });
    super::auto_stash::start_guarded_as(st, JobSpec::new("Checkout".into(), vec![cmd]), op, retry, fx);
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
