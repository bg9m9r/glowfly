//! The commit draft in the WIP panel: its message, options and template, the rules for when
//! committing can start, the commit button's label and the git commands that commit it.

use super::AppState;
use std::path::Path;
use trellis_core::gitcli::args;
use trellis_core::repo::CommitTemplate;

/// What the user is writing in the commit box. Views edit it in place.
#[derive(Default)]
pub struct CommitDraft {
    pub summary: String,
    pub description: String,
    pub amend: bool,
    /// --no-verify for this commit; resets with the draft after a successful commit.
    pub skip_hooks: bool,
    /// The template the draft was prefilled with. Some => commit with --cleanup=strip and
    /// don't prefill again.
    pub template: Option<CommitTemplate>,
}

impl CommitDraft {
    /// The full commit message: summary, blank line, description.
    pub fn message(&self) -> String {
        let mut m = self.summary.trim().to_owned();
        if !self.description.trim().is_empty() {
            m.push_str("\n\n");
            m.push_str(self.description.trim());
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

    /// The commit button's text: the base label, then " and push" when `push`, then
    /// " (skip hooks)" when hooks are skipped.
    pub fn button_label(&self, staged: usize, push: bool) -> String {
        let mut label = if self.amend {
            "Amend previous commit".to_owned()
        } else if staged == 0 {
            "Stage files to commit".to_owned()
        } else {
            format!("Commit changes to {staged} file{}", if staged == 1 { "" } else { "s" })
        };
        if push {
            label.push_str(" and push");
        }
        if self.skip_hooks {
            label.push_str(" (skip hooks)");
        }
        label
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
        // With -F git only trims whitespace by default, which would keep the comment lines.
        if self.template.is_some() {
            cmd.push("--cleanup=strip".into());
        }
        let mut cmds = Vec::new();
        if stage_all {
            cmds.push(args(&["add", "--all"]));
        }
        cmds.push(cmd);
        cmds
    }
}

/// Why committing (or staging everything and committing) can't start now; None when it can.
pub fn commit_blocked(st: &AppState, stage_all: bool) -> Option<&'static str> {
    let Some(snap) = &st.snap else { return Some("Open a repository first.") };
    let d = &st.draft;
    if st.job.is_some() {
        Some("Another operation is still running.")
    } else if d.is_untouched_template() {
        Some("Edit the commit template before committing.")
    } else if d.summary.trim().is_empty() {
        Some("Write a summary for the commit.")
    } else if stage_all && snap.unstaged.iter().any(|f| f.status == '!') {
        // Staging a conflicted file marks it resolved.
        Some("Resolve the conflicted files before staging everything.")
    } else if !d.amend && !stage_all && snap.staged.is_empty() {
        Some("Stage files to commit.")
    } else if !d.amend && stage_all && snap.staged.is_empty() && snap.unstaged.is_empty() {
        Some("There are no changes to commit.")
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
        let mut d = CommitDraft::default();
        assert_eq!(d.button_label(1, false), "Commit changes to 1 file");
        assert_eq!(d.button_label(0, false), "Stage files to commit");
        d.skip_hooks = true;
        assert_eq!(d.button_label(2, true), "Commit changes to 2 files and push (skip hooks)");
        d.skip_hooks = false;
        d.amend = true;
        assert_eq!(d.button_label(0, true), "Amend previous commit and push");
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
            vec![args(&["add", "--all"]), args(&["commit", "-F", &file, "--amend", "--no-verify", "--cleanup=strip"])]
        );
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
