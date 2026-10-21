//! Rebasing the current branch onto a commit chosen in the graph: when the menu and palette
//! entries can be used, and what they are called.

use git2::{Oid, RepositoryState};
use glowfly_core::i18n::Lang;
use glowfly_core::repo::Snapshot;

/// Why the current branch can't be rebased onto `oid`: an operation in progress, a detached
/// HEAD, or `oid` is HEAD or one of its ancestors in the loaded history (the branch already
/// contains it).
pub fn onto_commit_blocker(t: Lang, snap: &Snapshot, oid: Oid) -> Option<String> {
    if let Some(why) = in_progress(t, snap.state) {
        return Some(why.to_owned());
    }
    let Some(current) = snap.head_branch.as_deref() else { return Some(t.tr("commits.blocked.detached").to_owned()) };
    let head = snap.head_oid?;
    let contained = head == oid || snap.commits_between(head, None).iter().any(|c| c.oid == oid);
    let sha = oid.to_string();
    contained.then(|| t.trf("commits.blocked.contains", &[("branch", &current), ("sha", &&sha[..7])]))
}

/// The reason for the operation in progress, if any.
pub(crate) fn in_progress(t: Lang, state: RepositoryState) -> Option<&'static str> {
    use RepositoryState::*;
    Some(match state {
        Clean => return None,
        Merge => t.tr("commits.blocked.in_progress.merge"),
        Rebase | RebaseInteractive | RebaseMerge => t.tr("commits.blocked.in_progress.rebase"),
        CherryPick | CherryPickSequence => t.tr("commits.blocked.in_progress.cherry_pick"),
        Revert | RevertSequence => t.tr("commits.blocked.in_progress.revert"),
        ApplyMailbox | ApplyMailboxOrRebase => t.tr("commits.blocked.in_progress.am"),
        Bisect => t.tr("commits.blocked.in_progress.bisect"),
    })
}

/// The palette's plain and interactive entries: "Rebase main onto 1a2b3c4" and
/// "Interactive rebase main onto 1a2b3c4…".
pub fn onto_labels(current: &str, oid: Oid) -> (String, String) {
    let sha = &oid.to_string()[..7];
    (format!("Rebase {current} onto {sha}"), format!("Interactive rebase {current} onto {sha}…"))
}

/// The commit menu's plain and interactive entries: the menu's header already names the commit.
pub fn onto_menu_labels(t: Lang, current: &str) -> (String, String) {
    let args: [(&str, &dyn std::fmt::Display); 1] = [("branch", &current)];
    (t.trf("commits.menu.rebase_onto", &args), t.trf("commits.menu.interactive_onto", &args))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn onto_blockers_follow_the_language() {
        let mut snap = crate::state::op_fixture::snapshot("onto-blockers");
        let oid = snap.head_oid.unwrap();
        let branch = snap.head_branch.clone().unwrap();
        snap.state = RepositoryState::Merge;
        let merge = onto_commit_blocker(Lang::En, &snap, oid);
        assert_eq!(merge.as_deref(), Some("Finish or abort the merge in progress before starting an interactive rebase."));
        assert_eq!(merge, glowfly_core::rebase::in_progress_text(RepositoryState::Merge));
        assert_ne!(onto_commit_blocker(Lang::De, &snap, oid), merge);
        snap.state = RepositoryState::Clean;
        let contained = onto_commit_blocker(Lang::En, &snap, oid);
        assert_eq!(contained, Some(glowfly_core::rebase::contains_commit(&branch, oid)));
        let de = onto_commit_blocker(Lang::De, &snap, oid).unwrap();
        let sha = &oid.to_string()[..7];
        assert!(de.contains(&branch) && de.contains(sha) && Some(&de) != contained.as_ref(), "{de}");
        snap.head_branch = None;
        assert_eq!(onto_commit_blocker(Lang::En, &snap, oid).as_deref(), Some(glowfly_core::rebase::DETACHED));
        assert_ne!(onto_commit_blocker(Lang::De, &snap, oid).as_deref(), Some(glowfly_core::rebase::DETACHED));
    }

    #[test]
    fn menu_labels_leave_the_commit_to_the_header() {
        let want = ("Rebase main onto this commit".to_owned(), "Interactive rebase main onto this commit…".to_owned());
        assert_eq!(super::onto_menu_labels(glowfly_core::i18n::Lang::En, "main"), want);
    }
}
