//! Per-tab undo history: Trellis's own recent operations that can be reversed, most recent last.
//! In memory only, so it ends with the tab or the app.

use trellis_core::undo::Action;

/// Undo steps kept per tab; older ones are dropped.
pub const MAX_UNDO: usize = 20;

#[derive(Default, Debug)]
pub struct UndoHistory {
    pub undo: Vec<Action>,
    pub redo: Vec<Action>,
    /// Label of the most recent operation when it can't be undone (and cleared the history).
    pub blocked: Option<String>,
}

impl UndoHistory {
    /// A new undoable action succeeded.
    pub fn record(&mut self, a: Action) {
        self.push_undo(a);
        self.redo.clear();
        self.blocked = None;
    }

    /// An operation that can't be undone succeeded: older actions can't be undone past it.
    pub fn invalidate(&mut self, label: &str) {
        self.undo.clear();
        self.redo.clear();
        self.blocked = Some(label.to_owned());
    }

    /// The repository no longer matches the history.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// `a` (the top of the undo stack) was undone.
    pub fn undone(&mut self, a: Action) {
        if self.undo.last() == Some(&a) {
            self.undo.pop();
        }
        self.redo.push(a);
    }

    /// `original` (the top of the redo stack) was redone; `refreshed` is how it stands now.
    pub fn redone(&mut self, original: &Action, refreshed: Action) {
        if self.redo.last() == Some(original) {
            self.redo.pop();
        }
        self.push_undo(refreshed);
    }

    fn push_undo(&mut self, a: Action) {
        self.undo.push(a);
        if self.undo.len() > MAX_UNDO {
            let extra = self.undo.len() - MAX_UNDO;
            self.undo.drain(..extra);
        }
    }

    pub fn undo_tooltip(&self) -> String {
        match (self.undo.last(), &self.blocked) {
            (Some(a), _) => format!("Undo {} ({})", a.describe(), crate::keymap::hint(crate::keymap::BindingId::Undo)),
            (None, Some(label)) => {
                format!("Can't undo {label}: only checkout, commit, reset, branch delete, discards and file restores can be undone")
            }
            (None, None) => "Nothing to undo".to_owned(),
        }
    }

    pub fn redo_tooltip(&self) -> String {
        match self.redo.last() {
            Some(a) => format!("Redo {} ({})", a.describe(), crate::keymap::hint(crate::keymap::BindingId::Redo)),
            None => "Nothing to redo".to_owned(),
        }
    }
}
