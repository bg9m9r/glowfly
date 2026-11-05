//! The commit draft in the WIP panel: its message, options and template, the rules for when
//! committing can start, the commit button's label and the git commands that commit it.

use super::AppState;
use stop::{EditStop, rebase_stopped};

#[path = "commit_ready.rs"]
mod ready;
#[path = "commit_stop.rs"]
pub mod stop;
use git2::RepositoryState;
use glowfly_core::commit_encoding::Unencodable;
use glowfly_core::gitcli::args;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::CommitTemplate;
pub use ready::{commit_blocked, commit_label, commit_merge_blocked};
use std::path::Path;

/// What the user is writing in the commit box. The view edits a copy of its fields
/// (`CommitFields`) and sends it back as `EditMsg::Commit`.
#[derive(Default)]
pub struct CommitDraft {
    pub summary: String,
    pub description: String,
    pub amend: bool,
    /// --no-verify for this commit; resets with the draft after a successful commit.
    pub skip_hooks: bool,
    /// The template the draft was prefilled with. Some => leave its comment lines out of the
    /// message and don't prefill again.
    pub template: Option<CommitTemplate>,
    /// The MERGE_MSG text the draft was filled with; Some => a merge's, cherry-pick's or
    /// revert's own message, cleared again when the operation ends while it is untouched.
    pub merge_msg: Option<String>,
    /// The commit a rebase stopped to edit for which Amend was ticked by default, so it is
    /// ticked once per stop and the person can untick it.
    pub amend_stop: Option<EditStop>,
}

/// The commit box's fields the view edits, drawn from a copy (`CommitDraft::fields`) and set
/// back by `EditMsg::Commit`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CommitFields {
    pub summary: String,
    pub description: String,
    pub amend: bool,
    pub skip_hooks: bool,
}

impl CommitDraft {
    /// A copy of the fields the commit box edits.
    pub fn fields(&self) -> CommitFields {
        let (summary, description) = (self.summary.clone(), self.description.clone());
        CommitFields { summary, description, amend: self.amend, skip_hooks: self.skip_hooks }
    }

    /// Takes the commit box's edited fields; the template, merge message and edit stop stay.
    pub fn apply(&mut self, f: CommitFields) {
        (self.summary, self.description, self.amend, self.skip_hooks) = (f.summary, f.description, f.amend, f.skip_hooks);
    }

    /// The full commit message: summary, blank line, description. The comment lines of the
    /// draft's template are left out; every line the user wrote is kept, including one that
    /// starts with the comment char (an issue mention such as "#123").
    pub fn message(&self) -> String {
        let description = match &self.template {
            Some(t) => without_template_comments(&self.description, t),
            None => self.description.clone(),
        };
        let mut m = self.summary.trim().to_owned();
        if !description.trim().is_empty() {
            m.push_str("\n\n");
            m.push_str(description.trim());
        }
        m
    }

    /// Fills the summary and description from a full commit message. The draft no longer
    /// holds the template afterwards.
    pub fn fill_from(&mut self, message: &str) {
        let (first, rest) = message.split_once('\n').unwrap_or((message, ""));
        self.summary = first.to_owned();
        self.description = rest.trim().to_owned();
        self.template = None;
    }

    pub fn is_empty(&self) -> bool {
        self.summary.trim().is_empty() && self.description.trim().is_empty()
    }

    /// Prefills the summary and description from `t` and remembers it.
    pub fn apply_template(&mut self, t: &CommitTemplate) {
        self.summary = t.summary.clone();
        self.description = t.description.clone();
        self.template = Some(t.clone());
    }

    /// Whether the draft still holds exactly the template it was prefilled with.
    pub fn is_untouched_template(&self) -> bool {
        self.template.as_ref().is_some_and(|t| self.summary.trim() == t.summary.trim() && self.description.trim() == t.description.trim())
    }

    /// Fills the draft from a merge's MERGE_MSG and remembers it.
    pub fn fill_merge(&mut self, m: &str) {
        self.fill_from(m);
        self.merge_msg = Some(m.to_owned());
    }

    /// Whether the draft still holds exactly the merge message it was filled with.
    pub fn is_untouched_merge_msg(&self) -> bool {
        self.merge_msg.as_ref().is_some_and(|m| {
            let mut filled = CommitDraft::default();
            filled.fill_from(m);
            filled.message() == self.message()
        })
    }

    /// Nothing the user wrote: empty, or only the untouched template or merge message.
    pub fn is_blank(&self) -> bool {
        self.is_empty() || self.is_untouched_template() || self.is_untouched_merge_msg()
    }

