//! Command lines and checks for branch operations (rename, upstream, fast-forward, push and
//! delete with the remote branch). Everything here is pure; the app runs the commands through
//! `gitcli` so hooks, credentials and the user's git config apply.

use crate::gitcli::args;
use crate::repo::{BranchInfo, Tracking};

/// Whether git accepts `name` as a branch name (no spaces, "..", "~^:?*[\", leading "-",
/// trailing "/" or ".lock", "HEAD", "@{" and so on).
pub fn is_valid_branch_name(name: &str) -> bool {
    git2::Branch::name_is_valid(name).unwrap_or(false)
}

/// Why a new branch name can't be used.
#[derive(Debug, PartialEq)]
pub enum NameProblem {
    Empty,
    Invalid,
    /// Another branch has this name, possibly with different letter case.
    Exists(String),
    /// The name is a folder of an existing branch, or the other way round.
    Clashes(String),
}

impl NameProblem {
    /// The message shown for `name`.
    pub fn message(&self, name: &str) -> String {
        match self {
            NameProblem::Empty => "Enter a branch name.".to_owned(),
            NameProblem::Invalid => "Not a valid branch name.".to_owned(),
            NameProblem::Exists(other) => format!("A branch named {other} already exists."),
            NameProblem::Clashes(other) => format!("{name} clashes with branch {other}."),
        }
    }
}

/// Checks `name` (already trimmed) against the rules for a local branch and the `existing` local
/// branch names. When `renaming` is set, that branch itself is ignored. Names are compared
/// case-insensitively because branch ref files collide on Windows and macOS.
pub fn check_branch_name(name: &str, existing: &[&str], renaming: Option<&str>) -> Result<(), NameProblem> {
    if name.is_empty() {
        return Err(NameProblem::Empty);
    }
    if !is_valid_branch_name(name) {
        return Err(NameProblem::Invalid);
    }
    let lower = name.to_lowercase();
    let others = existing.iter().filter(|e| Some(**e) != renaming);
    for other in others {
        let o = other.to_lowercase();
        if o == lower {
            return Err(NameProblem::Exists((*other).to_owned()));
        }
        if o.starts_with(&format!("{lower}/")) || lower.starts_with(&format!("{o}/")) {
            return Err(NameProblem::Clashes((*other).to_owned()));
        }
    }
    Ok(())
}

/// Whether a branch can be fast-forwarded to its upstream, and if not, why.
#[derive(Debug, PartialEq)]
pub enum FastForward {
    Ready {
        behind: usize,
    },
    NoUpstream,
    /// An upstream is configured but its ref doesn't exist (deleted on the remote, not fetched).
    UpstreamGone,
    UpToDate,
    Ahead(usize),
    Diverged {
        ahead: usize,
        behind: usize,
    },
}

pub fn fast_forward_state(b: &BranchInfo) -> FastForward {
    if b.upstream_ref.is_none() {
        return if b.tracking.is_some() { FastForward::UpstreamGone } else { FastForward::NoUpstream };
    }
    match (b.ahead, b.behind) {
        (0, 0) => FastForward::UpToDate,
        (a, 0) => FastForward::Ahead(a),
        (0, behind) => FastForward::Ready { behind },
        (ahead, behind) => FastForward::Diverged { ahead, behind },
    }
}

/// `git branch -m old new`: git moves the branch's config and reflog with it.
pub fn rename_cmd(old: &str, new: &str) -> Vec<String> {
    args(&["branch", "-m", old, new])
}

/// The command that fast-forwards `b` to its upstream, when it can. The checked-out branch
/// merges with --ff-only; any other branch is updated with a local fetch, which moves the ref
/// without a checkout and refuses anything but a fast-forward (no `+`).
pub fn fast_forward_cmd(b: &BranchInfo) -> Option<Vec<String>> {
    if !matches!(fast_forward_state(b), FastForward::Ready { .. }) {
        return None;
    }
    let up = b.upstream_ref.as_deref()?;
    Some(if b.is_head {
        args(&["merge", "--ff-only", up])
    } else {
        args(&["fetch", "--no-recurse-submodules", ".", &format!("{up}:refs/heads/{}", b.name)])
    })
}

/// Sets `branch` to track `remote_branch` on `remote`. When the remote-tracking ref `exists`,
/// git's --set-upstream-to does it; otherwise (not pushed or fetched yet, which git refuses)
/// the two config values are written directly.
pub fn set_upstream_cmds(branch: &str, remote: &str, remote_branch: &str, exists: bool) -> Vec<Vec<String>> {
    if exists {
        vec![args(&["branch", &format!("--set-upstream-to={remote}/{remote_branch}"), branch])]
    } else {
        vec![
            args(&["config", &format!("branch.{branch}.remote"), remote]),
            args(&["config", &format!("branch.{branch}.merge"), &format!("refs/heads/{remote_branch}")]),
        ]
    }
}

pub fn unset_upstream_cmd(branch: &str) -> Vec<String> {
    args(&["branch", "--unset-upstream", branch])
}

/// Deletes `branch` and then its upstream branch on the remote, only when HEAD contains the
/// branch. `branch -d` alone isn't enough: for a branch with an upstream it checks only the
/// upstream, which is the ref deleted next, so a pushed but unmerged branch would end up on no
/// branch at all. The `merge-base --is-ancestor` check fails unless HEAD contains the branch tip,
/// and a failed command stops the sequence, so the remote branch is only removed once the local
/// one is gone.
pub fn delete_with_remote_cmds(branch: &str, t: &Tracking) -> Vec<Vec<String>> {
    vec![
        args(&["merge-base", "--is-ancestor", &format!("refs/heads/{branch}"), "HEAD"]),
        args(&["branch", "-d", branch]),
        args(&["push", &t.remote, "--delete", &t.merge]),
    ]
}

/// Pushes local `branch` to `remote_branch` on `remote` with an explicit refspec; `set_upstream`
/// adds -u so the branch tracks it afterwards.
pub fn push_cmd(branch: &str, remote: &str, remote_branch: &str, set_upstream: bool) -> Vec<String> {
    let mut cmd = args(&["push"]);
    if set_upstream {
        cmd.push("-u".into());
    }
    cmd.push(remote.to_owned());
    cmd.push(format!("refs/heads/{branch}:refs/heads/{remote_branch}"));
    cmd
}

#[cfg(test)]
mod tests;
