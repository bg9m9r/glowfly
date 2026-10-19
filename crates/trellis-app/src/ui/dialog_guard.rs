//! How a dialog was dismissed (a backdrop click or Esc), and the "Discard your changes?"
//! question stacked over an edited form (update/dialog.rs decides what each one does).

use super::theme::color;
use crate::msg::{DialogMsg, Dismiss, Msg};
use eframe::egui::{self, Id, Key, Modifiers, RichText};
use trellis_core::i18n::Lang;

/// A backdrop click, or Esc while `resp`'s modal is the top one and no popup (a combo box's
/// list) is open; Esc is consumed only then. The same test as egui's `should_close`.
pub(super) fn dismissal<T>(resp: &egui::ModalResponse<T>) -> Option<Dismiss> {
    if resp.backdrop_response.clicked() {
        return Some(Dismiss::Backdrop);
    }
    let ctx = &resp.response.ctx;
    let esc = resp.is_top_modal && !resp.any_popup_open && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
    esc.then_some(Dismiss::Escape)
}

/// The question over an edited form that Esc tried to close. Drawn after the form's modal, so
/// it is the top one: the form beneath is dimmed and takes no input. Esc and a backdrop click
/// mean Keep editing.
pub(super) fn discard_question(ctx: &egui::Context, t: Lang, msgs: &mut Vec<Msg>) {
    let resp = super::theme::modal(ctx, Id::new("trellis-modal-discard")).show(ctx, |ui| {
        ui.set_width(360.0);
        ui.heading(t.tr("dialog.discard.title"));
        ui.add_space(6.0);
        ui.label(RichText::new(t.tr("dialog.discard.body")).color(color::weak()));
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if ui.button(t.tr("dialog.discard.keep.button")).clicked() {
                msgs.push(Msg::Dialog(DialogMsg::KeepEditing));
            }
            if ui.button(t.tr("dialog.discard.discard.button")).clicked() {
                msgs.push(Msg::Dialog(DialogMsg::Discard));
            }
        });
    });
    if dismissal(&resp).is_some() {
        msgs.push(Msg::Dialog(DialogMsg::KeepEditing));
    }
}