    /// The commit button's text: the base label with its slots for " and push" (when `push`)
    /// and " (skip hooks)" (when hooks are skipped), placed where the language puts them.
    /// "Commit merge" while `merging` (not for an amend). Plain "Commit", without the slots,
    /// while nothing is staged or the summary is empty (the caption says what is missing).
    pub fn button_label(&self, t: Lang, staged: usize, push: bool, merging: bool) -> String {
        let push_text = if push { t.tr("commit.button.and_push") } else { "" };
        let skip_text = if self.skip_hooks { t.tr("commit.button.skip_hooks") } else { "" };
        let slots: [(&str, &dyn std::fmt::Display); 2] = [("push", &push_text), ("skip", &skip_text)];
        if self.amend {
            t.trf("commit.button.amend", &slots)
        } else if merging {
            t.trf("commit.button.merge", &slots)
        } else if staged == 0 || self.summary.trim().is_empty() {
            t.tr("commit.button.plain").to_owned()
        } else {
            t.trn("commit.button.commit", staged, &slots)
        }
    }

    /// The commit button's text at a rebase stopped to edit: "Amend and continue", or with
    /// Amend unticked "Commit and continue".
    pub fn continue_label(&self, t: Lang) -> String {
        let skip_text = if self.skip_hooks { t.tr("commit.button.skip_hooks") } else { "" };
        let slots: [(&str, &dyn std::fmt::Display); 1] = [("skip", &skip_text)];
        if self.amend { t.trf("commit.button.amend_continue", &slots) } else { t.trf("commit.button.commit_continue", &slots) }
    }

    /// The git commands that commit this draft with its message in `file`: `add --all` first
    /// when `stage_all`, then `commit -F <file>` with the draft's options.
    pub fn commit_cmds(&self, file: &Path, stage_all: bool) -> Vec<Vec<String>> {
        let mut cmd = args(&["commit", "-F"]);
        cmd.push(file.display().to_string());
        if self.amend {
            cmd.push("--amend".into());
        }
        if self.skip_hooks {
            cmd.push("--no-verify".into());
        }
        // The template's comment lines are already gone from the message. Everything else must
        // stay even when commit.cleanup is "strip", which would drop a "#123" line.
        if self.template.is_some() {
            cmd.push("--cleanup=whitespace".into());
        }
        let mut cmds = Vec::new();
        if stage_all {
            cmds.push(args(&["add", "--all"]));
        }
        cmds.push(cmd);
        cmds
    }
}

/// `description` without the lines that are comment lines of `t` (compared without trailing
/// whitespace).
fn without_template_comments(description: &str, t: &CommitTemplate) -> String {
    let comment = |l: &str| l.starts_with(t.comment_char);
    let template: Vec<&str> = t.description.lines().filter(|l| comment(l)).map(str::trim_end).collect();
    description.lines().filter(|l| !(comment(l) && template.contains(&l.trim_end()))).collect::<Vec<_>>().join("\n")
}

/// Why Amend can't be used in `state`: a merge, cherry-pick or revert is stopped, and amending
/// would rewrite HEAD under it. None otherwise (a rebase stopped to edit a commit amends it).
pub fn amend_blocked(t: Lang, state: RepositoryState) -> Option<&'static str> {
    use RepositoryState::*;
    match state {
        Merge => Some(t.tr("commit.blocked.amend_merge")),
        CherryPick | CherryPickSequence => Some(t.tr("commit.blocked.amend_cherry_pick")),
        Revert | RevertSequence => Some(t.tr("commit.blocked.amend_revert")),
        _ => None,
    }
}

/// Why a message can't be written: the repository's commit encoding can't hold some characters.
pub fn unencodable_text(t: Lang, e: &Unencodable) -> String {
    t.trf("commit.blocked.unencodable", &[("encoding", &e.encoding), ("chars", &e.listed())])
}

/// Whether a commit now would be followed by a push (setting on, HEAD on a branch and no
/// rebase stopped: pushing halfway through one is never right).
pub fn pushes_after(st: &AppState) -> bool {
    st.settings.push_after_commit && st.snap.as_ref().is_some_and(|s| s.head_branch.is_some() && !rebase_stopped(s))
}

/// A summary's length up to here reads well (git-commit(1): ideally under 50 characters).
const SUMMARY_SOFT: usize = 50;
/// Past this, logs and hosting services cut the summary off.
pub const SUMMARY_HARD: usize = 72;

/// How a summary's length sits with the convention, for its counter's tone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SummaryLength {
    Fine,
    Long,
    TooLong,
}

