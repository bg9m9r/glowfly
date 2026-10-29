//! A specimen of the bundled fonts in a dark and a light theme: body text, the semibold weight,
//! monospace SHAs, paths and code, a sidebar row with its painted ahead/behind arrows, and a
//! character only egui's emoji fallback has. A second specimen shows the type scale: the five
//! steps, a section header with its count, prose with an inline monospace SHA, a link and a
//! caption. A third shows Chinese, Japanese and Korean author names, commit subjects, branch
//! names, paths and dates, drawn from the CJK fallback (in tests the bundled subset).

use super::color::{self, Palette};
use super::{apply_style, egui_theme, fonts, type_scale};
use crate::ui::common::with_code;
use crate::ui::snapshot_check::CheckSnapshot;
use crate::ui::widgets::{self, Part};
use eframe::egui::{FontFamily, FontId, Label, Link, RichText, vec2};

fn render(name: &str, p: Palette) {
    let mut h = crate::ui::snapshot_check::builder().with_size(vec2(440.0, 290.0)).build_ui(move |ui| {
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
    let mut h = crate::ui::snapshot_check::builder().with_size(vec2(440.0, 330.0)).build_ui(move |ui| {
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

/// Author names with their commit subjects, branch rows, paths and a date in each CJK locale.
#[test]
fn fonts_cjk() {
    let p = super::GARDEN_DARK;
    // 2026-10-13 18:30 UTC, a Tuesday afternoon.
    let date = |locale: &str| glowfly_core::dates::format(1_791_916_200, "YYYY/MM/DD dddd A h:mm", locale, 0);
    let dates: Vec<String> = ["ja_JP", "zh_CN", "ko_KR"].map(date).to_vec();
    let commits = [
        ("\u{7530}\u{4E2D}\u{592A}\u{90CE}", "\u{8A2D}\u{5B9A}\u{753B}\u{9762}\u{3092}\u{4FEE}\u{6B63}"),
        ("\u{738B}\u{5C0F}\u{660E}", "\u{4FEE}\u{590D}\u{767B}\u{5F55}\u{9875}\u{9762}"),
        ("\u{AE40}\u{BBFC}\u{C900}", "\u{AC80}\u{C0C9} \u{AE30}\u{B2A5} \u{CD94}\u{AC00}"),
    ];
    let paths = [
        "docs/\u{65E5}\u{672C}\u{8A9E}/\u{8A2D}\u{5B9A}.md",
        "\u{8D44}\u{6E90}/\u{56FE}\u{6807}.png",
        "src/\u{D55C}\u{AD6D}\u{C5B4}/\u{BAA8}\u{B4C8}.rs",
    ];
    let branches = ["\u{6A5F}\u{80FD}/\u{691C}\u{7D22}", "\u{AE30}\u{B2A5}/\u{AC80}\u{C0C9}"];
    let mut shown: Vec<String> = dates.clone();
    shown.extend(commits.iter().flat_map(|(a, s)| [a.to_string(), s.to_string()]));
    shown.extend(paths.iter().chain(&branches).map(|s| s.to_string()));
    let mut h = crate::ui::snapshot_check::builder().with_size(vec2(440.0, 330.0)).build_ui(move |ui| {
        color::set_palette(&p);
        apply_style(ui.ctx(), &p);
        ui.ctx().set_theme(egui_theme(&p));
        ui.painter().rect_filled(ui.max_rect(), 0.0, p.panel);
        for (author, subject) in commits {
            ui.horizontal(|ui| {
                ui.label(RichText::new(author).font(fonts::semibold(ui, 13.0)).color(p.strong));
                ui.label(RichText::new(subject).color(p.text));
            });
        }
        ui.add_space(4.0);
        for (i, name) in branches.iter().enumerate() {
            let _ = widgets::list_row_cut(ui, widgets::ROW_INDENT, name, Some((&[Part::Ahead(1)][..], p.weak)), 0.0, i == 0, false);
        }
        for path in paths {
            ui.label(RichText::new(path).monospace().color(p.text));
        }
        for d in &dates {
            ui.label(RichText::new(d).color(p.weak));
        }
    });
    h.run();
    h.check_snapshot("fonts_cjk");
    // After the snapshot: asking for glyphs adds them to the font atlas, which the frame
    // already drawn wouldn't match.
    let fonts = [FontId::proportional(13.0), FontId::monospace(13.0), FontId::new(13.0, FontFamily::Name(fonts::SEMIBOLD.into()))];
    for text in &shown {
        for c in text.chars().filter(|c| !c.is_ascii()) {
            for font in &fonts {
                assert!(h.ctx.fonts(|f| f.has_glyph(font, c)), "U+{:04X} in {text:?} would draw as a box ({font:?})", c as u32);
            }
        }
    }
}
