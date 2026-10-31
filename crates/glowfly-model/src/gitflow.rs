//! Messages of the Git Flow preferences page, the Start and Finish dialogs, continuing a Finish
//! stopped at a merge conflict, the Git Flow panel and the left panel's GIT FLOW section (state/gitflow*.rs, update/gitflow*.rs,
//! ui/prefs/gitflow.rs, ui/gitflow_*.rs).

use glowfly_core::gitflow::FlowKind;

pub enum GitFlowMsg {
    /// Nav row: switch Preferences to the page and fill the form from the snapshot.
    ShowPage,
    /// Initialize, Create missing branches, or Save (the Save / Revert bar) with the form's
    /// values.
    Apply,
    /// Open the Start dialog (page button or palette).
    StartDialog(FlowKind),
    /// Create <prefix><name> from the kind's base and check it out.
    Start { kind: FlowKind, name: String },
    /// Open the Finish dialog for this local branch (palette entry or the page's Finish… button).
    FinishDialog(String),
    /// Merge, tag and delete as planned by core::gitflow::check_finish.
    Finish { branch: String, options: glowfly_core::gitflow::FinishOptions },
    /// Finish with the uncommitted changes stashed first and put back afterwards (Finish
    /// dialog).
    StashAndFinish { branch: String, options: glowfly_core::gitflow::FinishOptions },
    /// Show the Git Flow panel in place of the graph (section header menu, palette).
    OpenPanel,
    /// Close the panel (Close button, Esc).
    ClosePanel,
    /// Open the Start dialog for `kind` with `name` typed after the prefix (an issue's branch
    /// name; issue menu, palette).
    StartFromIssue { kind: FlowKind, name: String },
    /// Run the steps a Finish stopped at a conflict still has to do, once the merge is
    /// committed (window banner, panel row, palette).
    ContinueFinish,
    /// Drop the remembered steps of a stopped Finish (Forget).
    ForgetFinish,
}

impl GitFlowMsg {
    /// What this result still needs when its tab is no longer active (`Msg::Scoped`); `None`
    /// drops it.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::ShowPage
            | Self::Apply
            | Self::StartDialog(..)
            | Self::Start { .. }
            | Self::FinishDialog(..)
            | Self::Finish { .. }
            | Self::StashAndFinish { .. }
            | Self::OpenPanel
            | Self::ClosePanel
            | Self::StartFromIssue { .. }
            | Self::ContinueFinish
            | Self::ForgetFinish => None,
        }
    }
}
