//! Messages of the Init dialog and the init job it starts (update/init.rs).

use glowfly_core::init_ops::{FolderFacts, InitOutcome};

pub enum InitMsg {
    /// The folder text changed: check it on disk after a short pause.
    FolderEdited,
    /// What the folder text (trimmed, `folder`) names, or None when it names no usable folder.
    FolderChecked { folder: String, found: Option<FolderFacts> },
    /// git's user.name, to suggest as the license's copyright holder.
    AuthorRead(Option<String>),
    /// Create the repository (the dialog's request).
    Start(crate::state::init::InitRequest),
    /// The init job ended.
    Finished(InitOutcome),
}

impl InitMsg {
    /// What this result still needs when its tab is no longer active (`Msg::Scoped`); `None`
    /// drops it. The Init dialog's effects are unscoped, so their results always reach
    /// `update` and are never wrapped in `Msg::Scoped`.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::FolderEdited | Self::FolderChecked { .. } | Self::AuthorRead(..) | Self::Start(..) | Self::Finished(..) => None,
        }
    }
}
