//! Messages of the Git Flow preferences page and the Start dialog (state/gitflow.rs,
//! update/gitflow.rs, ui/prefs/gitflow.rs, ui/gitflow_dialog.rs).

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
}
