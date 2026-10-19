//! The three button styles and their disabled look in a dark, a light and a high-contrast theme.

use super::snapshot_check::CheckSnapshot;
use super::theme::color::{self, Palette};
use super::theme::{self, apply_style, egui_theme};
use super::widgets::buttons;
use eframe::egui::vec2;
use egui_kittest::Harness;

fn render_buttons(name: &str, p: Palette) {
    let mut first = true;
    let mut h = Harness::builder().with_size(vec2(440.0, 150.0)).build_ui(move |ui| {
        color::set_palette(&p);
        apply_style(ui.ctx(), &p);
        ui.ctx().set_theme(egui_theme(&p));
        ui.painter().rect_filled(ui.max_rect(), 0.0, p.panel);
        ui.add_space(8.0);
        for enabled in [true, false] {
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.add(buttons::primary("Create branch").enabled(enabled));
                ui.add(buttons::secondary("Cancel").enabled(enabled));
                ui.add(buttons::danger("Delete branch").enabled(enabled));
            });
            ui.add_space(6.0);
        }
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            let r = ui.add(buttons::primary("Focused").min_width(76.0));
            if first {
                r.request_focus();
                first = false;
            }
            ui.add(buttons::secondary("Browse…").small());
        });
    });
    h.run();
    h.check_snapshot(name);
}

#[test]
fn button_styles() {
    render_buttons("button_styles_dark", theme::DARK);
    render_buttons("button_styles_light", theme::LIGHT);
    render_buttons("button_styles_hc_light", theme::HIGH_CONTRAST_LIGHT);
}
