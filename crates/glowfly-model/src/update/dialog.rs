//! Dismissing the open dialog: a backdrop click, Esc or the close button in its header closes an
//! information or confirm dialog. A form ignores the backdrop, and Esc or the close button
//! closes it only while it still matches how it opened (`DialogGuard`); otherwise it asks
//! "Discard your changes?" first. Closing a Fork dialog opened by Push to your fork… goes back
//! to the create dialog with what was typed (update/fork_pr.rs).

use super::{deep_link, trust};
use crate::effect::Effect;
use crate::msg::{DialogMsg, Dismiss};
use crate::state::dialog_kind::DialogKind;
use crate::state::{AppState, Modal};

pub(super) fn handle(st: &mut AppState, msg: DialogMsg, fx: &mut Vec<Effect>) {
    match msg {
        DialogMsg::Dismiss(how) => dismiss(st, how, fx),
        DialogMsg::KeepEditing => st.dialog.asking = false,
        DialogMsg::Discard => {
            if st.modal.is_some() {
                super::stack_prs::dismissed(st);
                close(st, fx);
            }
        }
    }
}

fn dismiss(st: &mut AppState, how: Dismiss, fx: &mut Vec<Effect>) {
    // Under a credential prompt, Esc and clicks belong to the prompt.
    let Some(modal) = st.modal.as_ref() else { return };
    if st.askpass.showing() {
        return;
    }
    match (modal.kind(), how) {
        (DialogKind::Info | DialogKind::Confirm, _) => close(st, fx),
        (DialogKind::Form, Dismiss::Backdrop) => {}
        // Esc on the question means Keep editing, so pressing Esc twice never loses text.
        (DialogKind::Form, Dismiss::Escape | Dismiss::CloseButton) if st.dialog.asking => st.dialog.asking = false,
        (DialogKind::Form, Dismiss::Escape | Dismiss::CloseButton) => {
            if st.dialog.edited(modal) {
                st.dialog.asking = true;
            } else {
                super::stack_prs::dismissed(st);
                close(st, fx);
            }
        }
    }
}

/// Closes whichever dialog is open (`Msg::CloseModal`, the question's Discard).
pub(super) fn close(st: &mut AppState, fx: &mut Vec<Effect>) {
    // Closing the trust prompt without a decision drops the job waiting for it.
    if matches!(st.modal, Some(Modal::Trust(_))) {
        trust::dismissed(st);
    }
    if matches!(st.modal, Some(Modal::DeepLink(_))) {
        deep_link::modal_closed(st);
    }
    let back = super::fork_pr::taken_on_close(st);
    st.modal = None;
    // A form kept under a question comes back after this update (`dialog_guard::restore`).
    st.dialog.closed();
    // Cancelling Push to your fork… goes back to the create dialog with what was typed.
    if let Some(then) = back {
        super::fork_pr::reopen(st, then, fx);
    }
}
