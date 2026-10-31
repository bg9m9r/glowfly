//! The rebase banner on the WIP row: its text (the step, the stopped commit and why, a failed
//! command, the autostash note) and what its buttons send.

use crate::msg::{Msg, RebaseMsg};
use git2::Oid;
use git2::RepositoryState;
use glowfly_core::i18n::Lang;
use glowfly_core::rebase::{RebaseProgress, Resume, StopKind};

/// The line naming the stopped commit: `before`, the short SHA, the summary, then `after`
/// (which brings its own leading space or punctuation). The SHA links to `oid`.
#[derive(Clone, Debug, PartialEq)]
pub struct StopLine {
    pub before: String,
    pub oid: Oid,
    pub sha: String,
    pub summary: String,
    pub after: String,
}

impl StopLine {
    /// The whole line as plain text.
    #[cfg(test)]
    pub fn text(&self) -> String {
        let summary = if self.summary.is_empty() { String::new() } else { format!(" {}", self.summary) };
        format!("{} {}{summary}{}", self.before, self.sha, self.after)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BannerText {
    /// `None` when the stopped commit is unknown (e.g. a rebase-apply).
    pub stop: Option<StopLine>,
    /// The command that failed, for a stop at an exec step.
    pub command: Option<String>,
    pub stash_note: bool,
}

/// The banner's text for `p` (its title is `op_actions::banner_title`), with `conflicts_left`
/// whether conflicted files remain.
pub fn lines(t: Lang, p: &RebaseProgress, conflicts_left: bool) -> BannerText {
    let stop = p.stop.oid.map(|oid| {
        let template = match p.stop.kind {
            StopKind::Conflict if conflicts_left => t.tr("details.banner.stop.conflict_applying"),
            StopKind::Conflict => t.tr("details.banner.stop.conflict_resolved"),
            StopKind::Edit => t.tr("details.banner.stop.edit"),
            StopKind::Exec => t.tr("details.banner.stop.exec"),
            StopKind::Command => t.tr("details.banner.stop.command"),
            StopKind::Break => t.tr("details.banner.stop.break"),
            StopKind::Other => t.tr("details.banner.stop.stopped"),
        };
        let (before, after) = split_at_commit(template);
        StopLine { before, oid, sha: oid.to_string()[..7].to_owned(), summary: p.summary.clone(), after }
    });
    let command = p.command.clone().filter(|_| p.stop.kind == StopKind::Command);
    BannerText { stop, command, stash_note: p.autostash }
}

/// A stop line's template split around its first "{commit}" (where the SHA link and summary
/// go): the part before without its trailing space, and the rest. Without the placeholder,
/// all of it comes before.
fn split_at_commit(template: &str) -> (String, String) {
    match template.split_once("{commit}") {
        Some((before, after)) => (before.trim_end().to_owned(), after.to_owned()),
        None => (template.to_owned(), String::new()),
    }
}

/// What a banner button sends: a rebase's Continue and Skip run through
/// `glowfly_core::rebase::resume`, which reports where the rebase stopped next; everything else
/// is the plain git command.
pub fn action_msg(state: RepositoryState, label: &str, args: &[&str]) -> Msg {
    use RepositoryState::*;
    let rebasing = matches!(state, Rebase | RebaseInteractive | RebaseMerge | ApplyMailboxOrRebase);
    match args {
        ["rebase", "--continue"] if rebasing => Msg::Rebase(RebaseMsg::Resume(Resume::Continue)),
        ["rebase", "--skip"] if rebasing => Msg::Rebase(RebaseMsg::Resume(Resume::Skip)),
        _ => Msg::git(label, args),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glowfly_core::rebase::Stop;

    fn progress(kind: StopKind, total: usize) -> RebaseProgress {
        let oid = git2::Oid::from_bytes(&[0x1a; 20]).unwrap();
        let command = (kind == StopKind::Command).then(|| "cargo test".to_owned());
        RebaseProgress {
            step: 2,
            total,
            stop: Stop { kind, oid: Some(oid) },
            summary: "Fix it".into(),
            autostash: true,
            interactive: true,
            command,
        }
    }

    fn stop_text(kind: StopKind, conflicts_left: bool) -> String {
        lines(Lang::En, &progress(kind, 4), conflicts_left).stop.unwrap().text()
    }

    #[test]
    fn lines_for_each_stop() {
        let b = lines(Lang::En, &progress(StopKind::Conflict, 4), true);
        assert!(b.stash_note);
        assert_eq!(stop_text(StopKind::Conflict, true), "Conflict while applying 1a1a1a1 Fix it");
        assert_eq!(stop_text(StopKind::Conflict, false), "Conflicts in 1a1a1a1 Fix it are resolved; Continue to go on.");
        assert_eq!(stop_text(StopKind::Edit, false), "Paused to edit 1a1a1a1 Fix it. Amend it or add commits, then Continue.");
        assert_eq!(stop_text(StopKind::Exec, false), "Rewording 1a1a1a1 Fix it failed.");
        assert_eq!(stop_text(StopKind::Other, false), "Stopped at 1a1a1a1 Fix it");
        assert_eq!(stop_text(StopKind::Command, false), "The command after 1a1a1a1 Fix it failed. Fix the problem, then Continue.");
        assert_eq!(stop_text(StopKind::Break, false), "Paused after 1a1a1a1 Fix it at a break step. Continue when you're ready.");
        let b = lines(Lang::En, &progress(StopKind::Command, 4), false);
        assert_eq!(b.command.as_deref(), Some("cargo test"));
        assert_eq!(b.stop.unwrap().oid, git2::Oid::from_bytes(&[0x1a; 20]).unwrap(), "the SHA links to the commit");
        assert_eq!(lines(Lang::En, &progress(StopKind::Break, 4), false).command, None);

        let mut p = progress(StopKind::Conflict, 3);
        p.stop.oid = None;
        p.autostash = false;
        let b = lines(Lang::En, &p, true);
        assert_eq!((b.stop, b.stash_note), (None, false));
    }

    #[test]
    fn stop_lines_follow_the_language() {
        for kind in [StopKind::Conflict, StopKind::Edit, StopKind::Exec, StopKind::Command, StopKind::Break, StopKind::Other] {
            let de = lines(Lang::De, &progress(kind, 4), false).stop.unwrap();
            assert_eq!(de.sha, "1a1a1a1", "the SHA stays a link of its own");
            assert_ne!(de.text(), lines(Lang::En, &progress(kind, 4), false).stop.unwrap().text(), "{kind:?}");
            assert!(!de.before.is_empty() && !de.before.contains("{commit}") && !de.after.contains("{commit}"), "{de:?}");
        }
        let edit = lines(Lang::De, &progress(StopKind::Edit, 4), false).stop.unwrap();
        assert!(edit.text().contains("1a1a1a1 Fix it"), "{}", edit.text());
    }

    #[test]
    fn a_template_without_the_placeholder_is_all_before() {
        assert_eq!(split_at_commit("Stopped"), ("Stopped".to_owned(), String::new()));
        assert_eq!(split_at_commit("Paused at {commit}."), ("Paused at".to_owned(), ".".to_owned()));
    }

    #[test]
    fn continue_and_skip_resume() {
        let s = RepositoryState::RebaseInteractive;
        assert!(matches!(action_msg(s, "Continue", &["rebase", "--continue"]), Msg::Rebase(RebaseMsg::Resume(Resume::Continue))));
        assert!(matches!(action_msg(s, "Skip commit", &["rebase", "--skip"]), Msg::Rebase(RebaseMsg::Resume(Resume::Skip))));
        assert!(matches!(action_msg(s, "Abort rebase", &["rebase", "--abort"]), Msg::Job(_)));
        let c = RepositoryState::CherryPick;
        assert!(matches!(action_msg(c, "Continue", &["cherry-pick", "--continue"]), Msg::Job(_)));
    }
}
