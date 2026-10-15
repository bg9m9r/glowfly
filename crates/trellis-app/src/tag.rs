//! Messages of the New tag dialog and the TAGS section's remote actions (state/tags.rs, update/tags.rs,
//! ui/tags.rs, ui/tag_dialog.rs).

#[derive(Clone, Debug, PartialEq)]
pub enum TagMsg {
    /// Push `tag` to each of `remotes` in one job (one remote from "Push to X", every remote
    /// from "Push to all remotes").
    Push { tag: String, remotes: Vec<String> },
    /// Confirmed: delete `tag` from each of `remotes` in one job (git push <r> --delete
    /// refs/tags/<tag>).
    DeleteRemote { tag: String, remotes: Vec<String> },
    /// Opens the New tag dialog at commit `at` (a full sha); `annotated` makes the message required.
    NewTag { at: String, annotated: bool },
    /// Creates tag `name` at commit `at` (a full sha), annotated when `message` isn't empty,
    /// then pushes it to remote `push`.
    CreateTag { at: String, name: String, message: String, push: Option<String> },
    /// The New tag dialog's push checkbox, saved in settings.
    SetPushNewTags(bool),
}

impl TagMsg {
    /// What this result still needs when its tab is no longer active (`Msg::Scoped`); `None`
    /// drops it.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::Push { .. } | Self::DeleteRemote { .. } | Self::NewTag { .. } | Self::CreateTag { .. } | Self::SetPushNewTags(..) => None,
        }
    }
}
