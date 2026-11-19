//! Pushing a stack: which branches make up the stack a branch is in, where each layer goes
//! (to a chosen remote, else where its plain push goes, as a force push does:
//! `branches/force_dest.rs`), whether any needs a push, and why Push stack can't run (the same
//! text for the branch menu's disabled item, the palette and `update`'s refusal): a layer git
//! refuses to push, or one whose destination isn't a branch on a named remote, blocks it.

use super::branches::{self, NO_REMOTES, force_dest, plain_push};
use glowfly_core::i18n::Lang;
use glowfly_core::repo::{BranchInfo, Snapshot};
use glowfly_core::stack::{PushTarget, Stacks};
use glowfly_core::text::redact_credentials;

/// Every layer of the stack `branch` is in, bottom first: the bottom layer (the one built on
/// the base) then everything stacked on it, depth first in name order, so siblings come too.
/// Empty for a branch in no stack.
pub fn layers(stacks: &Stacks, branch: &str) -> Vec<String> {
    let mut seen: Vec<&str> = vec![branch];
    let mut bottom = branch;
    while let Some(l) = stacks.below(bottom) {
        if seen.contains(&l.on.as_str()) {
            break;
        }
        seen.push(&l.on);
        bottom = &l.on;
    }
    let above = stacks.descendants(bottom);
    if above.is_empty() {
        return Vec::new();
    }
    std::iter::once(bottom).chain(above).map(str::to_owned).collect()
}

/// Where each of `layers` goes, with the remote tip the snapshot shows there: with a `remote`
/// chosen, `branches::push_target` of it; without one, where the layer's plain push goes
/// (`own_target`). Layers that are gone, have nowhere to go or can't be pushed (`layer_blocked`)
/// are left out.
pub fn targets(snap: &Snapshot, layers: &[String], remote: Option<&str>) -> Vec<PushTarget> {
    layers
        .iter()
        .filter_map(|name| {
            let b = snap.locals.iter().find(|b| b.name == *name)?;
            let Some(remote) = remote else { return own_target(snap, b).ok().flatten() };
            let (remote, dest, set_upstream) = branches::push_target(b, snap, Some(remote))?;
            let seen = remote_tip(snap, &remote, &dest);
            Some(PushTarget { branch: b.name.clone(), remote, dest, set_upstream, seen })
        })
        .collect()
}

/// Why a layer has no target of its own.
enum Unpushable {
    /// Git refuses its plain push, or there is no remote-tracking ref to compare against.
    Refused,
    /// A plain push sends it to this, which isn't a branch on a named remote.
    NotABranch(String),
}

/// Where `b`'s plain push goes, as a target that sets no upstream and sees the tip there; a
/// layer whose plain push isn't resolved goes where an explicit push does (the default remote,
/// with -u for a branch tracking nothing). Ok(None): nowhere to go.
fn own_target(snap: &Snapshot, b: &BranchInfo) -> Result<Option<PushTarget>, Unpushable> {
    let Some(p) = plain_push::resolved(b) else {
        return Ok(branches::push_target(b, snap, None).map(|(remote, dest, set_upstream)| {
            let seen = remote_tip(snap, &remote, &dest);
            PushTarget { branch: b.name.clone(), remote, dest, set_upstream, seen }
        }));
    };
    let Ok(d) = &p.dest else { return Err(Unpushable::Refused) };
    if d.tracking_ref.is_none() {
        return Err(Unpushable::Refused);
    }
    match d.dest_ref.strip_prefix("refs/heads/") {
        Some(dest) if snap.remotes.iter().any(|(r, _)| *r == p.remote) => Ok(Some(PushTarget {
            branch: b.name.clone(),
            remote: p.remote.clone(),
            dest: dest.to_owned(),
            set_upstream: false,
            seen: d.tip,
        })),
        _ => Err(Unpushable::NotABranch(redact_credentials(&p.label(d)))),
    }
}

/// Why the first of `layers` (bottom first) that can't be pushed can't be, or None when each
/// can: git refuses its plain push (or there is no lease to take), or the plain push goes
/// somewhere Push stack can't send it.
fn layer_blocked(t: Lang, snap: &Snapshot, layers: &[String]) -> Option<String> {
    layers.iter().filter_map(|name| snap.locals.iter().find(|b| b.name == *name)).find_map(|b| match own_target(snap, b) {
        Ok(_) => None,
        Err(Unpushable::Refused) => force_dest::of(t, snap, b).err(),
        Err(Unpushable::NotABranch(dest)) => Some(t.trf("stack.blocked.layer_dest", &[("branch", &b.name), ("dest", &dest)])),
    })
}

