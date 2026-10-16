//! The rebase banner on the WIP row: its text (the step, the stopped commit and why, the
//! autostash note) and what its buttons send.

use crate::msg::{Msg, RebaseMsg};
use git2::RepositoryState;
use trellis_core::rebase::{RebaseProgress, Resume, StopKind};

/// Why Continue is unavailable while conflicted files remain.
pub const RESOLVE_FIRST: &str = "Resolve the conflicted files first.";
pub const STASH_NOTE: &str = "Your uncommitted changes are stashed until the rebase ends.";

/// The line naming the stopped commit: `before`, the short SHA, the summary, then `after`
/// (which brings its own leading space or punctuation).
#[derive(Clone, Debug, PartialEq)]
pub struct StopLine {
    pub before: &'static str,
    pub sha: String,
    pub summary: String,
    pub after: &'static str,
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
    /// "Interactive rebase in progress · step 2 of 4".
    pub title: String,
    /// `None` when the stopped commit is unknown (e.g. a rebase-apply).
    pub stop: Option<StopLine>,
    pub stash_note: bool,
}

/// The banner's text for `p`, with `name` the operation's name ("Interactive rebase" or
/// "Rebase") and `conflicts_left` whether conflicted files remain.
pub fn lines(p: &RebaseProgress, name: &str, conflicts_left: bool) -> BannerText {
    let title = if p.total == 0 { format!("{name} in progress") } else { format!("{name} in progress · step {} of {}", p.step, p.total) };
    let stop = p.stop.oid.map(|oid| {
        let (before, after) = match p.stop.kind {
            StopKind::Conflict if conflicts_left => ("Conflict while applying", ""),
            StopKind::Conflict => ("Conflicts in", " are resolved; Continue to go on."),
            StopKind::Edit => ("Paused to edit", ". Amend it or add commits, then Continue."),
            StopKind::Exec => ("Rewording", " failed."),
            StopKind::Other => ("Stopped at", ""),
        };
        StopLine { before, sha: oid.to_string()[..7].to_owned(), summary: p.summary.clone(), after }
    });
    BannerText { title, stop, stash_note: p.autostash }
}

/// What a banner button sends: a rebase's Continue and Skip run through
/// `trellis_core::rebase::resume`, which reports where the rebase stopped next; everything else
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
    use trellis_core::rebase::Stop;

    fn progress(kind: StopKind, total: usize) -> RebaseProgress {
        let oid = git2::Oid::from_bytes(&[0x1a; 20]).unwrap();
        RebaseProgress { step: 2, total, stop: Stop { kind, oid: Some(oid) }, summary: "Fix it".into(), autostash: true, interactive: true }
    }

    fn stop_text(kind: StopKind, conflicts_left: bool) -> String {
        lines(&progress(kind, 4), "Interactive rebase", conflicts_left).stop.unwrap().text()
    }

    #[test]
    fn lines_for_each_stop() {
        let b = lines(&progress(StopKind::Conflict, 4), "Interactive rebase", true);
        assert_eq!(b.title, "Interactive rebase in progress · step 2 of 4");
        assert!(b.stash_note);
        assert_eq!(stop_text(StopKind::Conflict, true), "Conflict while applying 1a1a1a1 Fix it");
        assert_eq!(stop_text(StopKind::Conflict, false), "Conflicts in 1a1a1a1 Fix it are resolved; Continue to go on.");
        assert_eq!(stop_text(StopKind::Edit, false), "Paused to edit 1a1a1a1 Fix it. Amend it or add commits, then Continue.");
        assert_eq!(stop_text(StopKind::Exec, false), "Rewording 1a1a1a1 Fix it failed.");
        assert_eq!(stop_text(StopKind::Other, false), "Stopped at 1a1a1a1 Fix it");

        let b = lines(&progress(StopKind::Other, 0), "Rebase", false);
        assert_eq!(b.title, "Rebase in progress", "no step without a count");
        let mut p = progress(StopKind::Conflict, 3);
        p.stop.oid = None;
        p.autostash = false;
        let b = lines(&p, "Rebase", true);
        assert_eq!((b.stop, b.stash_note), (None, false));
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
