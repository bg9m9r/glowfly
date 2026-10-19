//! The Git Flow preferences page (its form, filled from the snapshot's gitflow config), the
//! Start and Finish dialogs' forms, the running Finish job, and the texts and checks of the Git
//! Flow panel and the left panel's GIT FLOW section.

use super::AppState;
use std::path::PathBuf;
use trellis_core::gitflow::{
    Branches, FinishOptions, FlowKind, GitFlowConfig, GitFlowSettings, StartProblem, default_tag, suggested, targets,
};
use trellis_core::repo::Snapshot;

/// Why Git Flow actions are refused in a repository without the config.
pub const NOT_SET_UP: &str = "Git Flow isn't set up in this repository. Set it up in Preferences > Git Flow.";

/// The job label of a first Initialize.
pub const INIT_LABEL: &str = "Initialize Git Flow";
/// The job label of a Save once initialized.
pub const SAVE_LABEL: &str = "Save Git Flow settings";

#[derive(Default)]
pub struct GitFlowPage {
    /// The fields (view text buffers).
    pub form: GitFlowSettings,
    /// What the form was filled with, for Discard and "edited".
    pub filled: GitFlowSettings,
    /// The config the form was filled from.
    pub base: GitFlowConfig,
    /// The repository the form belongs to.
    pub workdir: Option<PathBuf>,
    /// Why the last Apply was refused; cleared when a field changes.
    pub error: Option<String>,
    /// The Finish row's chosen branch (view state, untouched by `fill`).
    pub finish_branch: String,
}

impl GitFlowPage {
    /// Fills the form from `snap`: the configured values, else the suggested defaults.
    pub fn fill(&mut self, snap: &Snapshot) {
        self.base = snap.gitflow.clone();
        self.form = self.base.settings().unwrap_or_else(|| suggested(&self.base, &Branches::of(snap)));
        self.filled = self.form.clone();
        self.workdir = Some(snap.workdir.clone());
        self.error = None;
    }

    pub fn edited(&self) -> bool {
        self.form != self.filled
    }

    /// Initialized, and the form's prefixes differ from the saved ones.
    pub fn prefixes_changed(&self) -> bool {
        let Some(saved) = self.base.settings() else { return false };
        let form = self.form.trimmed();
        (&form.feature, &form.release, &form.hotfix, &form.version_tag)
            != (&saved.feature, &saved.release, &saved.hotfix, &saved.version_tag)
    }
}

/// The Start dialog: which kind of branch and the name typed after its prefix.
pub struct StartForm {
    pub kind: FlowKind,
    pub name: String,
    /// Whether the name was edited or Start tried, so its problem waits for them.
    pub touched: super::touched::Touched,
}

/// The Finish dialog: the branch, the tag and message (release and hotfix) and Keep.
pub struct FinishForm {
    pub kind: FlowKind,
    pub branch: String,
    pub tag: String,
    pub message: String,
    pub keep: bool,
    /// Focus the Tag field on the next frame.
    pub focus_tag: bool,
    /// Whether the tag was edited or Finish tried, so its problem waits for them.
    pub touched: super::touched::Touched,
}

impl FinishForm {
    pub fn options(&self) -> FinishOptions {
        FinishOptions { tag: self.tag.clone(), message: self.message.clone(), keep: self.keep }
    }
}

/// A running Finish job: its label (to match `st.job`), the plan's step texts and the success
/// summary, for the headline when it ends.
pub struct FinishJob {
    pub label: String,
    pub steps: Vec<String>,
    pub done: String,
}

/// "Features", "Releases" or "Hotfixes".
pub fn group_title(kind: FlowKind) -> &'static str {
    match kind {
        FlowKind::Feature => "Features",
        FlowKind::Release => "Releases",
        FlowKind::Hotfix => "Hotfixes",
    }
}

/// Why a `kind` branch can't be started now: Git Flow isn't set up, or its base branch is
/// missing locally. None when Start can open.
pub fn start_blocked(snap: &Snapshot, kind: FlowKind) -> Option<String> {
    let Some(s) = snap.gitflow.settings() else { return Some(NOT_SET_UP.to_owned()) };
    let base = s.base(kind);
    if Branches::of(snap).local(base) {
        return None;
    }
    Some(StartProblem::MissingBase { kind, base: base.to_owned() }.message())
}

/// A group with no branches: "No open features. Start one from develop."
pub fn empty_text(s: &GitFlowSettings, kind: FlowKind) -> String {
    format!("No open {}. Start one from {}.", group_title(kind).to_lowercase(), s.base(kind))
}

/// A flow branch row's hover text: the full name, where it started and what Finish does.
pub fn row_tooltip(s: &GitFlowSettings, kind: FlowKind, branch: &str) -> String {
    let tag = default_tag(s, kind, branch).map(|t| format!(" and tags it {t}")).unwrap_or_default();
    format!(
        "{branch}
Started from {}. Finish merges it into {}{tag}.",
        s.base(kind),
        targets(s, kind)
    )
}

/// Whether the palette can offer starting a flow branch from an issue: Git Flow is set up and
/// the tracker has issues assigned to the user.
pub fn issues_ready(st: &AppState) -> bool {
    st.snap.as_ref().is_some_and(|s| s.gitflow.is_initialized())
        && st.forge.as_ref().is_some_and(|f| f.tracker.is_some() && !f.my_issues.is_empty())
}
