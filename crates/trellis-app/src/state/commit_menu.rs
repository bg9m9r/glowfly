//! The commit menus' wording: the reset submenu's title and modes, the header's summary, and a
//! commit named by its summary in confirmations.

use git2::Oid;
use trellis_core::repo::CommitInfo;

pub const RESET_SOFT: &str = "Soft: keep changes staged";
pub const RESET_MIXED: &str = "Mixed: keep files, unstage";
pub const RESET_HARD: &str = "Hard: reset files too…";
/// The weak line under the Hard item.
pub const RESET_HARD_NOTE: &str = "Changes are stashed and Undo restores them";

/// The most characters of a summary the menu's header shows.
const HEADER_CHARS: usize = 80;

fn short(oid: Oid) -> String {
    oid.to_string()[..7].to_owned()
}

/// The reset submenu's title: "Reset main to this commit", or "Reset to this commit" when
/// HEAD is detached.
pub fn reset_title(current: Option<&str>) -> String {
    match current {
        Some(b) => format!("Reset {b} to this commit"),
        None => "Reset to this commit".into(),
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
pub fn hard_reset_body(current: Option<&str>, c: &CommitInfo) -> String {
    format!(
        "Reset {} to {}? Uncommitted changes to tracked files are stashed first, so Undo brings them back. Untracked files stay as they are.",
        current.unwrap_or("HEAD"),
        commit_name(c)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(reset_title(Some("main")), "Reset main to this commit");
        assert_eq!(reset_title(None), "Reset to this commit");
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
        let body = hard_reset_body(Some("main"), &commit("Fix build"));
        assert!(body.starts_with("Reset main to \"Fix build\" (1a2b3c4)?"), "{body}");
        assert!(body.contains("stashed") && body.contains("Undo") && body.contains("Untracked files"), "{body}");
        assert!(hard_reset_body(None, &commit("Fix build")).starts_with("Reset HEAD to"));
        assert!(hard_reset_body(None, &commit("")).starts_with("Reset HEAD to 1a2b3c4?"));
    }
}
