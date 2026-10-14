//! The Git Flow preferences page (its form, filled from the snapshot's gitflow config), the
//! Start and Finish dialogs' forms, and the running Finish job.

use std::path::PathBuf;
use trellis_core::gitflow::{Branches, FinishOptions, FlowKind, GitFlowConfig, GitFlowSettings, suggested};
use trellis_core::repo::Snapshot;

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
