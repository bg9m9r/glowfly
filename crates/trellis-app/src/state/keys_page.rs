//! Preferences > Keyboard page state: the filter, the key being recorded, and the warning or
//! notice under a row. The keys themselves live in `AppState::keymap`.

use crate::keymap::{BindingId, Chord, Slot};

#[derive(Default)]
pub struct KeysPage {
    pub filter: String,
    /// The next key press goes to this action's slot.
    pub recording: Option<Recording>,
    /// What the row of `Pending::id` shows under it.
    pub pending: Option<Pending>,
    /// Reset all asked for and waiting for its confirmation.
    pub confirm_reset_all: bool,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Recording {
    pub id: BindingId,
    pub slot: Slot,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Pending {
    /// A key the action can't have, and why.
    Refused { id: BindingId, text: String },
    /// `chord` belongs to `other`. A fixed binding keeps it; a rebindable one can swap (getting
    /// `swap`, when that is valid for it) or give it up.
    Clash { id: BindingId, slot: Slot, chord: Chord, other: BindingId, fixed: bool, swap: Option<Chord> },
    /// What a reset took from other bindings.
    Notice { id: BindingId, text: String },
}

impl Pending {
    /// The row it belongs under.
    pub fn id(&self) -> BindingId {
        match self {
            Pending::Refused { id, .. } | Pending::Clash { id, .. } | Pending::Notice { id, .. } => *id,
        }
    }
}
