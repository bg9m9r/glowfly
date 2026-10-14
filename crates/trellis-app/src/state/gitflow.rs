//! The Git Flow preferences page (its form, filled from the snapshot's gitflow config) and the
//! Start dialog's form.

use std::path::PathBuf;
use trellis_core::gitflow::{Branches, FlowKind, GitFlowConfig, GitFlowSettings, suggested};
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
