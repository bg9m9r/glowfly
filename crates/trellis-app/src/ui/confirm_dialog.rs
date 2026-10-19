//! The confirmation dialog (`Modal::Confirm`), and its danger form for steps that lose work
//! (`Modal::ConfirmDanger`): the danger-filled button at the right edge, Cancel apart to its left.

use crate::msg::Msg;
use crate::ui::widgets::buttons;
use eframe::egui::{self, Align, Layout, Ui};
use trellis_core::i18n::Lang;

/// Draws the dialog; returns true when it should close (Cancel).
pub(super) fn show(ui: &mut Ui, t: Lang, title: &str, body: &str, button: &str, danger: bool, msgs: &mut Vec<Msg>) -> bool {
    let mut close = false;
    ui.heading(title);
    ui.add_space(6.0);
    if !danger {
        ui.label(body);
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if ui.button(button).clicked() {
                msgs.push(Msg::ConfirmAccepted);
            }
            if ui.button("Cancel").clicked() {
                close = true;
            }
        });
        return close;
    }
    ui.add(egui::Label::new(body).wrap());
    ui.add_space(14.0);
    ui.horizontal(|ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.add(buttons::danger(button)).clicked() {
                msgs.push(Msg::ConfirmAccepted);
            }
            ui.add_space(12.0);
            if ui.button(t.tr("common.cancel.button")).clicked() {
                close = true;
            }
        });
    });
    close
}
