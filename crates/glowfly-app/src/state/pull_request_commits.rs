//! The pull request panel's Commits tab (ui/pull_request/commits.rs): its label, heading and
//! texts. Pure; the commits are loaded with the files (`PrView::shown`).

use super::pull_request::{Files, PrView};
use super::pull_request_source::Shown;
use glowfly_core::repo::RangeCommit;
use glowfly_core::text::printable;

/// "1 commit" or "3 commits".
pub fn commits_heading(n: usize) -> String {
    if n == 1 { "1 commit".to_owned() } else { format!("{n} commits") }
}

/// The line above a list cut short: "Showing the newest 500 of 742 commits."
pub fn more_text(shown: &Shown) -> Option<String> {
    let (n, more) = (shown.commits.commits.len(), shown.commits.more);
    (more > 0).then(|| format!("Showing the newest {n} of {} commits.", n + more))
}

/// The tab's label: "Commits", with the count once loaded ("Commits  3").
pub fn tab_label(v: &PrView) -> String {
    match (&v.files, &v.shown) {
        (Files::Loaded(_), Some(s)) => format!("Commits  {}", s.commits.commits.len() + s.commits.more),
        _ => "Commits".to_owned(),
    }
}

/// The empty state's text when the source brings no commits.
pub fn empty_text(v: &PrView) -> String {
    format!("{} has no commits that {} doesn't.", printable(&v.pr.source), printable(&v.pr.target))
}

/// A row's second line: the author ("Unknown" without a name), and " · merge" for a merge.
pub fn second_line(c: &RangeCommit) -> String {
    let author = if c.author.trim().is_empty() { "Unknown".to_owned() } else { printable(&c.author) };
    if c.merge { format!("{author} · merge") } else { author }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glowfly_core::repo::RangeCommits;

    fn commit(author: &str, merge: bool) -> RangeCommit {
        RangeCommit { oid: git2::Oid::zero(), summary: "s".into(), author: author.into(), email: String::new(), time: 0, merge }
    }

    #[test]
    fn texts() {
        assert_eq!(commits_heading(1), "1 commit");
        assert_eq!(commits_heading(3), "3 commits");
        assert_eq!(second_line(&commit("Ada", false)), "Ada");
        assert_eq!(second_line(&commit("", true)), "Unknown · merge");
        let source = glowfly_core::repo::SourceTip { oid: git2::Oid::zero(), from: glowfly_core::repo::SourceFrom::Branch };
        let shown = |n: usize, more| Shown { source, commits: RangeCommits { commits: vec![commit("a", false); n], more } };
        assert_eq!(more_text(&shown(2, 0)), None);
        assert_eq!(more_text(&shown(500, 242)).as_deref(), Some("Showing the newest 500 of 742 commits."));
    }
}
