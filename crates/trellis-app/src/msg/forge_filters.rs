//! Messages of pull request and issue filters: the section headers' pickers and the saved-filter
//! dialog (update/forge_filters.rs, update/forge_filter_edit.rs). Their results come back as
//! `ForgeMsg::ForgeReplied`.

use trellis_core::forge::{FilterChoice, FilterKind};

pub enum FilterMsg {
    /// Show `choice` in `kind`'s section (saved for the repository).
    Choose { kind: FilterKind, choice: FilterChoice },
    /// Open the filter dialog for a new filter, starting from the one on show.
    NewDialog(FilterKind),
    /// Open the filter dialog on a saved filter.
    EditDialog(u64),
    /// Save the dialog's filter (`Modal::ForgeFilter`).
    Save,
    /// Remove a saved filter (after the confirmation).
    Remove(u64),
    /// Move a saved filter one place up or down among those of its kind.
    Move { id: u64, up: bool },
}

impl FilterMsg {
    /// What this result still needs when its tab is no longer active (`Msg::Scoped`); `None`
    /// drops it. Every variant is a user intent.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::Choose { .. } | Self::NewDialog(..) | Self::EditDialog(..) | Self::Save | Self::Remove(..) | Self::Move { .. } => None,
        }
    }
}
