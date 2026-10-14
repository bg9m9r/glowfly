//! Messages of the Git Flow preferences page and the Start and Finish dialogs (state/gitflow.rs,
//! update/gitflow.rs, update/gitflow_finish.rs, ui/prefs/gitflow.rs, ui/gitflow_dialog.rs,
//! ui/gitflow_finish_dialog.rs).

use trellis_core::gitflow::FlowKind;

pub enum GitFlowMsg {
    /// Nav row: switch Preferences to the page and fill the form from the snapshot.
    ShowPage,
    /// Initialize or Save with the form's values.
    Apply,
    /// Put the form back to what it was filled with.
    Discard,
    /// Open the Start dialog (page button or palette).
    StartDialog(FlowKind),
    /// Create <prefix><name> from the kind's base and check it out.
    Start { kind: FlowKind, name: String },
    /// Open the Finish dialog for this local branch (palette entry or the page's Finish… button).
    FinishDialog(String),
    /// Merge, tag and delete as planned by core::gitflow::check_finish.
    Finish { branch: String, options: trellis_core::gitflow::FinishOptions },
}
