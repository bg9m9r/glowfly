//! Force push with lease of any local branch and Push to all remotes (update/branch_push.rs):
//! which branches a row or toast may force push (`force_blocked`: only one with an upstream on
//! a remote; never the remote's default branch or a Git Flow base, as Rename refuses on the
//! remote), the confirmation naming the commits the remote would lose (`force_confirm`), the
//! toolbar's confirmation, which warns plainly before overwriting a default or base branch
//! (`head_force_confirm`), and Push to all remotes' headline.

use crate::msg::{BranchMsg, JobMsg, Msg};
use crate::state::{branch_sync, branches, remote_branch};
use git2::Oid;
use glowfly_core::gitflow::FlowBase;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::{BranchInfo, Snapshot};

/// Why `b` can't be force pushed, or None when it may be: no other worktree may be rebasing or
/// bisecting it, it needs an upstream on a remote that still exists, and that upstream must not
/// be the remote's default branch, nor either name a Git Flow base (others build on those).
pub fn force_blocked(t: Lang, snap: &Snapshot, b: &BranchInfo) -> Option<String> {
    let name = b.name.as_str();
    // Its tip moves when that rebase ends: the lease would hold for the wrong commits.
    if let Some(why) = crate::state::worktree_branches::held_blocked(t, snap, name) {
        return Some(why);
    }
    let Some(tr) = branches::remote_tracking(b) else {
        return Some(t.trf("branch_push.force.no_upstream", &[("branch", &name)]));
    };
    let (remote, dest) = (tr.remote.as_str(), tr.merge.as_str());
    if !snap.remotes.iter().any(|(r, _)| r == remote) {
        return Some(t.trf("branch_push.force.no_remote", &[("remote", &remote)]));
    }
    base_refusal(t, snap, name, remote, dest)
}

/// A branch others build on: the remote's default branch, or a Git Flow base (by its name).
enum Base<'a> {
    Default,
    Production(&'a str),
    Development(&'a str),
}

/// Whether force pushing local `name` to `dest` on `remote` overwrites a branch others build
/// on: the remote's default branch, or a Git Flow base by either name.
fn base<'a>(snap: &Snapshot, name: &'a str, remote: &str, dest: &'a str) -> Option<Base<'a>> {
    if branch_sync::is_default(snap, remote, dest) {
        return Some(Base::Default);
    }
    [name, dest].into_iter().find_map(|n| match snap.gitflow.base_role(n)? {
        FlowBase::Production => Some(Base::Production(n)),
        FlowBase::Development => Some(Base::Development(n)),
    })
}

/// Why a row or toast may not force push local `name` to `dest` on `remote`.
fn base_refusal(t: Lang, snap: &Snapshot, name: &str, remote: &str, dest: &str) -> Option<String> {
    Some(match base(snap, name, remote, dest)? {
        Base::Default => t.trf("branch_push.force.default", &[("branch", &dest), ("remote", &remote)]),
        Base::Production(n) => t.trf("branch_push.force.production", &[("branch", &n)]),
        Base::Development(n) => t.trf("branch_push.force.development", &[("branch", &n)]),
    })
}

/// The toolbar's Force push confirmation for the checked-out branch. Force pushing the remote's
/// default branch or a Git Flow base stays possible there (a repository of one may need it), so
/// for those the body says plainly which branch it is, what it overwrites and how many commits
/// that upstream has that the branch lacks.
pub fn head_force_confirm(t: Lang, snap: &Snapshot) -> Msg {
    let plain = t.tr("toolbar.force_push.body");
    let body = match snap.head_info().and_then(|b| head_force_warning(t, snap, b)) {
        Some(warning) => format!("{plain} {warning}"),
        None => plain.to_owned(),
    };
    let (title, button) = (t.tr("toolbar.force_push.title"), t.tr("toolbar.force_push.button"));
    Msg::confirm_danger(title, body, button, Msg::Job(JobMsg::Push { force: true }))
}

/// The toolbar confirmation's warning for `b` when its push target is a branch others build on.
fn head_force_warning(t: Lang, snap: &Snapshot, b: &BranchInfo) -> Option<String> {
    let (remote, dest, _) = branches::push_target(b, snap, None)?;
    let upstream = format!("{remote}/{dest}");
    let kind = base(snap, &b.name, &remote, &dest)?;
    let base_name = match kind {
        Base::Default => dest.as_str(),
        Base::Production(n) | Base::Development(n) => n,
    };
    let args: [(&str, &dyn std::fmt::Display); 5] =
        [("branch", &b.name), ("base", &base_name), ("remote", &remote), ("upstream", &upstream), ("dest", &dest)];
    let mut warning = match kind {
        Base::Default => t.trf("branch_push.force.warn.default", &args),
        Base::Production(_) => t.trf("branch_push.force.warn.production", &args),
        Base::Development(_) => t.trf("branch_push.force.warn.development", &args),
    };
    let tracks = branches::remote_tracking(b).is_some_and(|tr| tr.remote == remote && tr.merge == dest);
    if tracks && b.behind > 0 {
        warning = format!("{warning} {}", t.trn("branch_push.force.body", b.behind, &args));
    }
    Some(warning)
}

/// The upstream's tip as the snapshot last saw it (inner None: not on the remote), the lease's
/// expected value; None when `b` has no upstream on a remote.
pub fn seen_tip(snap: &Snapshot, b: &BranchInfo) -> Option<Option<Oid>> {
    let tr = branches::remote_tracking(b)?;
    Some(remote_branch::exists(snap, &tr.remote, &tr.merge))
}

/// The danger confirmation for force pushing `b`, naming how many commits its upstream has
/// that `b` lacks (from the remote-tracking ref), or why it is refused.
pub fn force_confirm(t: Lang, snap: &Snapshot, b: &BranchInfo) -> Result<Msg, String> {
    if let Some(why) = force_blocked(t, snap, b) {
        return Err(why);
    }
    let (Some(tr), Some(seen)) = (branches::remote_tracking(b), seen_tip(snap, b)) else {
        return Err(t.trf("branch_push.force.no_upstream", &[("branch", &b.name)]));
    };
    let upstream = tr.display();
    let args: [(&str, &dyn std::fmt::Display); 3] = [("branch", &b.name), ("upstream", &upstream), ("remote", &tr.remote)];
    let lost = match (seen, b.behind) {
        (None, _) => t.trf("branch_push.force.body.gone", &args),
        (Some(_), 0) => t.trf("branch_push.force.body.none", &args),
        (Some(_), n) => t.trn("branch_push.force.body", n, &args),
    };
    let body = format!("{lost} {}", t.trf("branch_push.force.body.lease", &args));
    let (branch, remote, dest) = (b.name.clone(), tr.remote.clone(), tr.merge.clone());
    let then = Msg::Branch(BranchMsg::ForcePush { branch, remote, dest, seen });
    let title = t.trf("branch_push.force.title", &[("branch", &b.name)]);
    Ok(Msg::confirm_danger(&title, body, t.tr("toolbar.force_push.button"), then))
}

/// Push to all remotes' headline: every remote took it, or which ones didn't.
pub fn pushed_all_headline(t: Lang, branch: &str, total: usize, failed: &[String]) -> String {
    if failed.is_empty() {
        return t.trn("branch_push.all.done", total, &[("branch", &branch)]);
    }
    let names = failed.join(", ");
    let args: [(&str, &dyn std::fmt::Display); 3] = [("branch", &branch), ("remotes", &names), ("total", &total)];
    t.trn("branch_push.all.failed", failed.len(), &args)
}
