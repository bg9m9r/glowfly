//! A specimen of the bundled fonts in a dark and a light theme: body text, the semibold weight,
//! monospace SHAs, paths and code, a sidebar row with its painted ahead/behind arrows, and a
//! character only egui's emoji fallback has. A second specimen shows the type scale: the five
//! steps, a section header with its count, prose with an inline monospace SHA, a link and a
//! caption.

use super::color::{self, Palette};
use super::{apply_style, egui_theme, fonts, type_scale};
use crate::ui::common::with_code;
use crate::ui::snapshot_check::CheckSnapshot;
use crate::ui::widgets::{self, Part};
use eframe::egui::{Label, Link, RichText, vec2};
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
        ui.label(RichText::new("crates/glowfly-app/src/ui/theme/fonts.rs").monospace().color(p.text));
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
    render("fonts_dark", super::GARDEN_DARK);
}

#[test]
fn fonts_light() {
    render("fonts_light", super::GARDEN_LIGHT);
}

fn render_scale(name: &str, p: Palette) {
    let mut h = Harness::builder().with_size(vec2(440.0, 330.0)).build_ui(move |ui| {
        color::set_palette(&p);
        apply_style(ui.ctx(), &p);
        ui.ctx().set_theme(egui_theme(&p));
        ui.painter().rect_filled(ui.max_rect(), 0.0, p.panel);
        ui.label(RichText::new("Preferences").font(type_scale::strong(ui, type_scale::TITLE)).color(p.text));
        ui.label(RichText::new("Fix the login redirect").font(type_scale::strong(ui, type_scale::SUBHEAD)).color(p.text));
        ui.label(RichText::new("Body: labels, buttons, menus and list rows").size(type_scale::BODY).color(p.text));
        ui.label(RichText::new("Small: help lines, hints and the 0/72 counter").size(type_scale::SMALL).color(p.weak));
        ui.label(RichText::new("Caption: badges and column titles").size(type_scale::CAPTION).color(p.weak));
        ui.add_space(4.0);
        let mut open = true;
        let _ = widgets::section_header(ui, "Local", Some(12), &mut open);
        ui.label(type_scale::section_text(ui, "Unstaged files (2)"));
        let job = with_code("Commit {sha} compared with the working directory", "{sha}", "3f2a9c1", type_scale::SMALL, p.weak);
        ui.add(Label::new(job).wrap());
        let job = with_code("Commit {sha}", "{sha}", "e0e76cb", type_scale::BODY, p.text);
        ui.add(Label::new(job));
        ui.add(Link::new("Open all"));
    });
    h.run();
    h.check_snapshot(name);
}

#[test]
fn type_scale_specimen_dark() {
    render_scale("type_scale_specimen_dark", super::GARDEN_DARK);
}

#[test]
fn type_scale_specimen_light() {
    render_scale("type_scale_specimen_light", super::GARDEN_LIGHT);
}
