//! A remote-tracking branch's operations: the local branch that tracks it, whether it can be
//! checked out or compared with HEAD, and why not. The remote branch menu shows these reasons on
//! disabled items and `update` logs the same text when it refuses a late message.

use git2::Oid;
use trellis_core::branch_ops::{NameProblem, check_branch_name};
use trellis_core::gitcli::args;
use trellis_core::repo::{BranchInfo, Snapshot};

/// Whether local branch `b` tracks `name` on `remote`.
pub fn tracks(b: &BranchInfo, remote: &str, name: &str) -> bool {
    b.tracking.as_ref().is_some_and(|t| t.remote == remote && t.merge == name)
}

/// The local branch tracking `remote`/`name`: the checked-out one first, then the one named
/// `name`, then the first in `snap.locals`.
pub fn tracking_local<'a>(snap: &'a Snapshot, remote: &str, name: &str) -> Option<&'a BranchInfo> {
    let found: Vec<&BranchInfo> = snap.locals.iter().filter(|b| tracks(b, remote, name)).collect();
    found.iter().find(|b| b.is_head).or_else(|| found.iter().find(|b| b.name == name)).or(found.first()).copied()
}

/// The tip of remote-tracking branch `remote`/`name`, when the snapshot has it.
pub fn exists(snap: &Snapshot, remote: &str, name: &str) -> Option<Oid> {
    let (_, branches) = snap.remotes.iter().find(|(r, _)| r == remote)?;
    branches.iter().find(|(n, _)| n == name).map(|(_, o)| *o)
}

/// Why checking out `remote`/`name` as a new tracking branch is refused, when no local branch
/// tracks it: `name` can't be a local branch here, or it clashes with an existing local branch
/// (the same name, a case or Unicode variant, a folder clash or a folder spelled differently),
/// checked as the New branch dialog checks names.
pub fn checkout_blocked(snap: &Snapshot, remote: &str, name: &str) -> Option<String> {
    if tracking_local(snap, remote, name).is_some() {
        return None;
    }
    let locals: Vec<&str> = snap.locals.iter().map(|b| b.name.as_str()).collect();
    match check_branch_name(name, &locals, None).err()? {
        NameProblem::Exists(other) if other == name => {
            Some(format!("A local branch {name} already exists and doesn't track {remote}/{name}."))
        }
        NameProblem::Empty | NameProblem::Invalid => Some(format!("{name} can't be used as a local branch name.")),
        p => Some(p.message(name)),
    }
}

/// Creates a local branch tracking `remote`/`name` and checks it out.
pub fn checkout_cmd(remote: &str, name: &str) -> Vec<String> {
    args(&["checkout", "--track", &format!("{remote}/{name}"), "--"])
}

/// Whether the checked-out branch already tracks `remote`/`name`.
pub fn is_upstream_of_head(snap: &Snapshot, remote: &str, name: &str) -> bool {
    snap.locals.iter().any(|b| b.is_head && tracks(b, remote, name))
}

/// Why commit `oid` (the tip of the branch called `name` in messages) can't be compared with
/// HEAD's commit as a selected pair.
pub fn compare_blocked(snap: &Snapshot, oid: Oid, name: &str) -> Option<String> {
    let (Some(current), Some(head)) = (snap.head_branch.as_deref(), snap.head_oid) else {
        return Some("Check out a branch with commits to compare with.".to_owned());
    };
    if head == oid {
        return Some(format!("{name} is at the same commit as {current}."));
    }
    if !snap.row_of.contains_key(&oid) {
        return Some(format!("{name}'s commit isn't loaded in the graph."));
    }
    if !snap.row_of.contains_key(&head) {
        return Some(format!("{current}'s commit isn't loaded in the graph."));
    }
    None
}
