//! The open dialog as it opened, to tell whether a form was edited, and whether the "Discard
//! your changes?" question is showing over it (`update/dialog.rs`).

use super::Modal;
use std::mem::Discriminant;

#[derive(Default)]
pub struct DialogGuard {
    /// The open dialog's variant and its `Modal::draft` when it opened.
    opened: Option<(Discriminant<Modal>, Option<String>)>,
    /// "Discard your changes?" shows over the open form. Only `update` sets it.
    pub asking: bool,
}

impl DialogGuard {
    /// Called after every update: records a dialog that just opened, and forgets a closed one
    /// (which drops the question with it).
    ///
    /// A dialog replaced by another of the same variant within one update keeps the first
    /// record. `update` never does that today: `push_pr` and the forge code avoid replacing an
    /// open Create pull request dialog.
    pub fn track(&mut self, modal: Option<&Modal>) {
        let Some(m) = modal else {
            *self = Self::default();
            return;
        };
        let d = std::mem::discriminant(m);
        if self.opened.as_ref().is_none_or(|(o, _)| *o != d) {
            self.opened = Some((d, m.draft()));
            self.asking = false;
        }
    }

    /// Whether `modal` differs from how it opened. A dialog with no record counts as unedited.
    pub fn edited(&self, modal: &Modal) -> bool {
        self.opened.as_ref().is_some_and(|(d, draft)| *d == std::mem::discriminant(modal) && *draft != modal.draft())
    }
}
