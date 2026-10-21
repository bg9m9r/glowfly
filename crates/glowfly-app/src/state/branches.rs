//! Branch actions: why one is blocked and where a push goes. The menu shows these reasons on
//! disabled items and `update` logs the same text when it refuses a late message.

use git2::RepositoryState;
use glowfly_core::branch_ops::{FastForward, fast_forward_state};
use glowfly_core::gitcli::args;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::{BranchInfo, Snapshot, Tracking};

pub const NO_REMOTES: &str = "This repository has no remotes.";
/// Why the checked-out branch can't be hidden: HEAD is always walked, so only its label would go.
pub const HEAD_ALWAYS_SHOWN: &str = "The checked-out branch is always shown in the graph.";
/// Why the main branch has no Color submenu: it is always drawn in the accent.
pub const MAIN_USES_ACCENT: &str = "The main branch always uses the accent color.";

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

/// "1 commit" or "N commits".
pub(crate) fn commits(n: usize) -> String {
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

/// Why `b` can't be deleted together with its upstream `t`: the checked-out commit (`head`, the
/// branch name or None when detached) doesn't contain it, so both deletes would leave its
/// commits on no branch.
pub fn delete_with_remote_blocked(b: &BranchInfo, t: &Tracking, head: Option<&str>) -> Option<String> {
    if b.merged {
        return None;
    }
    let head = head.unwrap_or("HEAD");
    Some(format!("{} has commits that aren't merged into {head}. Deleting it and {} would leave them on no branch.", b.name, t.display()))
}

/// The confirmation text for deleting `b` and its upstream `t`.
pub fn delete_with_remote_body(lang: Lang, b: &BranchInfo, t: &Tracking) -> String {
    let (name, remote, merge) = (&b.name, &t.remote, &t.merge);
    let mut body = lang.trf("branch.delete_both.body", &[("branch", name), ("remote_branch", merge), ("remote", remote)]);
    if b.behind > 0 {
        body.push(' ');
        body.push_str(&lang.trn("branch.delete_both.behind_note", b.behind, &[("remote", remote), ("merge", merge), ("branch", name)]));
    }
    body
}

/// `git checkout` of the local branch `name`. The trailing `--` keeps a branch named like a
/// file (e.g. `src`) from being read as a path whose changes git would discard.
pub fn checkout_local(name: &str) -> Vec<String> {
    args(&["checkout", name, "--"])
}

/// How to check out a pull request's source branch `source`: the local branch of that name, or
/// a new one tracking `<remote>/<source>`. The name comes from the code host (a fork's author
/// picks it), so one git would read as an option is refused, and so is one this repository
/// doesn't have (a fork's branch that was never fetched), rather than letting git guess.
pub fn pr_checkout(snap: &Snapshot, remote: &str, source: &str) -> Result<Vec<String>, String> {
    if let Some(why) = pr_source_refused(source) {
        return Err(why);
    }
    if snap.locals.iter().any(|b| b.name == source) {
        return Ok(checkout_local(source));
    }
    let on_remote = snap.remotes.iter().any(|(r, bs)| r == remote && bs.iter().any(|(n, _)| n == source));
    if on_remote {
        return Ok(args(&["checkout", "--track", &format!("{remote}/{source}"), "--"]));
    }
    Err(pr_source_missing(source, remote))
}

/// Why a pull request's source branch name is never used: git would read it as an option.
pub(crate) fn pr_source_refused(source: &str) -> Option<String> {
    (source.is_empty() || source.starts_with('-')).then(|| format!("\"{source}\" isn't a branch name Trellis can check out."))
}

/// A pull request's source branch that is neither a local branch nor on the code host's remote.
pub(crate) fn pr_source_missing(source: &str, remote: &str) -> String {
    format!("{source} isn't a local branch or on {remote} (it may be on a fork). Fetch it first.")
}
