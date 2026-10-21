//! Starting a pull request: the create dialog (source, target, title, linked issues), pushing
//! the branch to the code host first, and opening the dialog once that push has landed.

use super::ops::{JobSpec, start_job};
use crate::effect::{Effect, ForgeCall};
use crate::state::push_pr::{self, HostBranch, PrStart};
use crate::state::{AppState, Modal, branches};
use trellis_core::branch_ops;
use trellis_core::forge::NewPullRequest;

/// Opens the create dialog for `name` (a local branch or a branch on the code host's remote),
/// with `preferred` as the target when it is one.
pub(super) fn open_dialog(st: &mut AppState, name: String, preferred: Option<String>, fx: &mut Vec<Effect>) {
    let (Some(snap), Some(fs)) = (&st.snap, &st.forge) else { return };
    if fs.host.is_none() {
        return;
    }
    let local = push_pr::local_for(snap, fs, &name);
    let source = push_pr::pr_source(snap, fs, &name);
    let remote_branches: &[(String, git2::Oid)] =
        snap.remotes.iter().find(|(r, _)| *r == fs.remote).map(|(_, b)| b.as_slice()).unwrap_or_default();
    let remote_tip = |n: &str| remote_branches.iter().find(|(b, _)| b == n).map(|(_, o)| *o);
    let local_tip = |n: &str| snap.locals.iter().find(|b| b.name == n).and_then(|b| b.oid);
    let mut targets: Vec<String> = if remote_branches.is_empty() {
        snap.locals.iter().map(|b| b.name.clone()).collect()
    } else {
        remote_branches.iter().map(|(n, _)| n.clone()).collect()
    };
    targets.retain(|b| *b != source);
    targets.sort();
    let target = preferred
        .filter(|t| targets.contains(t))
        .or_else(|| fs.default_branch.clone().filter(|b| *b != source))
        .or_else(|| ["main", "master", "develop"].iter().find(|b| remote_tip(b).is_some() && **b != source).map(|b| b.to_string()))
        .unwrap_or_else(|| "main".into());
    // The commit the pull request ends at: the local tip, else the remote branch's.
    let from = local.and_then(|b| b.oid).or_else(|| remote_tip(&source));
    let title = from.and_then(|o| snap.row_of.get(&o)).map(|&r| snap.commits[r].summary.clone()).unwrap_or_else(|| source.clone());
    // Issues mentioned on the branch: from its name and from commits not yet on the target.
    let branch = local.map(|b| b.name.as_str()).unwrap_or(&source);
    let mut keys: Vec<String> = fs.branch_issue(branch).into_iter().collect();
    if let Some(from) = from {
        for c in snap.commits_between(from, remote_tip(&target).or_else(|| local_tip(&target))) {
            for k in fs.issue_refs(&c.summary).into_iter().chain(fs.issue_refs(&c.body)) {
                if !keys.contains(&k) {
                    keys.push(k);
                }
            }
        }
    }
    let items = keys.join(", ");
    st.modal = Some(Modal::CreatePr { source, target, targets, title, description: String::new(), draft: false, items });
    super::forge::want_issues(st, keys, fx);
}

/// Pushes local `branch` to the code host's remote, then opens the create dialog (at once
/// when the remote already has everything). A diverged branch is refused, never forced.
pub(super) fn start(st: &mut AppState, branch: &str, target: Option<String>, fx: &mut Vec<Effect>) {
    let (Some(snap), Some(fs)) = (&st.snap, &st.forge) else { return };
    if fs.host.is_none() {
        return;
    }
    let Some(b) = snap.locals.iter().find(|b| b.name == branch).cloned() else {
        st.log(false, format!("Branch {branch} no longer exists."));
        return;
    };
    let Some(state) = push_pr::host_branch(&b, snap, fs) else {
        let msg = format!("There is no remote named {}.", fs.remote);
        st.log(false, msg);
        return;
    };
    match state {
        HostBranch::Blocked(why) => st.log(false, why),
        // A dialog already open for this branch keeps what was typed in it.
        HostBranch::Pushed => {
            let source = push_pr::pr_source(snap, fs, &b.name);
            if !matches!(&st.modal, Some(Modal::CreatePr { source: s, .. }) if *s == source) {
                open_dialog(st, b.name, target, fx);
            }
        }
        HostBranch::Missing | HostBranch::Ahead(_) => {
            let Some((remote, dest, set_upstream)) = branches::push_target(&b, snap, Some(&fs.remote)) else { return };
            let cmd = branch_ops::push_cmd(&b.name, &remote, &dest, set_upstream);
            let spec = JobSpec { then_pr: Some(PrStart { source: dest, target }), ..JobSpec::new("Push".into(), vec![cmd]) };
            start_job(st, spec, fx);
        }
    }
}

/// Sends a filled-in create dialog to the code host, unless its source isn't on the host's
/// remote (the service would refuse it).
pub(super) fn create(st: &mut AppState, pr: NewPullRequest, fx: &mut Vec<Effect>) {
    let Some(fs) = st.forge.as_ref() else { return };
    let Some(host) = fs.host.clone() else { return };
    let missing = st
        .snap
        .as_ref()
        .and_then(|s| s.remotes.iter().find(|(r, _)| *r == fs.remote))
        .is_some_and(|(_, bs)| !bs.iter().any(|(n, _)| *n == pr.source));
    if missing {
        let msg = format!("{} isn't on {} yet. Push it first, then create the {}.", pr.source, fs.remote, fs.pr_noun());
        st.log(false, msg);
        super::stack_prs::refused(st, &pr.source);
        return;
    }
    super::stack_prs::sending(st, &pr.source);
    super::forge::request(st, host, false, ForgeCall::Create(pr), fx);
}

/// A job with a create dialog to open (`then_pr`) ended. A push that worked in the active tab
/// (`here`) opens it once the refreshed snapshot lands; in another tab it is only logged.
pub(super) fn job_finished(st: &mut AppState, then_pr: Option<PrStart>, ok: bool, here: bool) {
    let Some(p) = then_pr.filter(|_| ok) else { return };
    if !here {
        st.log(true, format!("Pushed {}. Its tab is no longer active, so the create dialog wasn't opened.", p.source));
    } else if let Some(fs) = st.forge.as_mut() {
        fs.open_after_load = Some(p);
    }
}

/// A snapshot landed: opens the create dialog a push was waiting for, or starts a stack's
/// create dialogs whose push landed (update/stack_prs.rs).
pub(super) fn loaded(st: &mut AppState, fx: &mut Vec<Effect>) {
    open_after_load(st, fx);
    super::stack_prs::loaded(st, fx);
}

/// Opens the create dialog a push was waiting for. One already open for that branch (the push
/// started from it) keeps what was typed; another dialog isn't replaced.
fn open_after_load(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(fs) = st.forge.as_mut() else { return };
    let Some(p) = fs.open_after_load.take() else { return };
    let noun = fs.pr_noun();
    match &st.modal {
        Some(Modal::CreatePr { source, .. }) if *source == p.source => {}
        Some(_) => st.log(true, format!("Pushed {}. Use Create {noun}… to continue.", p.source)),
        None => open_dialog(st, p.source, p.target, fx),
    }
}

/// The refresh after the push failed: the dialog opens from the snapshot already loaded.
pub(super) fn load_failed(st: &mut AppState, fx: &mut Vec<Effect>) {
    loaded(st, fx);
}
