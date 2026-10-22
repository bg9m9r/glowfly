//! Starting a pull request: the create dialog (source and target repositories and branches,
//! title, linked issues), pushing the branch first (to the code host's remote, or to the fork it
//! comes from), and opening the dialog once that push has landed.

use super::ops::{JobSpec, start_job};
use crate::effect::{Effect, ForgeCall};
use crate::state::pr_repos::{self, PrRepoPick};
use crate::state::push_pr::{self, HostBranch, PrStart};
use crate::state::{AppState, Modal};
use git2::Oid;
use glowfly_core::branch_ops;
use glowfly_core::forge::{NewPullRequest, Target};

/// Opens the create dialog for `name` (a local branch or a branch on a PR repository's remote),
/// with `preferred` as the target when it is one. `from` is the remote the source is on; None
/// takes the branch's push remote (`pr_repos::push_remote`), else the code host's.
pub(super) fn open_dialog(st: &mut AppState, name: String, preferred: Option<String>, from: Option<String>, fx: &mut Vec<Effect>) {
    let (Some(snap), Some(fs)) = (&st.snap, &st.forge) else { return };
    if fs.host.is_none() {
        return;
    }
    let from = from
        .or_else(|| push_pr::local_for(snap, fs, &name).map(|b| pr_repos::push_remote(b, fs)))
        .filter(|r| pr_repos::is_pr_remote(fs, r))
        .unwrap_or_else(|| fs.remote.clone());
    let local = push_pr::local_for_on(snap, &from, &name);
    let source = push_pr::pr_source_on(snap, &from, &name);
    let into = pr_repos::default_into(snap, fs, &from, preferred.as_deref());
    let targets = pr_repos::target_branches(snap, fs, &into, &from, &source);
    let target = pr_repos::default_target(snap, fs, &into, &targets, &source, &from, preferred.as_deref());
    let tip = |remote: &str, n: &str| pr_repos::remote_branches(snap, remote).iter().find(|(b, _)| b == n).map(|(_, o)| *o);
    let local_tip = |n: &str| snap.locals.iter().find(|b| b.name == n).and_then(|b| b.oid);
    // The commit the pull request ends at: the local tip, else the remote branch's.
    let end: Option<Oid> = local.and_then(|b| b.oid).or_else(|| tip(&from, &source));
    let title = end.and_then(|o| snap.row_of.get(&o)).map(|&r| snap.commits[r].summary.clone()).unwrap_or_else(|| source.clone());
    // Issues mentioned on the branch: from its name and from commits not yet on the target.
    let branch = local.map(|b| b.name.as_str()).unwrap_or(&source);
    let mut keys: Vec<String> = fs.branch_issue(branch).into_iter().collect();
    if let Some(end) = end {
        for c in snap.commits_between(end, tip(&into, &target).or_else(|| local_tip(&target))) {
            for k in fs.issue_refs(&c.summary).into_iter().chain(fs.issue_refs(&c.body)) {
                if !keys.contains(&k) {
                    keys.push(k);
                }
            }
        }
    }
    let items = keys.join(", ");
    let repos = PrRepoPick { branch: name, from, into };
    st.modal = Some(Modal::CreatePr { source, target, targets, title, description: String::new(), draft: false, items, repos });
    super::forge::want_issues(st, keys, fx);
}

/// Pushes local `branch` to `remote` (None: its push remote), then opens the create dialog (at
/// once when the remote already has everything). A diverged branch is refused, never forced.
pub(super) fn start(st: &mut AppState, branch: &str, target: Option<String>, remote: Option<String>, fx: &mut Vec<Effect>) {
    let (Some(snap), Some(fs)) = (&st.snap, &st.forge) else { return };
    if fs.host.is_none() {
        return;
    }
    let Some(b) = snap.locals.iter().find(|b| b.name == branch).cloned() else {
        st.log(false, format!("Branch {branch} no longer exists."));
        return;
    };
    let remote = remote.unwrap_or_else(|| pr_repos::push_remote(&b, fs));
    let Some(state) = push_pr::host_branch_on(&b, snap, fs, &remote) else {
        st.log(false, format!("There is no remote named {remote}."));
        return;
    };
    match state {
        HostBranch::Blocked(why) => st.log(false, why),
        // A dialog already open for this branch and repository keeps what was typed in it.
        HostBranch::Pushed => {
            let source = push_pr::pr_source_on(snap, &remote, &b.name);
            let open =
                matches!(&st.modal, Some(Modal::CreatePr { source: s, repos, .. }) if *s == source && repos.remote_from(fs) == remote);
            if !open {
                open_dialog(st, b.name, target, Some(remote), fx);
            }
        }
        HostBranch::Missing | HostBranch::Ahead(_) => {
            let Some((to, dest, set_upstream)) = push_pr::pr_push(&b, snap, fs, &remote) else { return };
            let cmd = branch_ops::push_cmd(&b.name, &to, &dest, set_upstream);
            let then = PrStart { source: dest, target, remote: (to != fs.remote).then_some(to) };
            start_job(st, JobSpec { then_pr: Some(then), ..JobSpec::new("Push".into(), vec![cmd]) }, fx);
        }
    }
}

/// Sends a filled-in create dialog to the Into repository (`into`, a remote; "" is the code
/// host's), unless its source isn't on the From remote (the service would refuse it). A source
/// on another repository goes as `source_repo`.
pub(super) fn create(st: &mut AppState, mut pr: NewPullRequest, from: &str, into: &str, fx: &mut Vec<Effect>) {
    let Some(fs) = st.forge.as_ref() else { return };
    let Some(host) = fs.host.clone() else { return };
    let resolve = |r: &str| -> Option<(String, Target)> {
        if r.is_empty() || r == fs.remote {
            return Some((fs.remote.clone(), host.clone()));
        }
        pr_repos::repo(fs, r).map(|p| (p.remote.clone(), p.target.clone()))
    };
    let (Some((from, from_t)), Some((_, into_t))) = (resolve(from), resolve(into)) else {
        let gone = if resolve(from).is_none() { from } else { into };
        let msg = format!("{gone} is no longer a remote on {}. Pick another repository.", host.provider.name());
        st.log(false, msg);
        super::stack_prs::refused(st, &pr.source);
        return;
    };
    let missing = st
        .snap
        .as_ref()
        .and_then(|s| s.remotes.iter().find(|(r, _)| *r == from))
        .is_some_and(|(_, bs)| !bs.iter().any(|(n, _)| *n == pr.source));
    if missing {
        let msg = format!("{} isn't on {from} yet. Push it first, then create the {}.", pr.source, fs.pr_noun());
        st.log(false, msg);
        super::stack_prs::refused(st, &pr.source);
        return;
    }
    pr.source_repo = (from_t != into_t).then_some(from_t);
    super::stack_prs::sending(st, &pr.source);
    super::forge::request(st, into_t, false, ForgeCall::Create(pr), fx);
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
    let from = p.remote.as_deref().unwrap_or(&fs.remote);
    match &st.modal {
        Some(Modal::CreatePr { source, repos, .. }) if *source == p.source && repos.remote_from(fs) == from => {}
        Some(_) => st.log(true, format!("Pushed {}. Use Create {noun}… to continue.", p.source)),
        None => open_dialog(st, p.source, p.target, p.remote, fx),
    }
}

/// The refresh after the push failed: the dialog opens from the snapshot already loaded.
pub(super) fn load_failed(st: &mut AppState, fx: &mut Vec<Effect>) {
    loaded(st, fx);
}
