//! Branch actions: why one is blocked and where a push goes. The menu shows these reasons on
//! disabled items and `update` logs the same text when it refuses a late message.

use git2::RepositoryState;
use trellis_core::branch_ops::{FastForward, fast_forward_state};
use trellis_core::repo::{BranchInfo, Snapshot, Tracking};

pub const NO_REMOTES: &str = "This repository has no remotes.";

/// The remote a branch without a remote upstream is pushed to: origin if present, else the
/// first remote.
pub fn default_remote(snap: &Snapshot) -> Option<String> {
    let names: Vec<&str> = snap.remotes.iter().map(|(r, _)| r.as_str()).collect();
    names.iter().find(|r| **r == "origin").or(names.first()).map(|r| r.to_string())
}

/// The upstream's name for messages: origin/x when its ref exists, otherwise the configured one.
pub fn upstream_label(b: &BranchInfo) -> Option<String> {
    b.upstream.clone().or_else(|| b.tracking.as_ref().map(Tracking::display))
}

/// The upstream when it is on a remote (not a local branch).
pub fn remote_tracking(b: &BranchInfo) -> Option<&Tracking> {
    b.tracking.as_ref().filter(|t| t.is_remote())
}

fn commits(n: usize) -> String {
    if n == 1 { "1 commit".to_owned() } else { format!("{n} commits") }
}

/// Why `b` can't be fast-forwarded to its upstream now, or None when it can.
pub fn fast_forward_blocked(b: &BranchInfo, repo_state: RepositoryState) -> Option<String> {
    let name = &b.name;
    let up = upstream_label(b).unwrap_or_default();
    match fast_forward_state(b) {
        FastForward::NoUpstream => Some(format!("{name} has no upstream. Use Set upstream… first.")),
        FastForward::UpstreamGone => Some(format!("{up} no longer exists. Fetch, or set another upstream.")),
        FastForward::UpToDate => Some(format!("{name} is already up to date with {up}.")),
        FastForward::Ahead(n) => Some(format!("{name} is {} ahead of {up}. There is nothing to fast-forward.", commits(n))),
        FastForward::Diverged { ahead, behind } => {
            Some(format!("{name} and {up} have diverged ({ahead} ahead, {behind} behind). Merge or rebase instead."))
        }
        FastForward::Ready { .. } if b.is_head && repo_state != RepositoryState::Clean => {
            Some("Finish or abort the merge/rebase in progress first.".to_owned())
        }
        FastForward::Ready { .. } => None,
    }
}

/// Why pushing `b` to its upstream (or, without one, the default remote) is refused. Only the
/// state against a remote upstream is known; pushes elsewhere are left to git.
pub fn push_blocked(b: &BranchInfo, has_remotes: bool) -> Option<String> {
    if !has_remotes {
        return Some(NO_REMOTES.to_owned());
    }
    let on_remote = b.upstream_ref.as_deref().is_some_and(|r| r.starts_with("refs/remotes/"));
    let up = upstream_label(b).unwrap_or_default();
    if on_remote && b.behind > 0 {
        return Some(format!("{} is behind {up}. Fast-forward or pull first.", b.name));
    }
    if on_remote && b.ahead == 0 {
        return Some(format!("{} is up to date with {up}.", b.name));
    }
    None
}

/// Where a push of `b` goes: (remote, remote branch, whether to add -u). With `remote: None` it
/// goes to the upstream on its remote, or to the default remote when the upstream is missing or
/// local. A chosen remote other than the upstream's gets the branch's own name. -u is only
/// added when the branch tracks nothing, so an existing upstream is never moved.
pub fn push_target(b: &BranchInfo, snap: &Snapshot, remote: Option<&str>) -> Option<(String, String, bool)> {
    let tracked = remote_tracking(b);
    let set_upstream = b.tracking.is_none();
    let remote = match remote {
        Some(r) => {
            if !snap.remotes.iter().any(|(n, _)| n == r) {
                return None;
            }
            r.to_owned()
        }
        None => match tracked {
            Some(t) => t.remote.clone(),
            None => default_remote(snap)?,
        },
    };
    match tracked {
        Some(t) if t.remote == remote => Some((remote, t.merge.clone(), false)),
        _ => Some((remote, b.name.clone(), set_upstream)),
    }
}

/// The upstream to delete along with `b`: only for a branch that isn't checked out, whose
/// upstream is on a remote and still has its remote-tracking ref.
pub fn can_delete_with_remote(b: &BranchInfo) -> Option<&Tracking> {
    let exists = b.upstream_ref.as_deref().is_some_and(|r| r.starts_with("refs/remotes/"));
    remote_tracking(b).filter(|_| !b.is_head && exists)
}

/// The confirmation text for deleting `b` and its upstream `t`.
pub fn delete_with_remote_body(b: &BranchInfo, t: &Tracking) -> String {
    let (name, remote, merge) = (&b.name, &t.remote, &t.merge);
    let mut body =
        format!("Delete local branch {name} and {merge} on {remote}? The remote branch is removed for everyone who uses {remote}.");
    if b.behind > 0 {
        body.push_str(&format!(
            " {remote}/{merge} has {} that {name} doesn't have. They will no longer be on any branch.",
            commits(b.behind)
        ));
    }
    body
}
