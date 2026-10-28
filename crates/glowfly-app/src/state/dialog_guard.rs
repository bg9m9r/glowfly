//! The open dialog as it opened, to tell whether a form was edited, whether the "Discard your
//! changes?" question is showing over it (`update/dialog.rs`), and an edited form kept
//! underneath a question that opened over it (the quit question, a tab close's confirmation, the
//! trust prompt: `open_over`), which comes back once that question closes (`restore`).

use super::Modal;
use super::dialog_kind::DialogKind;
use std::mem::Discriminant;

/// A dialog's variant and its `Modal::draft` when it opened.
type Opened = (Discriminant<Modal>, Option<String>);

#[derive(Default)]
pub struct DialogGuard {
    /// The open dialog's variant and its draft when it opened.
    opened: Option<Opened>,
    /// "Discard your changes?" shows over the open form. Only `update` sets it.
    pub asking: bool,
    /// An edited form under the open question, with how it opened.
    under: Option<Box<(Modal, Opened)>>,
}

impl DialogGuard {
    /// Called after every update: records a dialog that just opened, and forgets a closed one
    /// (which drops the question with it). A form kept underneath stays.
    ///
    /// A dialog replaced by another of the same variant within one update keeps the first
    /// record. `update` never does that today: `push_pr` and the forge code avoid replacing an
    /// open Create pull request dialog.
    pub fn track(&mut self, modal: Option<&Modal>) {
        let Some(m) = modal else {
            self.closed();
            return;
        };
        let d = std::mem::discriminant(m);
        if self.opened.as_ref().is_none_or(|(o, _)| *o != d) {
            self.opened = Some((d, m.draft()));
            self.asking = false;
        }
    }

    /// The open dialog closed: its record and question go, a form kept underneath stays.
    pub fn closed(&mut self) {
        self.opened = None;
        self.asking = false;
    }

    /// Whether `modal` differs from how it opened. A dialog with no record counts as unedited.
    pub fn edited(&self, modal: &Modal) -> bool {
        self.opened.as_ref().is_some_and(|(d, draft)| *d == std::mem::discriminant(modal) && *draft != modal.draft())
    }

    /// The edited form kept under the open question, if any.
    pub fn under(&self) -> Option<&Modal> {
        self.under.as_deref().map(|(m, _)| m)
    }

    /// Takes the form underneath out to draw it beneath the question (`put_under` puts it back).
    pub fn take_under(&mut self) -> Option<(Modal, Opened)> {
        self.under.take().map(|b| *b)
    }

    /// Puts back what `take_under` took.
    pub fn put_under(&mut self, under: (Modal, Opened)) {
        self.under = Some(Box::new(under));
    }

    /// Forgets the form underneath (its tab closed).
    pub fn drop_under(&mut self) {
        self.under = None;
    }
}

/// Whether `modal` is a form the user edited since it opened.
pub fn edited_form(guard: &DialogGuard, modal: Option<&Modal>) -> bool {
    modal.is_some_and(|m| m.kind() == DialogKind::Form && guard.edited(m))
}

/// Whether an edited form is open, on top or kept under a question.
pub fn holds_edited_form(guard: &DialogGuard, modal: Option<&Modal>) -> bool {
    guard.under.is_some() || edited_form(guard, modal)
}

/// Opens `question` in the dialog's place. An edited form there isn't lost: it is kept
/// underneath (drawn beneath the question) and comes back once the question closes; any other
/// dialog is replaced, as before.
pub fn open_over(modal: &mut Option<Modal>, guard: &mut DialogGuard, question: Modal) {
    if guard.under.is_none()
        && edited_form(guard, modal.as_ref())
        && let (Some(form), Some(opened)) = (modal.take(), guard.opened.take())
    {
        guard.under = Some(Box::new((form, opened)));
        guard.asking = false;
    }
    *modal = Some(question);
}

/// After every update: once no dialog is open, the form kept underneath comes back as it was
/// (still counting as edited).
pub fn restore(modal: &mut Option<Modal>, guard: &mut DialogGuard) {
    if modal.is_some() {
        return;
    }
    if let Some((form, opened)) = guard.take_under() {
        *modal = Some(form);
        guard.opened = Some(opened);
        guard.asking = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::tabs::{RenameTabForm, Tab};

    fn rename_form() -> Modal {
        Modal::RenameTab(RenameTabForm::new(&Tab::unloaded(glowfly_core::paths::temp_dir().join("glowfly-guard"))))
    }

    fn edit(m: &mut Option<Modal>) {
        if let Some(Modal::RenameTab(f)) = m {
            f.text = "edited".into();
        }
    }

    #[test]
    fn an_edited_form_waits_under_a_question_and_comes_back() {
        let (mut modal, mut guard) = (Some(rename_form()), DialogGuard::default());
        guard.track(modal.as_ref());
        edit(&mut modal);
        open_over(&mut modal, &mut guard, Modal::About);
        assert!(matches!(modal, Some(Modal::About)));
        assert!(matches!(guard.under(), Some(Modal::RenameTab(f)) if f.text == "edited"));
        guard.track(modal.as_ref());
        assert!(holds_edited_form(&guard, modal.as_ref()));
        // The question closes: the form is back and still edited.
        modal = None;
        guard.track(modal.as_ref());
        restore(&mut modal, &mut guard);
        guard.track(modal.as_ref());
        assert!(matches!(&modal, Some(Modal::RenameTab(f)) if f.text == "edited"));
        assert!(guard.under().is_none());
        assert!(edited_form(&guard, modal.as_ref()));
    }

    #[test]
    fn an_unedited_dialog_is_replaced() {
        let (mut modal, mut guard) = (Some(rename_form()), DialogGuard::default());
        guard.track(modal.as_ref());
        open_over(&mut modal, &mut guard, Modal::About);
        assert!(guard.under().is_none());
        assert!(!holds_edited_form(&guard, modal.as_ref()));
        modal = None;
        restore(&mut modal, &mut guard);
        assert!(modal.is_none());
    }
}
