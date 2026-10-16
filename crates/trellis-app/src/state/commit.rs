//! The commit draft in the WIP panel: its message, options and template, the rules for when
//! committing can start, the commit button's label and the git commands that commit it.

use super::AppState;
use std::path::Path;
use trellis_core::gitcli::args;
use trellis_core::i18n::Lang;
use trellis_core::repo::CommitTemplate;

/// What the user is writing in the commit box. Views edit it in place.
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
}

impl CommitDraft {
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

    /// Nothing the user wrote: empty, or only the untouched template.
    pub fn is_blank(&self) -> bool {
        self.is_empty() || self.is_untouched_template()
    }

    /// The commit button's text: the base label with its slots for " and push" (when `push`)
    /// and " (skip hooks)" (when hooks are skipped), placed where the language puts them.
    pub fn button_label(&self, t: Lang, staged: usize, push: bool) -> String {
        let push_text = if push { t.tr("commit.button.and_push") } else { "" };
        let skip_text = if self.skip_hooks { t.tr("commit.button.skip_hooks") } else { "" };
        let slots: [(&str, &dyn std::fmt::Display); 2] = [("push", &push_text), ("skip", &skip_text)];
        if self.amend {
            t.trf("commit.button.amend", &slots)
        } else if staged == 0 {
            t.trf("commit.button.stage_first", &slots)
        } else {
            t.trn("commit.button.commit", staged, &slots)
        }
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

/// Why committing (or staging everything and committing) can't start now; None when it can.
pub fn commit_blocked(st: &AppState, stage_all: bool) -> Option<&'static str> {
    let t = st.settings.lang();
    let Some(snap) = &st.snap else { return Some(t.tr("commit.blocked.no_repo")) };
    let d = &st.draft;
    if st.job.is_some() {
        Some(t.tr("commit.blocked.job_running"))
    } else if d.is_untouched_template() {
        Some(t.tr("commit.blocked.untouched_template"))
    } else if d.summary.trim().is_empty() {
        Some(t.tr("commit.blocked.no_summary"))
    } else if stage_all && snap.unstaged.iter().any(|f| f.status == '!') {
        // Staging a conflicted file marks it resolved.
        Some(t.tr("commit.blocked.conflicts"))
    } else if !d.amend && !stage_all && snap.staged.is_empty() {
        Some(t.tr("commit.blocked.nothing_staged"))
    } else if !d.amend && stage_all && snap.staged.is_empty() && snap.unstaged.is_empty() {
        Some(t.tr("commit.blocked.no_changes"))
    } else {
        None
    }
}

/// Whether a commit now would be followed by a push (setting on and HEAD on a branch).
pub fn pushes_after(st: &AppState) -> bool {
    st.settings.push_after_commit && st.snap.as_ref().is_some_and(|s| s.head_branch.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn template() -> CommitTemplate {
        CommitTemplate { summary: "feat:".into(), description: "# why".into(), comment_char: '#' }
    }

    #[test]
    fn button_label_suffixes() {
        let en = Lang::En;
        let mut d = CommitDraft::default();
        assert_eq!(d.button_label(en, 1, false), "Commit changes to 1 file");
        assert_eq!(d.button_label(en, 0, false), "Stage files to commit");
        d.skip_hooks = true;
        assert_eq!(d.button_label(en, 2, true), "Commit changes to 2 files and push (skip hooks)");
        d.skip_hooks = false;
        d.amend = true;
        assert_eq!(d.button_label(en, 0, true), "Amend previous commit and push");
    }

    #[test]
    fn button_label_places_the_suffixes_where_the_language_does() {
        let de = Lang::De;
        let d = CommitDraft { skip_hooks: true, ..CommitDraft::default() };
        let want = trellis_core::i18n::fill(
            de.tr("commit.button.commit.other"),
            &[("count", &2), ("push", &de.tr("commit.button.and_push")), ("skip", &de.tr("commit.button.skip_hooks"))],
        );
        let got = d.button_label(de, 2, true);
        assert_eq!(got, want);
        assert!(got.contains(de.tr("commit.button.and_push").trim()) && got.contains(de.tr("commit.button.skip_hooks").trim()), "{got}");
        assert!(!got.contains('{'), "{got}");
        assert_ne!(got, d.button_label(Lang::En, 2, true));
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
}
