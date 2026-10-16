//! Messages of Preferences > Keyboard (update/keys.rs).

use super::Inactive;
use crate::keymap::{BindingId, Chord, Slot};

/// Preferences > Keyboard: recording a key, settling a clash, removing and resetting keys.
pub enum KeysMsg {
    /// Record the next key press for `id`'s `slot`.
    Record {
        id: BindingId,
        slot: Slot,
    },
    CancelRecord,
    /// The key pressed while recording (`ui/shortcuts/record.rs`).
    Recorded(Chord),
    /// Settle the pending clash by giving the other binding the replaced key.
    Swap,
    /// Settle the pending clash by taking the key from the other binding.
    UseHere,
    /// Close the warning or notice under a row.
    DismissPending,
    /// Remove `id`'s key at `index`.
    Remove {
        id: BindingId,
        index: usize,
    },
    /// Put `id`'s keys back to their defaults.
    Reset(BindingId),
    AskResetAll,
    CancelResetAll,
    ResetAll,
}

impl KeysMsg {
    /// None of these are results of background work: nothing to keep for an inactive tab.
    pub fn on_inactive(self) -> Option<Inactive> {
        match self {
            Self::Record { .. }
            | Self::CancelRecord
            | Self::Recorded(_)
            | Self::Swap
            | Self::UseHere
            | Self::DismissPending
            | Self::Remove { .. }
            | Self::Reset(_)
            | Self::AskResetAll
            | Self::CancelResetAll
            | Self::ResetAll => None,
        }
    }
}
