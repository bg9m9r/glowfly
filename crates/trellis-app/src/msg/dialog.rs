//! Messages about the open dialog: a backdrop click, Esc or the close button in its header, and
//! the "Discard your changes?" question (update/dialog.rs).

use super::Inactive;

/// How the user tried to dismiss the open dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dismiss {
    /// A click on the dimmed backdrop around it.
    Backdrop,
    Escape,
    /// The close button in the dialog's header: the same as Esc.
    CloseButton,
}

/// The open dialog's dismissal and the discard question (update/dialog.rs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogMsg {
    /// A backdrop click, Esc or the close button: closes an information or confirm dialog; a
    /// form ignores the backdrop, and on Esc or the close button asks first when it was edited.
    Dismiss(Dismiss),
    /// The question's "Keep editing" (or Esc or a backdrop click on it): back to the form.
    KeepEditing,
    /// The question's "Discard": closes the form.
    Discard,
}

impl DialogMsg {
    /// What this message still needs when its tab is no longer active (`Msg::Scoped`): nothing,
    /// as it is a user intent and never comes back scoped.
    pub fn on_inactive(self) -> Option<Inactive> {
        match self {
            Self::Dismiss(_) | Self::KeepEditing | Self::Discard => None,
        }
    }
}
