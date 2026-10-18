//! Rebasing the current branch onto a commit chosen in the graph: when the menu and palette
//! entries can be used, and what they are called.

use git2::Oid;
use trellis_core::rebase;
use trellis_core::repo::Snapshot;

/// Why the current branch can't be rebased onto `oid`: an operation in progress, a detached
/// HEAD, or `oid` is HEAD or one of its ancestors in the loaded history (the branch already
/// contains it).
pub fn onto_commit_blocker(snap: &Snapshot, oid: Oid) -> Option<String> {
    if let Some(text) = rebase::in_progress_text(snap.state) {
        return Some(text);
    }
    let Some(current) = snap.head_branch.as_deref() else { return Some(rebase::DETACHED.into()) };
    let head = snap.head_oid?;
    let contained = head == oid || snap.commits_between(head, None).iter().any(|c| c.oid == oid);
    contained.then(|| rebase::contains_commit(current, oid))
}

/// The plain and the interactive entry: "Rebase main onto 1a2b3c4" and
/// "Interactive rebase main onto 1a2b3c4…".
pub fn onto_labels(current: &str, oid: Oid) -> (String, String) {
    let sha = &oid.to_string()[..7];
    (format!("Rebase {current} onto {sha}"), format!("Interactive rebase {current} onto {sha}…"))
}
