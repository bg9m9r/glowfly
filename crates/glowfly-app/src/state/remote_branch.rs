//! A remote-tracking branch's operations: the local branches that track it (and whether checking
//! one out fast-forwards it first), whether it can be checked out or compared with HEAD, and why
//! not, and how a graph label splits into remote and branch. The remote branch menu shows these
//! reasons on disabled items and `update` logs the same text when it refuses a late message.

use git2::Oid;
use glowfly_core::branch_ops::{FastForward, NameProblem, check_branch_name, fast_forward_state};
use glowfly_core::gitcli::args;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::{BranchInfo, Snapshot};

/// Whether local branch `b` tracks `name` on `remote`.
pub fn tracks(b: &BranchInfo, remote: &str, name: &str) -> bool {
    b.tracking.as_ref().is_some_and(|t| t.remote == remote && t.merge == name)
}

/// The local branches tracking `remote`/`name`: the checked-out one first, then the one named
/// `name`, then the others in `snap.locals`' order.
pub fn trackers<'a>(snap: &'a Snapshot, remote: &str, name: &str) -> Vec<&'a BranchInfo> {
    let mut found: Vec<&BranchInfo> = snap.locals.iter().filter(|b| tracks(b, remote, name)).collect();
    found.sort_by_key(|b| (!b.is_head, b.name != name));
    found
}

/// The local branch tracking `remote`/`name` that a plain checkout of it uses: the first of
/// `trackers`.
pub fn tracking_local<'a>(snap: &'a Snapshot, remote: &str, name: &str) -> Option<&'a BranchInfo> {
    trackers(snap, remote, name).into_iter().next()
}

/// What checking out local branch `b` from its upstream's row does, `tip` being the upstream's
/// tip: a plain checkout (up to date or only ahead), a fast-forward to `tip` first (only
/// behind), or a checkout that leaves it where it is (diverged).
#[derive(Debug, PartialEq)]
pub enum TrackingCheckout {
    Plain,
    FastForward { from: Oid, to: Oid },
    Diverged { ahead: usize, behind: usize },
}

pub fn tracking_checkout(b: &BranchInfo, tip: Oid) -> TrackingCheckout {
    match fast_forward_state(b) {
        FastForward::Ready { .. } => match b.oid {
            Some(from) if from != tip => TrackingCheckout::FastForward { from, to: tip },
            _ => TrackingCheckout::Plain,
        },
        FastForward::Diverged { ahead, behind } => TrackingCheckout::Diverged { ahead, behind },
        _ => TrackingCheckout::Plain,
    }
}

/// What checking out a diverged `branch` from `upstream`'s row says: it stays where it is.
pub fn diverged_note(t: Lang, branch: &str, upstream: &str, ahead: usize, behind: usize) -> String {
    let args: [(&str, &dyn std::fmt::Display); 4] = [("branch", &branch), ("upstream", &upstream), ("ahead", &ahead), ("behind", &behind)];
    t.trf("remote_branch.checkout.diverged", &args)
}

/// Splits remote-tracking label `label` ("origin/feature") into its remote and branch name, by
/// the longest remote in the snapshot followed by '/'; None when no remote matches.
pub fn split<'a>(snap: &Snapshot, label: &'a str) -> Option<(&'a str, &'a str)> {
    let remotes: Vec<&str> = snap.remotes.iter().map(|(r, _)| r.as_str()).collect();
    split_among(&remotes, label)
}

/// Splits remote-tracking label `label` into its remote and branch name, by the longest of
/// `remotes` followed by '/' (remote names may hold '/'); None when none matches.
pub fn split_among<'a>(remotes: &[&str], label: &'a str) -> Option<(&'a str, &'a str)> {
    let remote = remotes.iter().filter(|r| label.strip_prefix(**r).is_some_and(|rest| rest.starts_with('/'))).max_by_key(|r| r.len())?;
    Some((&label[..remote.len()], &label[remote.len() + 1..]))
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
