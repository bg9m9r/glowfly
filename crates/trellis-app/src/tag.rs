//! Messages of the New tag and Annotate dialogs and the TAGS section's remote actions
//! (state/tags.rs, update/tags.rs, update/tag_annotate.rs, ui/tags.rs, ui/tag_dialog.rs).

#[derive(Clone, Debug, PartialEq)]
pub enum TagMsg {
    /// Push `tag` to each of `remotes` in one job (one remote from the tag menu's Push to
    /// submenu, or every remote from its "All remotes").
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
    /// From the TAGS row menu: opens the Annotate dialog for lightweight tag `tag`, at the commit
    /// the snapshot has for it.
    OpenAnnotate { tag: String },
    /// From the Annotate dialog: replaces `tag` with an annotated tag at `at` (a full sha)
    /// carrying `message`.
    Annotate { tag: String, at: String, message: String },
    /// Checks out tag `tag` detached (`git checkout refs/tags/<tag>`), from the TAGS menu, the
    /// graph's tag labels and the palette.
    Checkout { tag: String },
}

impl TagMsg {
    /// What this result still needs when its tab is no longer active (`Msg::Scoped`); `None`
    /// drops it.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::Push { .. }
            | Self::DeleteRemote { .. }
            | Self::NewTag { .. }
            | Self::CreateTag { .. }
            | Self::SetPushNewTags(..)
            | Self::OpenAnnotate { .. }
            | Self::Annotate { .. }
            | Self::Checkout { .. } => None,
        }
    }
}
