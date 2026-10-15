//! Messages of the REMOTE section and the Add/Edit remote dialog (state/remotes.rs,
//! update/remotes.rs, ui/remotes/).

#[derive(Clone, Debug, PartialEq)]
pub enum RemoteMsg {
    /// Open the Add remote dialog.
    AddDialog,
    /// Open the Edit remote dialog for this remote.
    EditDialog(String),
    /// The dialog's Add remote / Save: reads the form from `st.modal`.
    Submit,
    /// `git fetch --prune <remote>`.
    Fetch(String),
    /// Ask before removing this remote (opens Modal::Confirm).
    RemoveDialog(String),
    /// Confirmed: `git remote remove <remote>`.
    Remove(String),
}

impl RemoteMsg {
    /// What this result still needs when its tab is no longer active (`Msg::Scoped`); `None`
    /// drops it.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::AddDialog | Self::EditDialog(..) | Self::Submit | Self::Fetch(..) | Self::RemoveDialog(..) | Self::Remove(..) => None,
        }
    }
}
