//! Force push with lease of any local branch and Push to all remotes (update/branch_push.rs):
//! which branches a row or toast may force push (`force_blocked`: where `force_dest` sends it
//! must exist and be allowed; never the destination's default branch or a Git Flow base, as
//! Rename refuses on the remote), the confirmation naming the commits the destination would
//! lose (`force_confirm`), the toolbar's confirmation, which warns plainly before overwriting a
//! default or base branch (`head_force_confirm`), the re-check's text when the destination
//! changed (`changed`), and Push to all remotes' headline.

use super::force_dest::{self, ForceDest};
use super::{plain_push, upstream_label};
use crate::msg::{BranchMsg, JobMsg, Msg};
use crate::state::{branch_sync, branches};
use glowfly_core::gitflow::FlowBase;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::{BranchInfo, Snapshot};
use glowfly_core::text::redact_credentials;

/// Why `b` can't be force pushed, or None when it may be: no other worktree may be rebasing or
/// bisecting it, its force destination (`force_dest::of`) must exist and be allowed, and it
/// must not be the destination's default branch, nor either name a Git Flow base (others build
/// on those).
pub fn force_blocked(t: Lang, snap: &Snapshot, b: &BranchInfo) -> Option<String> {
    // Its tip moves when that rebase ends: the lease would hold for the wrong commits.
    if let Some(why) = crate::state::worktree_branches::held_blocked(t, snap, &b.name) {
        return Some(why);
    }
    match force_dest::of(t, snap, b) {
        Err(why) => Some(why),
        Ok(d) => base_blocked(t, snap, &b.name, &d),
    }
}

