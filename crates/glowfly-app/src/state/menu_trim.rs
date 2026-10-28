//! What the long context menus leave out or fold together (the menu review's spec §10: hide
//! what can't apply, collapse three or more rows blocked for the same reason): history edits
//! blocked alike, cherry-picks and rebases onto a commit the branch already has, a Push to
//! submenu that offers nothing Push doesn't and a fast-forward without an upstream.

use crate::state::rebase_quick::EditBlockers;
use git2::Oid;
use glowfly_core::branch_ops::{FastForward, fast_forward_state};
use glowfly_core::repo::{BranchInfo, Snapshot};

/// The reason when reword, squash and drop are all blocked by the same one: the commit menu
/// then shows one blocked row for the three.
pub fn edits_blocked_alike(b: &EditBlockers) -> Option<&str> {
    match (&b.reword, &b.squash, &b.drop) {
        (Some(r), Some(s), Some(d)) if r == s && s == d => Some(r),
        _ => None,
    }
}

/// Whether the checked-out branch already contains `oid` (it is HEAD or one of its loaded
/// ancestors): rebasing the branch onto it or cherry-picking it can never apply.
pub fn head_contains(snap: &Snapshot, oid: Oid) -> bool {
    let Some(head) = snap.head_oid else { return false };
    head == oid || snap.commits_between(head, None).iter().any(|c| c.oid == oid)
}

/// Whether the Push to submenu offers more than Push: two or more remotes, or one whose push
/// would go elsewhere than Push's (an upstream on it under another name).
pub fn push_to_adds_a_choice(b: &BranchInfo, snap: &Snapshot) -> bool {
    match snap.remotes.as_slice() {
        [] => false,
        [(only, _)] => b.tracking.as_ref().is_some_and(|t| t.is_remote() && (t.remote != *only || t.merge != b.name)),
        _ => true,
    }
}

/// Whether fast-forward can apply to `b` at all: it has an upstream (its own row sets one).
pub fn fast_forward_applies(b: &BranchInfo) -> bool {
    fast_forward_state(b) != FastForward::NoUpstream
}

#[cfg(test)]
mod tests {
    use super::*;
    use glowfly_core::repo::Tracking;

    fn branch(name: &str, tracking: Option<(&str, &str)>) -> BranchInfo {
        BranchInfo {
            name: name.into(),
            oid: None,
            is_head: false,
            upstream: None,
            ahead: 0,
            behind: 0,
            tracking: tracking.map(|(r, m)| Tracking { remote: r.into(), merge: m.into() }),
            upstream_ref: None,
            merged: false,
        }
    }

    #[test]
    fn edits_fold_only_when_all_three_share_a_reason() {
        let all = |why: &str| EditBlockers { reword: Some(why.into()), squash: Some(why.into()), drop: Some(why.into()) };
        assert_eq!(edits_blocked_alike(&all("Not on main")), Some("Not on main"));
        let mixed = EditBlockers { squash: Some("Root".into()), ..all("Not on main") };
        assert_eq!(edits_blocked_alike(&mixed), None);
        assert_eq!(edits_blocked_alike(&EditBlockers::default()), None);
    }

    #[test]
    fn push_to_shows_only_when_it_offers_another_target() {
        let mut snap = crate::state::op_fixture::snapshot("push-to");
        snap.remotes = vec![("origin".into(), Vec::new())];
        assert!(!push_to_adds_a_choice(&branch("topic", None), &snap), "Push goes to origin already");
        assert!(!push_to_adds_a_choice(&branch("topic", Some(("origin", "topic"))), &snap));
        assert!(push_to_adds_a_choice(&branch("topic", Some(("origin", "other"))), &snap), "Push goes to origin/other");
        snap.remotes.push(("fork".into(), Vec::new()));
        assert!(push_to_adds_a_choice(&branch("topic", None), &snap));
        snap.remotes.clear();
        assert!(!push_to_adds_a_choice(&branch("topic", None), &snap));
    }

    #[test]
    fn head_contains_its_own_line() {
        let snap = crate::state::op_fixture::snapshot("head-contains");
        let head = snap.head_oid.unwrap();
        assert!(head_contains(&snap, head));
        let parent = snap.commits.iter().find(|c| c.oid == head).and_then(|c| c.parents.first().copied());
        if let Some(p) = parent {
            assert!(head_contains(&snap, p));
        }
    }

    #[test]
    fn fast_forward_needs_an_upstream() {
        assert!(!fast_forward_applies(&branch("x", None)));
        assert!(fast_forward_applies(&branch("x", Some(("origin", "x")))));
    }
}
