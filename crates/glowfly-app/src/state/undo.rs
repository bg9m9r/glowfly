//! Per-tab undo history: Glowfly's own recent operations that can be reversed, most recent last.
//! In memory only, so it ends with the tab or the app. Also the form of the dialog that asks
//! whether undoing a checkout should delete the branch it created (`Modal::UndoCreated`).

use glowfly_core::i18n::Lang;
use glowfly_core::undo::Action;

/// Undo steps kept per tab; older ones are dropped.
pub const MAX_UNDO: usize = 20;

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

    /// Edit > Undo: its label ("Undo checkout of main", or "Undo" with nothing to undo) and why
    /// it is disabled, if it is: a job is running (`idle` false) or there is nothing to undo.
    pub fn undo_entry(&self, t: Lang, idle: bool) -> (String, Option<String>) {
        entry(t, false, self.undo.last(), idle, || self.undo_reason(t))
    }

    /// Edit > Redo, like `undo_entry`.
    pub fn redo_entry(&self, t: Lang, idle: bool) -> (String, Option<String>) {
        entry(t, true, self.redo.last(), idle, || self.redo_reason(t))
    }

    /// Why there is nothing to undo.
    fn undo_reason(&self, t: Lang) -> String {
        match &self.blocked {
            Some(label) => t.trf("menu.edit.cant_undo", &[("label", label)]),
            None => t.tr("menu.edit.nothing_to_undo").to_owned(),
        }
    }

    fn redo_reason(&self, t: Lang) -> String {
        t.tr("menu.edit.nothing_to_redo").to_owned()
    }
}

/// An Undo (`redo` false) or Redo entry for `top`, in `t`.
fn entry(t: Lang, redo: bool, top: Option<&Action>, idle: bool, reason: impl FnOnce() -> String) -> (String, Option<String>) {
    let label = match top {
        Some(a) if redo => t.trf("menu.edit.redo_action", &[("action", &a.describe())]),
        Some(a) => t.trf("menu.edit.undo_action", &[("action", &a.describe())]),
        None if redo => t.tr("menu.edit.redo").to_owned(),
        None => t.tr("menu.edit.undo").to_owned(),
    };
    let blocked = if !idle {
        Some(t.tr("commit.blocked.job_running").to_owned())
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
    use crate::state::rebase::JOB_RUNNING;
    use glowfly_core::undo::HeadPos;

    fn checkout() -> Action {
        Action::Checkout { from: HeadPos::Branch("dev".into()), to: HeadPos::Branch("main".into()), created: None }
    }

    #[test]
    fn entries_name_the_action_or_say_why_not() {
        let mut h = UndoHistory::default();
        assert_eq!(h.undo_entry(Lang::En, true), ("Undo".to_owned(), Some("Nothing to undo".to_owned())));
        assert_eq!(h.redo_entry(Lang::En, true), ("Redo".to_owned(), Some("Nothing to redo".to_owned())));
        h.record(checkout());
        assert_eq!(h.undo_entry(Lang::En, true), ("Undo checkout of main".to_owned(), None));
        assert_eq!(h.undo_entry(Lang::En, false), ("Undo checkout of main".to_owned(), Some(JOB_RUNNING.to_owned())));
        h.undone(checkout());
        assert_eq!(h.redo_entry(Lang::En, true), ("Redo checkout of main".to_owned(), None));
        assert_eq!(h.redo_entry(Lang::En, false).1.as_deref(), Some(JOB_RUNNING));
        h.invalidate("Rename branch");
        let (label, why) = h.undo_entry(Lang::En, true);
        assert_eq!(label, "Undo");
        assert!(why.as_deref().is_some_and(|w| w.starts_with("Can't undo Rename branch: only checkout, commit")), "{why:?}");
        assert_eq!(why, Some(Lang::En.trf("menu.edit.cant_undo", &[("label", &"Rename branch")])));
    }

    #[test]
    fn tooltips_are_unchanged() {
        // The toolbar's tooltips take their title and reason from the entries.
        let mut h = UndoHistory::default();
        assert_eq!(h.undo_entry(Lang::En, true).1.as_deref(), Some("Nothing to undo"));
        assert_eq!(h.redo_entry(Lang::En, true).1.as_deref(), Some("Nothing to redo"));
        h.record(checkout());
        assert_eq!(h.undo_entry(Lang::En, true), ("Undo checkout of main".to_owned(), None));
        h.invalidate("Rename branch");
        let tip = h.undo_entry(Lang::En, true).1.unwrap();
        assert_eq!(tip, Lang::En.trf("menu.edit.cant_undo", &[("label", &"Rename branch")]));
        assert!(tip.starts_with("Can't undo Rename branch: only checkout, commit"), "{tip}");
        assert!(tip.contains("merge, pull, rebase, cherry-pick, revert") && tip.contains("stash, pop, apply, drop"));
        assert!(Lang::En.tr("menu.edit.cant_undo").contains("drop"));
    }

    #[test]
    fn entries_follow_the_language() {
        let mut h = UndoHistory::default();
        h.record(checkout());
        let (label, _) = h.undo_entry(Lang::De, true);
        assert_eq!(label, Lang::De.trf("menu.edit.undo_action", &[("action", &"checkout of main")]));
        assert_ne!(label, "Undo checkout of main");
        assert_eq!(h.undo_entry(Lang::Es, false).1, Some(Lang::Es.tr("commit.blocked.job_running").to_owned()));
    }
}