/// The tone of a summary `len` characters long.
pub fn summary_length(len: usize) -> SummaryLength {
    if len <= SUMMARY_SOFT {
        SummaryLength::Fine
    } else if len <= SUMMARY_HARD {
        SummaryLength::Long
    } else {
        SummaryLength::TooLong
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_length_follows_the_convention() {
        assert_eq!(summary_length(0), SummaryLength::Fine);
        assert_eq!(summary_length(50), SummaryLength::Fine);
        assert_eq!(summary_length(51), SummaryLength::Long);
        assert_eq!(summary_length(72), SummaryLength::Long);
        assert_eq!(summary_length(73), SummaryLength::TooLong);
    }

    fn template() -> CommitTemplate {
        CommitTemplate { summary: "feat:".into(), description: "# why".into(), comment_char: '#' }
    }

    #[test]
    fn button_label_suffixes() {
        let en = Lang::En;
        let mut d = CommitDraft { summary: "Fix".into(), ..CommitDraft::default() };
        assert_eq!(d.button_label(en, 1, false, false), "Commit 1 file");
        assert_eq!(d.button_label(en, 0, true, false), "Commit", "nothing staged: plain, no suffixes");
        d.skip_hooks = true;
        assert_eq!(d.button_label(en, 2, true, false), "Commit 2 files and push (skip hooks)");
        d.skip_hooks = false;
        d.amend = true;
        assert_eq!(d.button_label(en, 0, true, false), "Amend last commit and push");
    }

    #[test]
    fn button_label_places_the_suffixes_where_the_language_does() {
        let de = Lang::De;
        let d = CommitDraft { skip_hooks: true, summary: "Fix".into(), ..CommitDraft::default() };
        let want = glowfly_core::i18n::fill(
            de.tr("commit.button.commit.other"),
            &[("count", &2), ("push", &de.tr("commit.button.and_push")), ("skip", &de.tr("commit.button.skip_hooks"))],
        );
        let got = d.button_label(de, 2, true, false);
        assert_eq!(got, want);
        assert!(got.contains(de.tr("commit.button.and_push").trim()) && got.contains(de.tr("commit.button.skip_hooks").trim()), "{got}");
        assert!(!got.contains('{'), "{got}");
        assert_ne!(got, d.button_label(Lang::En, 2, true, false));
    }

    #[test]
    fn commit_cmds_follow_the_options() {
        let f = Path::new("msg.txt");
        let file = f.display().to_string();
        let mut d = CommitDraft::default();
        assert_eq!(d.commit_cmds(f, false), vec![args(&["commit", "-F", &file])]);
        d.amend = true;
        d.skip_hooks = true;
        d.apply_template(&template());
        assert_eq!(
            d.commit_cmds(f, true),
            vec![args(&["add", "--all"]), args(&["commit", "-F", &file, "--amend", "--no-verify", "--cleanup=whitespace"])]
        );
    }

    #[test]
    fn message_drops_only_the_template_comments() {
        let mut d = CommitDraft::default();
        d.apply_template(&template());
        d.summary = "#42 Fix login".into();
        d.description = "# why  \nBecause.\n\n#123\n# my own note".into();
        assert_eq!(d.message(), "#42 Fix login\n\nBecause.\n\n#123\n# my own note");
        d.description = "# why\n\n#123".into();
        assert_eq!(d.message(), "#42 Fix login\n\n#123", "a mention added below the template is kept");
        d.description = "# why".into();
        assert_eq!(d.message(), "#42 Fix login");
        d.template = None;
        assert_eq!(d.message(), "#42 Fix login\n\n# why", "without a template nothing is dropped");
    }

    #[test]
    fn untouched_template_is_blank() {
        let mut d = CommitDraft::default();
        assert!(d.is_blank() && !d.is_untouched_template(), "empty");
        d.apply_template(&template());
        assert!(d.is_untouched_template() && d.is_blank(), "untouched");
        d.summary = "feat: login ".into();
        assert!(!d.is_untouched_template() && !d.is_blank(), "edited");
        d.summary.clear();
        d.description = "  ".into();
        assert!(!d.is_untouched_template() && d.is_blank(), "cleared");
        assert!(d.template.is_some(), "a cleared draft keeps its template, so it isn't prefilled again");
        d.fill_from("Message\n\nBody");
        assert!(d.template.is_none());
    }

    #[test]
    fn merge_label_and_untouched_merge_message() {
        let en = Lang::En;
        let mut d = CommitDraft::default();
        assert_eq!(d.button_label(en, 1, false, true), "Commit merge");
        assert_eq!(d.button_label(en, 0, false, true), "Commit merge", "a merge commits with nothing staged");
        d.skip_hooks = true;
        assert_eq!(d.button_label(en, 1, true, true), "Commit merge and push (skip hooks)");
        d.amend = true;
        assert_eq!(d.button_label(en, 1, false, true), "Amend last commit (skip hooks)", "amend wins");

        let mut d = CommitDraft::default();
        d.fill_merge(
            "Merge branch 'other'

Why it merges",
        );
        assert_eq!((d.summary.as_str(), d.description.as_str()), ("Merge branch 'other'", "Why it merges"));
        assert!(d.is_untouched_merge_msg() && d.is_blank(), "untouched");
        d.summary.push_str(" into main");
        assert!(!d.is_untouched_merge_msg() && !d.is_blank(), "edited");
    }
}
