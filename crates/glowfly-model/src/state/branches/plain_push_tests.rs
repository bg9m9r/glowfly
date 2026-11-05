//! `branches::push_blocked` (the branch menu's Push) and `plain_push`'s texts, on the toolbar
//! fixture with `BranchInfo::push` set by hand (`toolbar_tips::tests::push_to`).

use super::*;
use crate::state::branches;
use crate::state::toolbar_tips::tests::{diverge, push_to, state};
use git2::Oid;
use glowfly_core::repo::Snapshot;

fn head(snap: &Snapshot) -> &BranchInfo {
    snap.locals.iter().find(|b| b.is_head).unwrap()
}

fn blocked(snap: &Snapshot) -> Option<String> {
    branches::push_blocked(Lang::En, head(snap), true)
}

fn tip() -> Oid {
    Oid::from_bytes(&[7; 20]).unwrap()
}

#[test]
fn each_refusal_has_its_sentence() {
    let mut st = state("plain-push-refusals");
    let snap = st.snap.as_mut().unwrap();
    diverge(snap, 1, 0, true);
    push_to(snap, "fork", Err(PushRefusal::NotUpstreamRemote { upstream_remote: "origin".into() }), None, (0, 0));
    assert_eq!(
        blocked(snap).as_deref(),
        Some(
            "push.default is upstream, and remote.pushDefault sends main to fork rather than origin, where its upstream origin/main \
             is. Git refuses that push. Use Push to, or set push.default to simple or current."
        )
    );
    let b = snap.locals.iter_mut().find(|b| b.is_head).unwrap();
    b.name = "feat".into();
    push_to(snap, "origin", Err(PushRefusal::NameDiffers { merge: "main".into() }), None, (0, 0));
    assert_eq!(
        blocked(snap).as_deref(),
        Some(
            "feat tracks origin/main, which has another name, so a plain git push (push.default is simple) refuses it. Use Push to \
             origin to update origin/main, or set push.default to upstream."
        )
    );
    let others = [
        (PushRefusal::Nothing, "push.default is \"nothing\""),
        (PushRefusal::Invalid("bogus".into()), "push.default is set to \"bogus\""),
        (PushRefusal::NotOnRemote, "push.default is matching, and fork has no feat"),
        (PushRefusal::NotInRefspecs, "remote.fork.push doesn't include feat"),
        (PushRefusal::NoRemote, "fork isn't a remote in this repository"),
    ];
    for (refusal, start) in others {
        push_to(snap, "fork", Err(refusal.clone()), None, (0, 0));
        let why = blocked(snap).unwrap();
        assert!(why.starts_with(start), "{refusal:?}: {why}");
        assert_eq!(refused(Lang::En, head(snap)), Some(why));
        assert!(refused_any(head(snap)));
        let de = branches::push_blocked(Lang::De, head(snap), true).unwrap();
        assert_ne!(de, blocked(snap).unwrap(), "{refusal:?} has German text");
    }
}

#[test]
fn a_push_elsewhere_is_judged_against_that_ref() {
    let mut st = state("plain-push-elsewhere");
    let snap = st.snap.as_mut().unwrap();
    // Level with origin/main, which alone would refuse Push as up to date.
    diverge(snap, 0, 0, true);
    push_to(snap, "fork", Ok("refs/heads/main"), Some(tip()), (2, 0));
    assert_eq!(blocked(snap), None, "fork/main lacks two commits");
    push_to(snap, "fork", Ok("refs/heads/main"), Some(tip()), (0, 1));
    let behind = "main is behind fork/main, so git would reject the push. Bring in fork/main's commits first, or force push.";
    assert_eq!(blocked(snap).as_deref(), Some(behind));
    push_to(snap, "fork", Ok("refs/heads/main"), Some(tip()), (0, 0));
    assert_eq!(blocked(snap).as_deref(), Some("main is up to date with fork/main."));
    push_to(snap, "fork", Ok("refs/heads/main"), None, (0, 0));
    assert_eq!(blocked(snap), None, "the push creates fork/main");
    assert!(!refused_any(head(snap)));
}

#[test]
fn a_push_to_the_upstream_keeps_the_upstream_rules() {
    let mut st = state("plain-push-upstream");
    let snap = st.snap.as_mut().unwrap();
    diverge(snap, 1, 2, true);
    // The target's own counts are stale here; the upstream's are what counts.
    push_to(snap, "origin", Ok("refs/heads/main"), Some(tip()), (5, 0));
    assert_eq!(blocked(snap).as_deref(), Some("main is behind origin/main. Fast-forward or pull first."));
    assert_eq!(counts(head(snap), dest(head(snap)).unwrap().1), (1, 2));
    // Without a resolved target (hand-built snapshots): the same rules.
    snap.locals.iter_mut().find(|b| b.is_head).unwrap().push = None;
    assert_eq!(blocked(snap), branches::upstream_push_blocked(head(snap), true));
    assert!(blocked(snap).is_some());
    assert_eq!(branches::push_blocked(Lang::En, head(snap), false).as_deref(), Some(branches::NO_REMOTES));
}
