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
