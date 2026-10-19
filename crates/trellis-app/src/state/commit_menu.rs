//! The commit menus' wording: the reset submenu's title and modes, the header's summary, and a
//! commit named by its summary in confirmations.

use crate::msg::Msg;
use git2::Oid;
use trellis_core::i18n::Lang;
use trellis_core::repo::{CommitInfo, Snapshot};

/// The most characters of a summary the menu's header shows.
const HEADER_CHARS: usize = 80;

fn short(oid: Oid) -> String {
    oid.to_string()[..7].to_owned()
}

/// The reset submenu's title: "Reset main to this commit", or "Reset to this commit" when
/// HEAD is detached.
pub fn reset_title(t: Lang, current: Option<&str>) -> String {
    match current {
        Some(b) => t.trf("commits.menu.reset.title", &[("branch", &b)]),
        None => t.tr("commits.menu.reset.title_detached").to_owned(),
    }
}

/// The summary for the menu's header: trimmed, and cut to 80 characters with an ellipsis.
pub fn header_summary(summary: &str) -> String {
    let s = summary.trim();
    if s.chars().count() <= HEADER_CHARS {
        return s.to_owned();
    }
    let mut cut: String = s.chars().take(HEADER_CHARS - 1).collect();
    cut.push('…');
    cut
}

/// `"Fix build" (1a2b3c4)`, or just the short SHA for a commit without a summary.
pub fn commit_name(c: &CommitInfo) -> String {
    let summary = c.summary.trim();
    let sha = short(c.oid);
    if summary.is_empty() { sha } else { format!("\"{summary}\" ({sha})") }
}

/// The Hard reset confirmation's body. `current` is the checked-out branch (None: detached).
pub fn hard_reset_body(t: Lang, current: Option<&str>, c: &CommitInfo) -> String {
    t.trf("commits.menu.reset.hard_body", &[("branch", &current.unwrap_or("HEAD")), ("commit", &commit_name(c))])
}

/// The Hard reset item's message: the bare reset when the auto-stash prompt will ask instead
/// (so it asks once), otherwise the reset behind its danger confirmation. The prompt asks only
/// when its gate blocks; with an operation in progress or conflicted files it doesn't, so the
/// confirmation stays.
pub fn hard_reset_msg(t: Lang, snap: &Snapshot, c: &CommitInfo) -> Msg {
    let sha = c.oid.to_string();
    let hard = Msg::git("Reset (hard)", &["reset", "--hard", &sha]);
    if crate::state::auto_stash::blocks(snap) {
        return hard;
    }
    let body = hard_reset_body(t, snap.head_branch.as_deref(), c);
    Msg::confirm_danger(t.tr("commits.menu.reset.hard_title"), body, t.tr("commits.menu.reset.button"), hard)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::op_fixture::{rows, snapshot};
    use git2::RepositoryState;

    fn commit(summary: &str) -> CommitInfo {
        let oid = Oid::from_str("1a2b3c4d5e6f708192a3b4c5d6e7f80910111213").unwrap();
        CommitInfo {
            oid,
            summary: summary.into(),
            body: String::new(),
            author: String::new(),
            email: String::new(),
            time: 0,
            parents: Vec::new(),
        }
    }

    #[test]
    fn reset_title_names_the_branch() {
        assert_eq!(reset_title(Lang::En, Some("main")), "Reset main to this commit");
        assert_eq!(reset_title(Lang::En, None), "Reset to this commit");
        let de = reset_title(Lang::De, Some("main"));
        assert!(de.contains("main") && de != reset_title(Lang::En, Some("main")), "{de}");
    }

    #[test]
    fn commits_are_named_by_summary() {
        assert_eq!(commit_name(&commit("Fix build")), "\"Fix build\" (1a2b3c4)");
        assert_eq!(commit_name(&commit("")), "1a2b3c4");
        assert_eq!(commit_name(&commit("   ")), "1a2b3c4");
    }

    #[test]
    fn header_summary_is_cut_at_80_chars() {
        let eighty = "a".repeat(80);
        assert_eq!(header_summary(&eighty), eighty);
        let cut = header_summary(&"b".repeat(81));
        assert_eq!(cut, format!("{}…", "b".repeat(79)));
        let wide = header_summary(&"é".repeat(100));
        assert_eq!(wide.chars().count(), 80);
        assert_eq!(wide, format!("{}…", "é".repeat(79)));
        assert_eq!(header_summary("  Fix build  "), "Fix build");
    }

    #[test]
    fn hard_reset_body_says_what_it_keeps() {
        let body = hard_reset_body(Lang::En, Some("main"), &commit("Fix build"));
        assert!(body.starts_with("Reset main to \"Fix build\" (1a2b3c4)?"), "{body}");
        assert!(body.contains("stashed") && body.contains("Undo") && body.contains("Untracked files"), "{body}");
        assert!(hard_reset_body(Lang::En, None, &commit("Fix build")).starts_with("Reset HEAD to"));
        assert!(hard_reset_body(Lang::En, None, &commit("")).starts_with("Reset HEAD to 1a2b3c4?"));
        let want = "Reset main to \"Fix build\" (1a2b3c4)? Uncommitted changes to tracked files are stashed first, so Undo brings them back. Untracked files stay as they are.";
        assert_eq!(body, want);
    }

    #[test]
    fn hard_reset_confirms_unless_the_auto_stash_prompt_asks() {
        let c = commit("Fix build");
        let confirms = |snap: &Snapshot| matches!(hard_reset_msg(Lang::En, snap, &c), Msg::ConfirmDanger { .. });
        let mut snap = snapshot("hard-reset-confirm");
        assert!(confirms(&snap), "a clean tree confirms");
        snap.unstaged = rows(&[("a.txt", 'M')]);
        assert!(!confirms(&snap), "the auto-stash prompt asks instead");
        for state in [RepositoryState::Merge, RepositoryState::RebaseInteractive, RepositoryState::Bisect] {
            snap.state = state;
            assert!(confirms(&snap), "{state:?} with changes: no prompt, so the confirmation stays");
        }
        snap.state = RepositoryState::Clean;
        snap.unstaged = rows(&[("a.txt", '!')]);
        assert!(confirms(&snap), "conflicted files: no prompt, so the confirmation stays");
    }
}
