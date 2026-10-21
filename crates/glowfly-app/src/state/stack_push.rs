//! Pushing a stack: which branches make up the stack a branch is in, where each layer goes,
//! whether any needs a push, and why Push stack can't run (the same text for the branch menu's
//! disabled item, the palette and `update`'s refusal).

use super::branches::{self, NO_REMOTES};
use glowfly_core::i18n::Lang;
use glowfly_core::repo::Snapshot;
use glowfly_core::stack::{PushTarget, Stacks};

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

/// Where each of `layers` goes (`branches::push_target` with `remote`), with the remote tip the
/// snapshot shows there. Layers that are gone or have nowhere to go are left out.
pub fn targets(snap: &Snapshot, layers: &[String], remote: Option<&str>) -> Vec<PushTarget> {
    layers
        .iter()
        .filter_map(|name| {
            let b = snap.locals.iter().find(|b| b.name == *name)?;
            let (remote, dest, set_upstream) = branches::push_target(b, snap, remote)?;
            let seen = remote_tip(snap, &remote, &dest);
            Some(PushTarget { branch: b.name.clone(), remote, dest, set_upstream, seen })
        })
        .collect()
}

/// The tip of `remote`/`branch` in the snapshot.
pub fn remote_tip(snap: &Snapshot, remote: &str, branch: &str) -> Option<git2::Oid> {
    snap.remotes.iter().find(|(r, _)| r == remote).and_then(|(_, bs)| bs.iter().find(|(n, _)| n == branch)).map(|(_, o)| *o)
}

/// Whether any target's remote tip differs from its local branch's tip.
pub fn needs_push(snap: &Snapshot, targets: &[PushTarget]) -> bool {
    targets.iter().any(|t| t.seen != snap.local_oid(&t.branch))
}

/// Why the stack `branch` is in can't be pushed now: an operation in progress, no remotes, or
/// every layer already at its tip on its push target.
pub fn push_blocker(t: Lang, snap: &Snapshot, branch: &str) -> Option<String> {
    if let Some(why) = super::rebase_onto::in_progress(t, snap.state) {
        return Some(why.to_owned());
    }
    if snap.remotes.is_empty() {
        return Some(NO_REMOTES.to_owned());
    }
    let layers = layers(&snap.stacks, branch);
    (!needs_push(snap, &targets(snap, &layers, None))).then(|| t.tr("stack.blocked.pushed").to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::{Oid, RepositoryState};
    use glowfly_core::repo::{BranchInfo, Tracking};
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
