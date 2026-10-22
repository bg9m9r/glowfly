//! Starting a pull request from a local branch: whether the branch must be pushed to the code
//! host first, which other branch the user picked as the target, and the name the code host
//! knows the branch by. The menus, the toolbar, the palette and the create dialog show these,
//! and `update/push_pr.rs` checks them again when the message arrives. A branch tracking a fork
//! remote (`pr_repos`) is measured against that fork instead of the code host's remote.

use super::branches;
use super::pr_repos;
use super::{ForgeState, Selection};
use git2::Oid;
use glowfly_core::forge::PullRequest;
use glowfly_core::repo::{BranchInfo, Snapshot};

/// A create dialog to open once a push lands: `source` is the branch's name on the remote it
/// was pushed to, `target` the target to preselect.
#[derive(Clone, Debug, PartialEq)]
pub struct PrStart {
    pub source: String,
    pub target: Option<String>,
    /// The remote the source was pushed to; None means the code host's.
    pub remote: Option<String>,
}

/// Where a local branch stands on the code host's remote.
#[derive(Debug, PartialEq)]
pub enum HostBranch {
    /// The remote has everything the branch has.
    Pushed,
    /// The remote has no branch to open a pull request from.
    Missing,
    /// The remote has the branch, but not this many of its commits.
    Ahead(usize),
    /// A plain push would be rejected (the remote has commits the branch lacks); why.
    Blocked(String),
}

/// The code host's remote branches, when there is a code host and its remote is loaded.
fn host_branches<'a>(snap: &'a Snapshot, fs: &ForgeState) -> Option<&'a [(String, Oid)]> {
    branches_on(snap, fs, &fs.remote)
}

/// `remote`'s branches, when there is a code host and that remote is loaded.
fn branches_on<'a>(snap: &'a Snapshot, fs: &ForgeState, remote: &str) -> Option<&'a [(String, Oid)]> {
    fs.host.as_ref()?;
    snap.remotes.iter().find(|(r, _)| r == remote).map(|(_, b)| b.as_slice())
}

/// The tip of `name` on the code host's remote.
fn host_tip(host: &[(String, Oid)], name: &str) -> Option<Oid> {
    host.iter().find(|(n, _)| n == name).map(|(_, o)| *o)
}

/// Where `b` stands on the remote its pull request is pushed to (`pr_repos::push_remote`); None
/// without a code host or when that remote isn't loaded.
pub fn host_branch(b: &BranchInfo, snap: &Snapshot, fs: &ForgeState) -> Option<HostBranch> {
    host_branch_on(b, snap, fs, &pr_repos::push_remote(b, fs))
}

/// Where `b` stands on `remote`; None without a code host or when `remote` isn't loaded.
pub fn host_branch_on(b: &BranchInfo, snap: &Snapshot, fs: &ForgeState, remote: &str) -> Option<HostBranch> {
    let host = branches_on(snap, fs, remote)?;
    if branches::remote_tracking(b).is_some_and(|t| t.remote == remote) {
        let exists = b.upstream_ref.as_deref().is_some_and(|r| r.starts_with("refs/remotes/"));
        return Some(match (exists, b.ahead, b.behind) {
            (false, _, _) => HostBranch::Missing,
            (true, 0, _) => HostBranch::Pushed,
            (true, n, 0) => HostBranch::Ahead(n),
            (true, _, _) => HostBranch::Blocked(branches::push_blocked(b, true).unwrap_or_default()),
        });
    }
    // Tracking nothing, a local branch or another remote: the push goes to the same name.
    let Some(remote_tip) = host_tip(host, &b.name) else { return Some(HostBranch::Missing) };
    let Some(local) = b.oid.filter(|o| *o != remote_tip) else { return Some(HostBranch::Pushed) };
    match snap.commits_between(local, Some(remote_tip)).len() {
        0 => Some(HostBranch::Pushed),
        n => Some(HostBranch::Ahead(n)),
    }
}

/// Whether the branch has to be pushed before its pull request can be created or is complete.
pub fn needs_push(h: &Option<HostBranch>) -> bool {
    matches!(h, Some(HostBranch::Missing | HostBranch::Ahead(_)))
}

/// The target the user picked before opening `branch`'s menu (see `chosen_target_for`).
pub fn chosen_target(sel: &Selection, branch: &BranchInfo, snap: &Snapshot, fs: &ForgeState) -> Option<String> {
    let own = branches::remote_tracking(branch).filter(|t| t.remote == fs.remote).map(|t| t.merge.as_str());
    chosen_target_for(sel, &branch.name, branch.oid, own, snap, fs)
}

/// The target the user picked before opening the menu of `source` (at `source_oid`, with
/// `own_upstream` its upstream's name on the code host's remote): the selected commit is the tip
/// of another branch on that remote (the remote branch, or a local branch of the same name).
/// With several, the repository's default branch, else the first by name.
pub fn chosen_target_for(
    sel: &Selection,
    source: &str,
    source_oid: Option<Oid>,
    own_upstream: Option<&str>,
    snap: &Snapshot,
    fs: &ForgeState,
) -> Option<String> {
    let Selection::Commit(oid) = sel else { return None };
    if source_oid == Some(*oid) {
        return None;
    }
    let host = host_branches(snap, fs)?;
    let remote = host.iter().filter(|(_, o)| o == oid).map(|(n, _)| n.as_str());
    let local = snap.locals.iter().filter(|l| l.oid == Some(*oid) && host_tip(host, &l.name).is_some()).map(|l| l.name.as_str());
    let mut names: Vec<&str> = remote.chain(local).filter(|n| *n != source && Some(*n) != own_upstream).collect();
    names.sort_unstable();
    names.dedup();
    let preferred = fs.default_branch.as_deref().filter(|d| names.contains(d));
    preferred.or(names.first().copied()).map(str::to_owned)
}