/// The tip of `remote`/`branch` in the snapshot.
pub fn remote_tip(snap: &Snapshot, remote: &str, branch: &str) -> Option<git2::Oid> {
    snap.remotes.iter().find(|(r, _)| r == remote).and_then(|(_, bs)| bs.iter().find(|(n, _)| n == branch)).map(|(_, o)| *o)
}

/// Whether any target's remote tip differs from its local branch's tip.
pub fn needs_push(snap: &Snapshot, targets: &[PushTarget]) -> bool {
    targets.iter().any(|t| t.seen != snap.local_oid(&t.branch))
}

/// Why the stack `branch` is in can't be pushed now: an operation in progress, no remotes, a
/// layer that can't be pushed (`layer_blocked`), or every layer already at its tip on its push
/// target.
pub fn push_blocker(t: Lang, snap: &Snapshot, branch: &str) -> Option<String> {
    if let Some(why) = super::rebase_onto::in_progress(t, snap.state) {
        return Some(why.to_owned());
    }
    if snap.remotes.is_empty() {
        return Some(NO_REMOTES.to_owned());
    }
    let layers = layers(&snap.stacks, branch);
    if let Some(why) = layer_blocked(t, snap, &layers) {
        return Some(why);
    }
    (!needs_push(snap, &targets(snap, &layers, None))).then(|| t.tr("stack.blocked.pushed").to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::{Oid, RepositoryState};
    use glowfly_core::repo::{BranchInfo, PushDest, PushRefusal, PushRule, PushTarget as CorePush, Tracking};
    use glowfly_core::stack::StackLink;

    fn link(branch: &str, on: &str) -> StackLink {
        StackLink { branch: branch.into(), on: on.into(), base: Oid::zero(), behind: false }
    }

    /// low <- mid <- top and low <- side, every branch at the snapshot's one commit.
    fn tree(name: &str) -> Snapshot {
        let mut snap = crate::state::op_fixture::snapshot(name);
        let tip = snap.head_oid;
        let base = snap.locals[0].clone();
        for n in ["low", "mid", "side", "top"] {
            snap.locals.push(BranchInfo { name: n.into(), oid: tip, is_head: false, tracking: None, upstream: None, ..base.clone() });
        }
        snap.stacks = Stacks { links: vec![link("mid", "low"), link("side", "low"), link("top", "mid")] };
        snap
    }

    #[test]
    fn layers_cover_the_whole_tree_from_any_member() {
        let snap = tree("stack-push-layers");
        for from in ["low", "mid", "top", "side"] {
            assert_eq!(layers(&snap.stacks, from), ["low", "mid", "top", "side"], "from {from}");
        }
        assert!(layers(&snap.stacks, "main").is_empty());
        assert!(layers(&snap.stacks, "elsewhere").is_empty());
    }

    #[test]
    fn needs_push_compares_each_remote_tip_with_the_local_tip() {
        let mut snap = tree("stack-push-needs");
        snap.remotes = vec![("origin".into(), Vec::new())];
        let names = layers(&snap.stacks, "mid");
        let ts = targets(&snap, &names, None);
        assert_eq!(ts.len(), 4);
        assert!(ts.iter().all(|t| t.remote == "origin" && t.set_upstream && t.seen.is_none()), "{ts:?}");
        assert!(needs_push(&snap, &ts));
        let tip = snap.head_oid.unwrap();
        snap.remotes[0].1 = ["low", "mid", "side", "top"].iter().map(|n| (n.to_string(), tip)).collect();
        assert!(!needs_push(&snap, &targets(&snap, &names, None)));
        // A layer tracking another name keeps it; one tracking a local branch goes under its own.
        let mid = snap.locals.iter_mut().find(|b| b.name == "mid").unwrap();
        mid.tracking = Some(Tracking { remote: "origin".into(), merge: "feature/mid".into() });
        let low = snap.locals.iter_mut().find(|b| b.name == "low").unwrap();
        low.tracking = Some(Tracking { remote: ".".into(), merge: "main".into() });
        let ts = targets(&snap, &names, None);
        assert_eq!((ts[1].dest.as_str(), ts[1].set_upstream, ts[1].seen), ("feature/mid", false, None));
        assert_eq!((ts[0].dest.as_str(), ts[0].set_upstream), ("low", false), "a local upstream isn't moved");
    }

    /// Gives `name` an upstream on origin and a plain push to `remote` that ends as `dest`.
    fn pushing(snap: &mut Snapshot, name: &str, remote: &str, dest: Result<PushDest, PushRefusal>) {
        let b = snap.locals.iter_mut().find(|b| b.name == name).unwrap();
        b.tracking = Some(Tracking { remote: "origin".into(), merge: name.into() });
        b.upstream = Some(format!("origin/{name}"));
        b.upstream_ref = Some(format!("refs/remotes/origin/{name}"));
        b.push = Some(CorePush { remote: remote.into(), key: None, missing: None, dest });
    }

    fn to_fork(name: &str, dest_ref: &str, tip: Oid) -> Result<PushDest, PushRefusal> {
        let tracking_ref = Some(format!("refs/remotes/fork/{name}"));
        Ok(PushDest { dest_ref: dest_ref.into(), tracking_ref, tip: Some(tip), ahead: 1, behind: 0, rule: PushRule::Default })
    }

    #[test]
    fn layers_go_where_their_plain_push_goes() {
        let mut snap = tree("stack-push-plain");
        snap.remotes = vec![("origin".into(), Vec::new()), ("fork".into(), Vec::new())];
        let tip = Oid::from_bytes(&[7; 20]).unwrap();
        for n in ["low", "mid", "side", "top"] {
            pushing(&mut snap, n, "fork", to_fork(n, &format!("refs/heads/{n}"), tip));
        }
        let names = layers(&snap.stacks, "mid");
        let ts = targets(&snap, &names, None);
        assert_eq!(ts.len(), 4);
        assert!(ts.iter().all(|t| t.remote == "fork" && !t.set_upstream && t.seen == Some(tip)), "{ts:?}");
        assert_eq!(ts[1].dest, "mid");
        // A chosen remote is today's: the upstream's own name, nothing seen there.
        let ts = targets(&snap, &names, Some("origin"));
        assert!(ts.iter().all(|t| t.remote == "origin" && !t.set_upstream && t.seen.is_none()), "{ts:?}");
        assert_eq!(push_blocker(Lang::En, &snap, "mid"), None);
    }

    #[test]
    fn a_layer_git_refuses_to_push_blocks_the_stack() {
        let mut snap = tree("stack-push-refused");
        snap.remotes = vec![("origin".into(), Vec::new()), ("fork".into(), Vec::new())];
        let tip = Oid::from_bytes(&[7; 20]).unwrap();
        for n in ["low", "side", "top"] {
            pushing(&mut snap, n, "fork", to_fork(n, &format!("refs/heads/{n}"), tip));
        }
        pushing(&mut snap, "mid", "fork", Err(PushRefusal::Nothing));
        let mid = snap.locals.iter().find(|b| b.name == "mid").unwrap();
        let why = super::plain_push::refused(Lang::En, mid).unwrap();
        assert_eq!(push_blocker(Lang::En, &snap, "top").as_deref(), Some(why.as_str()));
        assert_ne!(push_blocker(Lang::De, &snap, "top").as_deref(), Some(why.as_str()));
        // The layer is left out of the targets, which is never pushed without the blocker.
        assert_eq!(targets(&snap, &layers(&snap.stacks, "top"), None).len(), 3);
    }

    #[test]
    fn a_layer_that_goes_to_another_ref_blocks_the_stack() {
        let mut snap = tree("stack-push-ref");
        snap.remotes = vec![("origin".into(), Vec::new()), ("fork".into(), Vec::new())];
        let tip = Oid::from_bytes(&[7; 20]).unwrap();
        for n in ["low", "side", "top"] {
            pushing(&mut snap, n, "fork", to_fork(n, &format!("refs/heads/{n}"), tip));
        }
        pushing(&mut snap, "mid", "fork", to_fork("mid", "refs/for/mid", tip));
        let en = push_blocker(Lang::En, &snap, "low").unwrap();
        assert_eq!(
            en,
            "A plain push sends mid to fork refs/for/mid, which isn't a branch on one of this repository's remotes, so Push stack can't push it. Push mid on its own, or change where it pushes."
        );
        assert_ne!(push_blocker(Lang::De, &snap, "low"), Some(en));
    }

    #[test]
    fn blockers_are_localised() {
        let mut snap = tree("stack-push-blockers");
        assert_eq!(push_blocker(Lang::En, &snap, "mid").as_deref(), Some(NO_REMOTES));
        snap.remotes = vec![("origin".into(), Vec::new())];
        assert_eq!(push_blocker(Lang::En, &snap, "mid"), None);
        let tip = snap.head_oid.unwrap();
        snap.remotes[0].1 = ["low", "mid", "side", "top"].iter().map(|n| (n.to_string(), tip)).collect();
        let en = push_blocker(Lang::En, &snap, "mid").unwrap();
        assert_eq!(en, "Every branch in the stack is already pushed.");
        assert_ne!(push_blocker(Lang::De, &snap, "mid"), Some(en));
        snap.state = RepositoryState::Merge;
        let busy = push_blocker(Lang::En, &snap, "mid").unwrap();
        assert_eq!(Some(busy.as_str()), super::super::rebase_onto::in_progress(Lang::En, RepositoryState::Merge));
        assert_ne!(push_blocker(Lang::De, &snap, "mid"), Some(busy));
    }
}
