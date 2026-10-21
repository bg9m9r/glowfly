//! Previewing an operation before it runs (update/op_preview.rs): the Preview… menu items, the
//! result, Preview again and Apply.

use crate::effect::OpPreviewJob;
use glowfly_core::op_preview::{OpPreview, PreviewOp};

/// Which reset the preview's footer runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResetMode {
    Soft,
    Mixed,
    Hard,
}

#[derive(Clone, Debug, PartialEq)]
pub enum OpPreviewMsg {
    /// A Preview… menu item: open the dialog and work the result out.
    Open(PreviewOp),
    /// The runner worked out `job`.
    Loaded { job: OpPreviewJob, result: Result<Box<OpPreview>, String> },
    /// Preview again, against the branch as it is now.
    Again,
    /// Run the operation as its direct menu item does; `Some` only for a reset.
    Apply(Option<ResetMode>),
}

impl OpPreviewMsg {
    /// Nothing is kept for an inactive tab: the intents only make sense in the active one, and
    /// the dialog can't outlive a tab switch (it covers the tab bar), so a late Loaded is dropped.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::Open(_) | Self::Loaded { .. } | Self::Again | Self::Apply(_) => None,
        }
    }
}