/// The toolbar's and the palette's "Push <head> and create a …" item: the checked-out branch's
/// name when it can be pushed for a new pull request, else why not.
pub fn push_item(snap: &Snapshot, fs: &ForgeState) -> Result<String, String> {
    let Some(b) = snap.head_info() else { return Err("Check out a branch first.".to_owned()) };
    if let Some(pr) = open_pr(snap, fs, &b.name) {
        return Err(format!("{} already has {} {}.", b.name, fs.pr_noun(), fs.pr_label(pr.id)));
    }
    match host_branch(b, snap, fs) {
        Some(HostBranch::Missing | HostBranch::Ahead(_)) => Ok(b.name.clone()),
        Some(HostBranch::Blocked(why)) => Err(why),
        Some(HostBranch::Pushed) | None => Err(format!("{} is up to date on {}.", b.name, pr_repos::push_remote(b, fs))),
    }
}

/// The local branch a pull request from `source` comes from: the one named `source`, else the
/// one whose upstream on the code host's remote is `source`.
pub fn local_for<'a>(snap: &'a Snapshot, fs: &ForgeState, source: &str) -> Option<&'a BranchInfo> {
    local_for_on(snap, &fs.remote, source)
}

/// `local_for` with the source on `remote`.
pub fn local_for_on<'a>(snap: &'a Snapshot, remote: &str, source: &str) -> Option<&'a BranchInfo> {
    snap.locals
        .iter()
        .find(|b| b.name == source)
        .or_else(|| snap.locals.iter().find(|b| branches::remote_tracking(b).is_some_and(|t| t.remote == remote && t.merge == source)))
}

/// The push the create dialog offers for `source` on `remote` (its From repository): its local
/// branch and where that stands.
pub fn dialog_push_on<'a>(snap: &'a Snapshot, fs: &ForgeState, remote: &str, source: &str) -> Option<(&'a BranchInfo, HostBranch)> {
    let b = local_for_on(snap, remote, source)?;
    Some((b, host_branch_on(b, snap, fs, remote)?))
}

/// The name the code host knows branch `name` by: its upstream's name when it tracks a branch
/// on the code host's remote, otherwise `name` (also for a branch that is only on the remote).
pub fn pr_source(snap: &Snapshot, fs: &ForgeState, name: &str) -> String {
    pr_source_on(snap, &fs.remote, name)
}

/// `pr_source` on `remote`.
pub fn pr_source_on(snap: &Snapshot, remote: &str, name: &str) -> String {
    local_for_on(snap, remote, name)
        .and_then(branches::remote_tracking)
        .filter(|t| t.remote == remote)
        .map(|t| t.merge.clone())
        .unwrap_or_else(|| name.to_owned())
}

/// The push of local branch `b` to `remote` for a pull request: (remote, branch there, -u). A
/// branch tracking a branch on `remote` pushes there; otherwise to its own name, with -u when
/// it tracks nothing or `remote` is a fork (not the base repository), so later pushes follow.
/// None when `remote` isn't configured.
pub fn pr_push(b: &BranchInfo, snap: &Snapshot, fs: &ForgeState, remote: &str) -> Option<(String, String, bool)> {
    if !snap.remotes.iter().any(|(n, _)| n == remote) {
        return None;
    }
    match branches::remote_tracking(b) {
        Some(t) if t.remote == remote => Some((remote.to_owned(), t.merge.clone(), false)),
        _ => Some((remote.to_owned(), b.name.clone(), b.tracking.is_none() || remote != pr_repos::base_remote(fs))),
    }
}

/// The open pull request from branch `name`, found by the name the code host knows it by (a
/// local branch's upstream there may be named differently).
pub fn open_pr<'a>(snap: &Snapshot, fs: &'a ForgeState, name: &str) -> Option<&'a PullRequest> {
    fs.pr_for_branch(&pr_source(snap, fs, name))
}

/// Why the pull request items are off when no remote is on a code host, and there are no remotes.
pub const NO_REMOTE_FOR_PR: &str = "Add a remote on a code host such as GitHub or GitLab to create pull requests.";

/// Why the pull request items are off when no remote is on a code host: `remote` (the one the
/// branch tracks or is on, else the default one) isn't on one, or there are no remotes.
pub fn no_host_reason(remote: Option<&str>) -> String {
    match remote {
        Some(r) => format!("{r} isn't on a code host such as GitHub or GitLab, so it can't take pull requests."),
        None => NO_REMOTE_FOR_PR.to_owned(),
    }
}

/// The remote a pull request from local branch `b` would go through: the one it tracks, else
/// the default one.
pub fn pr_remote(b: &BranchInfo, snap: &Snapshot) -> Option<String> {
    branches::remote_tracking(b).map(|t| t.remote.clone()).or_else(|| branches::default_remote(snap))
}

/// Whether local branch `b` is remote branch `remote`/`name` (named the same, or tracking it).
pub fn is_same_branch(b: &BranchInfo, remote: &str, name: &str) -> bool {
    b.name == name || branches::remote_tracking(b).is_some_and(|t| t.remote == remote && t.merge == name)
}
