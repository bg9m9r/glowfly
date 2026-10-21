//! Messages of the issue panel and the Create issue dialog (state/issue.rs,
//! state/issue_form.rs, update/issue.rs, update/issue_create.rs, ui/issue/). The panel's
//! Create branch… sends `BranchMsg::NewBranch`, its Git Flow starts `GitFlowMsg::StartFromIssue`,
//! and the browser and copy actions are `Msg::OpenUrl` and `Msg::Copy`.

use crate::effect::IssueReply;
use glowfly_core::forge::Target;
use std::path::PathBuf;

#[derive(Debug)]
pub enum IssueMsg {
    /// Open the panel for the issue with this key (an ISSUES row's click or menu, the palette).
    Open(String),
    /// Close the panel (its Close button, Esc).
    Close,
    /// Load the open issue again (Try again).
    Reload,
    /// Open the Create issue dialog (the ISSUES header's + and menu, the palette).
    CreateDialog,
    /// The dialog's Jira project or Trello board was chosen: load its choices.
    SetProject(String),
    /// Create the issue the dialog describes.
    Create,
    /// A call's answer, for the tracker `target` of the repository at `workdir`.
    Replied { workdir: PathBuf, target: Target, generation: u64, reply: Box<IssueReply> },
}

impl IssueMsg {
    /// Nothing is kept for an inactive tab: the intents belong to the active tab, and Replied
    /// comes from an unscoped effect, so it never arrives scoped (its handler matches the
    /// repository itself and logs a created issue for another tab).
    pub fn on_inactive(self) -> Option<super::Inactive> {
        match self {
            Self::Open(_) | Self::Close | Self::Reload | Self::CreateDialog | Self::SetProject(_) | Self::Create | Self::Replied { .. } => {
                None
            }
        }
    }
}