/// A branch others build on: the remote's default branch, or a Git Flow base (by its name).
enum Base<'a> {
    Default(&'a str),
    Production(&'a str),
    Development(&'a str),
}

/// Whether force pushing local `name` to branch `dest` (None: another ref) on `remote`
/// overwrites a branch others build on: the remote's default branch, or a Git Flow base by
/// either name. This repository (`.`) has no default branch of its own to check.
fn base<'a>(snap: &Snapshot, name: &'a str, remote: &str, dest: Option<&'a str>) -> Option<Base<'a>> {
    if let Some(d) = dest.filter(|d| remote != "." && branch_sync::is_default(snap, remote, d)) {
        return Some(Base::Default(d));
    }
    [Some(name), dest].into_iter().flatten().find_map(|n| match snap.gitflow.base_role(n)? {
        FlowBase::Production => Some(Base::Production(n)),
        FlowBase::Development => Some(Base::Development(n)),
    })
}

/// Why a row or toast may not force push local `name` to `d`.
fn base_blocked(t: Lang, snap: &Snapshot, name: &str, d: &ForceDest) -> Option<String> {
    Some(match base(snap, name, &d.remote, d.branch())? {
        Base::Default(n) => t.trf("branch_push.force.default", &[("branch", &n), ("remote", &d.remote)]),
        Base::Production(n) => t.trf("branch_push.force.production", &[("branch", &n)]),
        Base::Development(n) => t.trf("branch_push.force.development", &[("branch", &n)]),
    })
}

/// The toolbar's Force push confirmation for the checked-out branch. Force pushing the
/// destination's default branch or a Git Flow base stays possible there (a repository of one
/// may need it), so for those the body says plainly which branch it is, what it overwrites and
/// how many commits that has that the branch lacks. A destination other than the upstream
/// says which commits it would lose.
pub fn head_force_confirm(t: Lang, snap: &Snapshot) -> Msg {
    let plain = t.tr("toolbar.force_push.body");
    let body = match snap.head_info().and_then(|b| head_force_extra(t, snap, b)) {
        Some(extra) => format!("{plain} {extra}"),
        None => plain.to_owned(),
    };
    let (title, button) = (t.tr("toolbar.force_push.title"), t.tr("toolbar.force_push.button"));
    Msg::confirm_danger(title, body, button, Msg::Job(JobMsg::Push { force: true }))
}

/// What the toolbar confirmation adds to its plain question for `b`.
fn head_force_extra(t: Lang, snap: &Snapshot, b: &BranchInfo) -> Option<String> {
    let d = force_dest::of_head(t, snap, b).ok()??;
    let Some(kind) = base(snap, &b.name, &d.remote, d.branch()) else {
        return (!d.upstream).then(|| lost(t, b, &d));
    };
    let base_name = match kind {
        Base::Default(n) | Base::Production(n) | Base::Development(n) => n,
    };
    let dest = d.branch().unwrap_or(&d.dest_ref);
    let args: [(&str, &dyn std::fmt::Display); 5] =
        [("branch", &b.name), ("base", &base_name), ("remote", &d.remote), ("upstream", &d.label), ("dest", &dest)];
    let mut warning = match kind {
        Base::Default(_) => t.trf("branch_push.force.warn.default", &args),
        Base::Production(_) => t.trf("branch_push.force.warn.production", &args),
        Base::Development(_) => t.trf("branch_push.force.warn.development", &args),
    };
    if d.behind > 0 {
        warning = format!("{warning} {}", t.trn("branch_push.force.body", d.behind, &args));
    }
    Some(warning)
}

/// What force pushing `b` to `d` costs: the commits there that `b` lacks, or that it isn't
/// there as last fetched.
fn lost(t: Lang, b: &BranchInfo, d: &ForceDest) -> String {
    let args: [(&str, &dyn std::fmt::Display); 3] = [("branch", &b.name), ("upstream", &d.label), ("remote", &d.remote)];
    match (d.seen, d.behind) {
        (None, _) => t.trf("branch_push.force.body.gone", &args),
        (Some(_), 0) => t.trf("branch_push.force.body.none", &args),
        (Some(_), n) => t.trn("branch_push.force.body", n, &args),
    }
}

/// Why the toolbar's Force push of the checked-out branch is refused: git refuses its plain
/// push, or there is nothing to lease against.
pub fn head_force_blocked(t: Lang, snap: &Snapshot) -> Option<String> {
    snap.head_info().and_then(|b| force_dest::of_head(t, snap, b).err())
}

/// The danger confirmation for force pushing `b`, naming how many commits its destination has
/// that `b` lacks (from the remote-tracking ref), or why it is refused.
pub fn force_confirm(t: Lang, snap: &Snapshot, b: &BranchInfo) -> Result<Msg, String> {
    if let Some(why) = force_blocked(t, snap, b) {
        return Err(why);
    }
    let d = force_dest::of(t, snap, b)?;
    let args: [(&str, &dyn std::fmt::Display); 3] = [("branch", &b.name), ("upstream", &d.label), ("remote", &d.remote)];
    let body = format!("{} {}", lost(t, b, &d), t.trf("branch_push.force.body.lease", &args));
    let title = t.trf("branch_push.force.title", &[("branch", &b.name)]);
    let then = Msg::Branch(BranchMsg::ForcePush { branch: b.name.clone(), remote: d.remote, dest: d.dest_ref, seen: d.seen });
    Ok(Msg::confirm_danger(&title, body, t.tr("toolbar.force_push.button"), then))
}

/// What the branch menu's Force push item calls its destination: where it goes, or where it
/// would be said to go were it not refused.
pub fn force_label(t: Lang, snap: &Snapshot, b: &BranchInfo) -> String {
    if let Ok(d) = force_dest::of(t, snap, b) {
        return d.label;
    }
    match plain_push::dest(b) {
        Some((p, d)) => redact_credentials(&p.label(d)),
        None => upstream_label(b).unwrap_or_default(),
    }
}

/// Why a confirmed force push of `b` to (`remote`, `dest`) can't go now that its destination
/// is not that one: its upstream moved to another name on that remote, else why it is refused
/// now (`now`), else that its destination changed.
pub fn changed(t: Lang, b: &BranchInfo, (remote, dest): (&str, &str), now: Result<ForceDest, String>) -> String {
    let confirmed = force_dest::label(remote, dest);
    let short = dest.strip_prefix("refs/heads/").unwrap_or(dest);
    if branches::remote_tracking(b).is_some_and(|up| up.remote == remote && up.merge != short) {
        return t.trf("branch_push.force.upstream_changed", &[("branch", &b.name), ("upstream", &confirmed)]);
    }
    match now {
        Err(why) => why,
        Ok(_) => t.trf("branch_push.force.dest_changed", &[("branch", &b.name), ("dest", &confirmed)]),
    }
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
