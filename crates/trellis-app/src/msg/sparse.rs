//! Messages of the Sparse Checkout preferences page (state/sparse.rs, update/sparse.rs,
//! ui/prefs/sparse.rs).

use trellis_core::sparse_ops::SparseStatus;

pub enum SparseMsg {
    /// The navigation row: switch Preferences to the page and read the status.
    ShowPage,
    /// Read the status again ("Check again").
    Reload,
    /// Turn on or Save (the Save / Revert bar) with the form's mode and text (checked in
    /// update).
    Apply,
    /// `git sparse-checkout reapply`.
    Reapply,
    /// `git sparse-checkout disable`.
    Disable,
    // ---- from the runner
    Loaded {
        generation: u64,
        result: Result<SparseStatus, String>,
    },
    /// A sparse job ended (unscoped: it ends the active job).
    Done {
        ok: bool,
        output: String,
    },
}

impl SparseMsg {
    /// What this result still needs when its tab is no longer active (`Msg::Scoped`); `None`
    /// drops it (a status for another tab means nothing here; `Done` is never scoped).
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::ShowPage | Self::Reload | Self::Apply | Self::Reapply | Self::Disable | Self::Loaded { .. } | Self::Done { .. } => None,
        }
    }
}
