//! Rebasing the current branch onto a commit chosen in the graph: when the menu and palette
//! entries can be used, and what they are called.

use git2::Oid;
use trellis_core::i18n::Lang;
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
    #[test]
    fn menu_labels_leave_the_commit_to_the_header() {
        let want = ("Rebase main onto this commit".to_owned(), "Interactive rebase main onto this commit…".to_owned());
        assert_eq!(super::onto_menu_labels(trellis_core::i18n::Lang::En, "main"), want);
    }
}
