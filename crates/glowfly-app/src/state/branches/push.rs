//! Force push with lease of any local branch and Push to all remotes (update/branch_push.rs, from `state/branches.rs`):
//! which branches may be force pushed (`force_blocked`: only one with an upstream on a remote,
//! never the remote's default branch or a Git Flow base, as Rename refuses on the remote),
//! the confirmation naming the commits the remote would lose (`force_confirm`), the local
//! branch a rejected push was for (`rejected_local`), and Push to all remotes' headline.

use crate::msg::{BranchMsg, Msg};
use crate::state::{branch_sync, branches, remote_branch};
use git2::Oid;
use glowfly_core::gitflow::FlowBase;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::{BranchInfo, Snapshot};

/// Why `b` can't be force pushed, or None when it may be: it needs an upstream on a remote
/// that still exists, and that upstream must not be the remote's default branch, nor either
/// name a Git Flow base (others build on those).
pub fn force_blocked(t: Lang, snap: &Snapshot, b: &BranchInfo) -> Option<String> {
    let name = b.name.as_str();
    let Some(tr) = branches::remote_tracking(b) else {
        return Some(t.trf("branch_push.force.no_upstream", &[("branch", &name)]));
    };
    let (remote, dest) = (tr.remote.as_str(), tr.merge.as_str());
    if !snap.remotes.iter().any(|(r, _)| r == remote) {
        return Some(t.trf("branch_push.force.no_remote", &[("remote", &remote)]));
    }
    if branch_sync::is_default(snap, remote, dest) {
        return Some(t.trf("branch_push.force.default", &[("branch", &dest), ("remote", &remote)]));
    }
    for n in [name, dest] {
        match snap.gitflow.base_role(n) {
            Some(FlowBase::Production) => return Some(t.trf("branch_push.force.production", &[("branch", &n)])),
            Some(FlowBase::Development) => return Some(t.trf("branch_push.force.development", &[("branch", &n)])),
            None => {}
        }
    }
    None
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
    let then = Msg::Branch(BranchMsg::ForcePush { branch: b.name.clone(), seen });
    let title = t.trf("branch_push.force.title", &[("branch", &b.name)]);
    Ok(Msg::confirm_danger(&title, body, t.tr("toolbar.force_push.button"), then))
}

/// The local branch whose push git rejected as behind, from the rejected remote branch `dest`
/// and the job's `output`: the one branch whose upstream is `dest`, narrowed to the remote
/// whose URL the output prints when several are. None when that leaves it open.
pub fn rejected_local<'a>(snap: &'a Snapshot, dest: &str, output: &str) -> Option<&'a BranchInfo> {
    let tracking = |b: &&BranchInfo| branches::remote_tracking(b).is_some_and(|t| t.merge == dest);
    let all: Vec<&BranchInfo> = snap.locals.iter().filter(tracking).collect();
    if let [only] = all.as_slice() {
        return Some(only);
    }
    let printed = |remote: &str| {
        let info = snap.remote_info.iter().find(|r| r.name == remote);
        info.is_some_and(|r| r.urls.iter().chain(&r.push_urls).any(|u| !u.is_empty() && output.contains(u.trim_end_matches('/'))))
    };
    let on: Vec<&BranchInfo> = all.into_iter().filter(|b| branches::remote_tracking(b).is_some_and(|t| printed(&t.remote))).collect();
    if let [only] = on.as_slice() { Some(only) } else { None }
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
