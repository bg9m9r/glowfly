//! How a dialog was dismissed (a backdrop click or Esc), and the "Discard your changes?"
//! question stacked over an edited form (update/dialog.rs decides what each one does).

use super::dialog_frame::{self, Cx, Dialog, Footer, Size};
use super::theme::color;
use super::widgets::buttons;
use crate::msg::{DialogMsg, Dismiss, Msg};
use eframe::egui::{self, Id, Key, Modifiers, RichText};
use glowfly_core::i18n::Lang;

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
/// it is the top one: the form beneath is dimmed and takes no input. Esc, its close button and a
/// backdrop click mean Keep editing, and so does Enter: Keep editing has the focus when it opens.
pub(super) fn discard_question(ctx: &egui::Context, t: Lang, msgs: &mut Vec<Msg>) {
    let id = Id::new("glowfly-modal-discard");
    // It takes the keyboard over the form beneath.
    let cx = Cx { t, keyboard: true, key: id };
    let resp = super::theme::show_modal(ctx, super::theme::dialog(ctx, id), |ui| {
        let d = Dialog::new(cx, t.tr("dialog.discard.title")).size(Size::Small).begin(ui);
        ui.label(RichText::new(t.tr("dialog.discard.body")).color(color::weak()));
        let footer = Footer::new()
            .alt(buttons::secondary(t.tr("dialog.discard.discard.button")))
            .primary(buttons::primary(t.tr("dialog.discard.keep.button")));
        let out = d.finish(ui, footer);
        if out.primary {
            msgs.push(Msg::Dialog(DialogMsg::KeepEditing));
        }
        if out.alt == Some(0) {
            msgs.push(Msg::Dialog(DialogMsg::Discard));
        }
    });
    // Its close button, like Esc, means Keep editing.
    if dialog_frame::take_closed(&resp) || dismissal(&resp).is_some() {
        msgs.push(Msg::Dialog(DialogMsg::KeepEditing));
    }
}
