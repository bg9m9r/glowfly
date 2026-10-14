//! The interactive rebase panel, shown in place of the graph: a header with Reset, Cancel and
//! Start rebase, why Start is unavailable, and the list of commits in the order git applies
//! them (rows in `rows`).

mod rows;

use super::UiCache;
use super::theme::color;
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::rebase::{HINT, RebasePanel};
use eframe::egui::{self, Align, Color32, Layout, RichText, Stroke, Ui};

pub fn show(ui: &mut Ui, st: &mut AppState, cache: &mut UiCache, msgs: &mut Vec<Msg>) {
    let Some(panel) = &st.rebase else { return };
    let blocker = st.snap.as_ref().and_then(|s| panel.start_blocker(s, st.job.is_some()));
    ui.add_space(8.0);
    header(ui, panel, blocker.as_deref(), msgs);
    ui.add_space(4.0);
    egui::Frame::new().inner_margin(egui::Margin { left: 14, right: 14, top: 0, bottom: 4 }).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        if panel.editable()
            && let Some(why) = &blocker
        {
            ui.label(RichText::new(why).color(color::red()));
        }
        if let Some(e) = panel.error.as_ref().filter(|e| blocker.as_ref() != Some(*e)) {
            ui.add(egui::Label::new(RichText::new(e).color(color::red())).wrap());
        }
        ui.label(RichText::new(HINT).size(12.0).color(color::weak()));
    });
    ui.separator();
    if panel.loading {
        ui.add_space(24.0);
        ui.vertical_centered(|ui| ui.add(egui::Spinner::new().size(22.0)));
        return;
    }
    rows::list(ui, st, cache, msgs);
}

/// Title and subtitle on the left; Reset, Cancel and Start rebase on the right.
fn header(ui: &mut Ui, panel: &RebasePanel, blocker: Option<&str>, msgs: &mut Vec<Msg>) {
    ui.horizontal(|ui| {
        ui.add_space(14.0);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.label(RichText::new("Interactive rebase").strong().size(16.0).color(color::text()));
            ui.label(RichText::new(panel.subtitle()).color(color::weak()));
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(14.0);
            let enabled = blocker.is_none();
            let (fg, fill) = if enabled { (Color32::WHITE, color::primary()) } else { (color::faint(), color::primary_off()) };
            let start = egui::Button::new(RichText::new("Start rebase").strong().color(fg))
                .fill(fill)
                .stroke(Stroke::new(1.0_f32, color::primary_border()))
                .min_size(egui::vec2(0.0, 28.0));
            let mut r = ui.add_enabled(enabled, start);
            if let Some(why) = blocker {
                r = r.on_disabled_hover_text(why);
            }
            if r.clicked() {
                msgs.push(Msg::StartRebase { confirmed: false });
            }
            let cancel = egui::Button::new("Cancel").min_size(egui::vec2(0.0, 28.0));
            if ui.add_enabled(!panel.starting, cancel).on_hover_text("Close without changing anything (Esc)").clicked() {
                msgs.push(Msg::CloseRebase);
            }
            let reset = egui::Button::new("Reset").min_size(egui::vec2(0.0, 28.0));
            let tip = if panel.stale { "Reload the list from the repository" } else { "Back to the original order and actions" };
            if ui.add_enabled(panel.editable(), reset).on_hover_text(tip).clicked() {
                msgs.push(Msg::RebaseReset);
            }
            if panel.starting {
                ui.add(egui::Spinner::new());
            }
        });
    });
}
