//! Undoing branch and tag changes: deleting a branch is restoring it in reverse and creating one
//! is deleting it in reverse, so both share `remove_branch` and `restore_branch`; tags likewise
//! share `put_tag` and `drop_tag`. A checkout that created a branch can delete it on undo and
//! make it again on redo.

use super::planning::{gone, stale, unsafe_};
use super::record::ref_exists;
use super::{Created, HeadPos, Plan};
use crate::gitcli::args;
use git2::{Oid, Repository};

fn tip(repo: &Repository, name: &str) -> Option<Option<Oid>> {
    repo.find_reference(&format!("refs/heads/{name}")).ok().map(|r| r.target())
}

/// Deletes branch `name` if it still points at `oid` (else stale, with `moved_msg`) and isn't
/// checked out.
pub(super) fn remove_branch(repo: &Repository, pos: &Option<HeadPos>, name: &str, oid: Oid, moved_msg: String) -> Plan {
    match tip(repo, name) {
        None => stale(format!("Branch {name} no longer exists")),
        Some(t) if t != Some(oid) => stale(moved_msg),
        _ if matches!(pos, Some(HeadPos::Branch(b)) if b == name) => {
            unsafe_(format!("Branch {name} is checked out; switch to another branch first."))
        }
        _ => Plan::Run(vec![args(&["branch", "-D", name])]),
    }
}

/// Makes branch `name` at `oid` again, with its upstream if that remote-tracking branch is still
/// there.
pub(super) fn restore_branch(repo: &Repository, name: &str, oid: Oid, upstream: Option<&str>) -> Plan {
    if tip(repo, name).is_some() {
        return stale(format!("A branch named {name} exists again"));
    }
    if repo.find_commit(oid).is_err() {
        return gone();
    }
    let mut cmds = vec![args(&["branch", name, &oid.to_string()])];
    if let Some(u) = upstream.filter(|u| ref_exists(repo, &format!("refs/remotes/{u}"))) {
        cmds.push(args(&["branch", "--set-upstream-to", u, name]));
    }
    Plan::Run(cmds)
}

/// Makes tag `name` point at `oid` again (the tag object for an annotated tag, so message,
/// tagger and signature come back with the same id), only while no tag of that name exists.
fn put_tag(repo: &Repository, name: &str, oid: Oid) -> Plan {
    let refname = format!("refs/tags/{name}");
    if ref_exists(repo, &refname) {
        return stale(format!("Tag {name} exists again"));
    }
    if repo.find_object(oid, None).is_err() {
        return gone();
    }
    // An empty old value: only if the tag doesn't exist.
    Plan::Run(vec![args(&["update-ref", &refname, &oid.to_string(), ""])])
}

/// Deletes tag `name` if it still points at `oid` (else stale, with `changed`).
fn drop_tag(repo: &Repository, name: &str, oid: Oid, changed: &str) -> Plan {
    match repo.find_reference(&format!("refs/tags/{name}")).ok().map(|r| r.target()) {
        None => stale(format!("Tag {name} no longer exists")),
        Some(t) if t != Some(oid) => stale(changed),
        Some(_) => Plan::Run(vec![args(&["tag", "-d", name])]),
    }
}

/// Undoes (or redoes) creating tag `name` at `oid` (the tag object for an annotated tag, so the
/// same tag comes back).
pub(super) fn tag(repo: &Repository, name: &str, oid: Oid, redo: bool) -> Plan {
    if redo { put_tag(repo, name, oid) } else { drop_tag(repo, name, oid, &format!("Tag {name} has changed since it was created")) }
}

/// Undoes (or redoes) deleting tag `name`, which pointed at `oid`: creating it in reverse. Only
/// the local tag; copies deleted from remotes stay deleted.
pub(super) fn deleted_tag(repo: &Repository, name: &str, oid: Oid, redo: bool) -> Plan {
    if redo { drop_tag(repo, name, oid, &format!("Tag {name} has changed since it was restored")) } else { put_tag(repo, name, oid) }
}

/// Undoes (or redoes) moving branch `name` from `from` to `to` with `update-ref`, whose old
/// value check refuses it if the branch moved meanwhile. Not while it is checked out here:
/// moving it would leave the index and files behind.
pub(super) fn set_branch(repo: &Repository, pos: &Option<HeadPos>, name: &str, from: Oid, to: Oid, redo: bool) -> Plan {
    let (expect, target) = if redo { (from, to) } else { (to, from) };
    match tip(repo, name) {
        None => stale(format!("Branch {name} no longer exists")),
        Some(t) if t != Some(expect) => stale(format!("Branch {name} has moved since it was restored")),
        _ if matches!(pos, Some(HeadPos::Branch(b)) if b == name) => {
            unsafe_(format!("Branch {name} is checked out; switch to another branch first."))
        }
        _ if repo.find_commit(target).is_err() => gone(),
        _ => {
            let reason = format!("Glowfly: {} move of {name}", if redo { "redo" } else { "undo" });
            Plan::Run(vec![args(&["update-ref", "-m", &reason, &format!("refs/heads/{name}"), &target.to_string(), &expect.to_string()])])
        }
    }
}

/// The command that deletes branch `c` after undoing the checkout that created it, if it still
/// points where it was created.
pub(super) fn delete_created(repo: &Repository, c: &Created) -> Result<Vec<String>, Plan> {
    if tip(repo, &c.name) != Some(Some(c.oid)) {
        return Err(unsafe_(format!(
            "Branch {} has commits made after it was created, so it wasn't deleted: undo again without deleting it",
            c.name
        )));
    }
    Ok(args(&["branch", "-D", &c.name]))
}

/// Redoes a checkout that created branch `c` after the branch was deleted: makes it again at the
/// same commit, checked out, with its upstream.
pub(super) fn recreate(repo: &Repository, c: &Created) -> Plan {
    if repo.find_commit(c.oid).is_err() {
        return gone();
    }
    let mut cmds = vec![args(&["checkout", "-b", &c.name, &c.oid.to_string(), "--"])];
    if let Some(u) = c.upstream.as_deref().filter(|u| ref_exists(repo, &format!("refs/remotes/{u}"))) {
        cmds.push(args(&["branch", "--set-upstream-to", u, &c.name]));
    }
    Plan::Run(cmds)
}
