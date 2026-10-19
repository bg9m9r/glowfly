//! A specimen of the bundled fonts in a dark and a light theme: body text, the semibold weight,
//! monospace SHAs, paths and code, a sidebar row with its painted ahead/behind arrows, and a
//! character only egui's emoji fallback has.

use super::color::{self, Palette};
use super::{apply_style, egui_theme, fonts};
use crate::ui::snapshot_check::CheckSnapshot;
use crate::ui::widgets::{self, Part};
use eframe::egui::{RichText, vec2};
use egui_kittest::Harness;

fn render(name: &str, p: Palette) {
    let mut h = Harness::builder().with_size(vec2(440.0, 290.0)).build_ui(move |ui| {
        color::set_palette(&p);
        apply_style(ui.ctx(), &p);
        ui.ctx().set_theme(egui_theme(&p));
        ui.painter().rect_filled(ui.max_rect(), 0.0, p.panel);
        ui.label(RichText::new("Body: Fetch, pull and push your branches.").color(p.text));
        ui.label(RichText::new("Semibold: the checked-out branch").font(fonts::semibold(ui, 13.0)).color(p.strong));
        ui.label(RichText::new("3f2a9c1  e0e76cb5a1d4").monospace().color(p.weak));
        ui.label(RichText::new("crates/trellis-app/src/ui/theme/fonts.rs").monospace().color(p.text));
        ui.label(RichText::new("fn main() { let x = 0x1f; } // 0O 1lI").monospace().color(p.text));
        ui.label(RichText::new("\u{2714} main (emoji fallback)").color(p.text));
        ui.add_space(4.0);
        let parts = [Part::Ahead(2), Part::Behind(13)];
        let _ = widgets::list_row_cut(ui, widgets::ROW_INDENT, "main", Some((&parts[..], p.weak)), 0.0, false, true);
        let _ = widgets::list_row_cut(ui, widgets::ROW_INDENT, "feature/login", Some((&[Part::Ahead(1)][..], p.weak)), 0.0, true, false);
    });
    h.run();
    h.check_snapshot(name);
}

#[test]
fn fonts_dark() {
    render("fonts_dark", super::DARK);
}

#[test]
fn fonts_light() {
    render("fonts_light", super::LIGHT);
}
