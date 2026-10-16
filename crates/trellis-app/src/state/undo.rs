//! Per-tab undo history: Trellis's own recent operations that can be reversed, most recent last.
//! In memory only, so it ends with the tab or the app. Also the form of the dialog that asks
//! whether undoing a checkout should delete the branch it created (`Modal::UndoCreated`).

use trellis_core::undo::Action;

/// Undo steps kept per tab; older ones are dropped.
pub const MAX_UNDO: usize = 20;

/// What can be undone, for the reason Undo is unavailable after something else.
const UNDOABLE: &str = "only checkout, commit, reset, merge, pull, rebase, cherry-pick, revert, creating or deleting a branch or tag, stash, pop, apply, discards and file restores can be undone";

/// The dialog shown before undoing a checkout that created `branch`, which still points where
/// the checkout made it: switch back to `back_to`, and also delete the branch when `delete`.
#[derive(Clone, Debug, PartialEq)]
pub struct UndoCreatedForm {
    pub action: Action,
    pub branch: String,
    pub back_to: String,
    pub delete: bool,
}

impl UndoCreatedForm {
    pub fn new(action: Action, back_to: String) -> Self {
        let branch = match &action {
            Action::Checkout { created: Some(c), .. } => c.name.clone(),
            _ => String::new(),
        };
        Self { action, branch, back_to, delete: true }
    }
}

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

    pub fn undo_tooltip(&self, keys: &crate::keymap::Keymap) -> String {
        match self.undo.last() {
            Some(a) => keys.tip(&format!("Undo {}", a.describe()), crate::keymap::BindingId::Undo),
            None => self.undo_reason(),
        }
    }

    pub fn redo_tooltip(&self, keys: &crate::keymap::Keymap) -> String {
        match self.redo.last() {
            Some(a) => keys.tip(&format!("Redo {}", a.describe()), crate::keymap::BindingId::Redo),
            None => self.redo_reason(),
        }
    }

    /// Edit > Undo: its label ("Undo checkout of main", or "Undo" with nothing to undo) and why
    /// it is disabled, if it is: a job is running (`idle` false) or there is nothing to undo.
    pub fn undo_entry(&self, idle: bool) -> (String, Option<String>) {
        entry("Undo", self.undo.last(), idle, || self.undo_reason())
    }

    /// Edit > Redo, like `undo_entry`.
    pub fn redo_entry(&self, idle: bool) -> (String, Option<String>) {
        entry("Redo", self.redo.last(), idle, || self.redo_reason())
    }

    /// Why there is nothing to undo.
    fn undo_reason(&self) -> String {
        match &self.blocked {
            Some(label) => format!("Can't undo {label}: {UNDOABLE}"),
            None => "Nothing to undo".to_owned(),
        }
    }

    fn redo_reason(&self) -> String {
        "Nothing to redo".to_owned()
    }
}

fn entry(verb: &str, top: Option<&Action>, idle: bool, reason: impl FnOnce() -> String) -> (String, Option<String>) {
    let label = top.map_or_else(|| verb.to_owned(), |a| format!("{verb} {}", a.describe()));
    let blocked = if !idle {
        Some(super::rebase::JOB_RUNNING.to_owned())
    } else if top.is_none() {
        Some(reason())
    } else {
        None
    };
    (label, blocked)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::{BindingId, Keymap};
    use crate::state::rebase::JOB_RUNNING;
    use trellis_core::undo::HeadPos;

    fn checkout() -> Action {
        Action::Checkout { from: HeadPos::Branch("dev".into()), to: HeadPos::Branch("main".into()), created: None }
    }

    #[test]
    fn entries_name_the_action_or_say_why_not() {
        let mut h = UndoHistory::default();
        assert_eq!(h.undo_entry(true), ("Undo".to_owned(), Some("Nothing to undo".to_owned())));
        assert_eq!(h.redo_entry(true), ("Redo".to_owned(), Some("Nothing to redo".to_owned())));
        h.record(checkout());
        assert_eq!(h.undo_entry(true), ("Undo checkout of main".to_owned(), None));
        assert_eq!(h.undo_entry(false), ("Undo checkout of main".to_owned(), Some(JOB_RUNNING.to_owned())));
        h.undone(checkout());
        assert_eq!(h.redo_entry(true), ("Redo checkout of main".to_owned(), None));
        assert_eq!(h.redo_entry(false).1.as_deref(), Some(JOB_RUNNING));
        h.invalidate("Rename branch");
        let (label, why) = h.undo_entry(true);
        assert_eq!(label, "Undo");
        assert!(why.as_deref().is_some_and(|w| w.starts_with("Can't undo Rename branch: only checkout, commit")), "{why:?}");
        assert_eq!(why.as_deref(), Some(format!("Can't undo Rename branch: {UNDOABLE}").as_str()));
    }

    #[test]
    fn tooltips_are_unchanged() {
        let keys = Keymap::default();
        let mut h = UndoHistory::default();
        assert_eq!(h.undo_tooltip(&keys), "Nothing to undo");
        assert_eq!(h.redo_tooltip(&keys), "Nothing to redo");
        h.record(checkout());
        assert_eq!(h.undo_tooltip(&keys), keys.tip("Undo checkout of main", BindingId::Undo));
        h.invalidate("Rename branch");
        assert_eq!(h.undo_tooltip(&keys), format!("Can't undo Rename branch: {UNDOABLE}"));
        assert!(UNDOABLE.contains("merge, pull, rebase, cherry-pick, revert") && UNDOABLE.contains("stash, pop, apply"));
    }
}
