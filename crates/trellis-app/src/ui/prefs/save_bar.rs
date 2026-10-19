//! The Save / Revert bar fixed under a Preferences page with unsaved edits (shown only while
//! `state::prefs_save::unsaved` holds), and the "Saved" tick beside the page title. The bar's
//! buttons push `PrefsMsg::SavePage` and `PrefsMsg::RevertPage`.

use super::primary_button;
use crate::msg::{Msg, PrefsMsg};
use crate::state::blocked::BUSY;
use crate::ui::theme::{color, type_scale};
use crate::ui::widgets::{self, Icon, buttons};
use eframe::egui::{self, Align, Layout, RichText, Sense, Stroke, Ui, vec2};
use trellis_core::i18n::Lang;

/// What the bar needs from outside the page.
pub struct BarView {
    /// A git job runs and the page's save would be one too: Save waits.
    pub busy: bool,
}

pub fn show(ctx: &egui::Context, t: Lang, view: BarView, msgs: &mut Vec<Msg>) {
    let frame = egui::Frame::new().fill(color::toolbar()).inner_margin(egui::Margin::symmetric(24, 10));
    let resp = egui::TopBottomPanel::bottom("prefs-save-bar").frame(frame).show_separator_line(false).show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(t.tr("prefs.save_bar.unsaved")).color(color::weak()));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let busy = view.busy;
                if primary_button(ui, t.tr("prefs.save_bar.save.button"), !busy, busy.then_some(BUSY)).clicked() {
                    msgs.push(Msg::Prefs(PrefsMsg::SavePage));
                }
                if ui.add(buttons::secondary(t.tr("prefs.save_bar.revert.button"))).clicked() {
                    msgs.push(Msg::Prefs(PrefsMsg::RevertPage));
                }
            });
        });
    });
    let r = resp.response.rect;
    ctx.layer_painter(resp.response.layer_id).hline(r.x_range(), r.top() + 0.5, Stroke::new(1.0_f32, color::border()));
}

/// A green tick and "Saved", at `alpha` of full strength (it fades out).
pub fn saved_tick(ui: &mut Ui, t: Lang, alpha: f32) {
    let col = color::added().gamma_multiply(alpha);
    let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
    // The fonts lack a check mark: painted.
    widgets::paint_icon(ui.painter(), r.center(), Icon::Check, col);
    ui.label(RichText::new(t.tr("prefs.saved")).size(type_scale::SMALL).color(col));
}
