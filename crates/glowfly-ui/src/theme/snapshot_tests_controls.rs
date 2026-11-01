//! Snapshots of the form controls and ref chips in every built-in theme, and of a modal over a
//! window in a dark and a light theme (the backdrop and the dialog border).

use super::color::{self, Palette};
use super::contrast::{self, ChipStyle};
use super::controls::{self, Check};
use super::{apply_style, egui_theme};
use crate::ui::snapshot_check::CheckSnapshot;
use crate::ui::theme::branch_colours::{chip_in, swatches};
use crate::ui::widgets;
use eframe::egui::{self, vec2};
use glowfly_core::branch_colour::Swatch;

struct Form {
    empty: String,
    filled: String,
    focused: String,
    first_frame: bool,
    choice: u8,
}

/// Draws one of every control in `p`, then a row of ref chips.
fn render_controls(name: &str, p: Palette) {
    let form = Form { empty: String::new(), filled: "feature/login".into(), focused: "Focused".into(), first_frame: true, choice: 1 };
    let mut h = crate::ui::snapshot_check::builder().with_size(vec2(620.0, 300.0)).build_ui_state(
        move |ui, f: &mut Form| {
            color::set_palette(&p);
            apply_style(ui.ctx(), &p);
            ui.ctx().set_theme(egui_theme(&p));
            ui.painter().rect_filled(ui.max_rect(), 0.0, p.panel);
            ui.horizontal(|ui| {
                controls::field(ui, egui::TextEdit::singleline(&mut f.empty).hint_text("Filter branches").desired_width(180.0));
                controls::field(ui, egui::TextEdit::singleline(&mut f.filled).desired_width(180.0));
                let r = controls::field(ui, egui::TextEdit::singleline(&mut f.focused).desired_width(180.0));
                if f.first_frame {
                    r.request_focus();
                    f.first_frame = false;
                }
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let (mut off, mut on, mut part, mut locked) = (false, true, true, true);
                controls::checkbox(ui, &mut off, "Unchecked");
                controls::checkbox(ui, &mut on, "Checked");
                ui.add(Check::new(&mut part, "Indeterminate").indeterminate(true));
                ui.add_enabled(false, Check::new(&mut locked, "Disabled"));
            });
            ui.horizontal(|ui| {
                controls::radio_value(ui, &mut f.choice, 0, "Merge");
                controls::radio_value(ui, &mut f.choice, 1, "Rebase");
                ui.add_space(12.0);
                let _ = ui.selectable_label(true, "Hunks");
                let _ = ui.selectable_label(false, "Whole file");
            });
            ui.add_space(10.0);
            let (_, rect) = ui.allocate_space(vec2(ui.available_width(), 60.0));
            let painter = ui.painter();
            let mut x = rect.left();
            let mut y = rect.top() + 12.0;
            let mut chips = vec![(ChipStyle::Head, Swatch::Main, "HEAD"), (ChipStyle::CurrentBranch, Swatch::Main, "✔ main")];
            chips.extend(swatches().skip(1).map(|lane| (ChipStyle::Branch, lane, "topic")));
            chips.extend([(ChipStyle::Remote, Swatch::Main, "origin/main"), (ChipStyle::Tag, Swatch::Main, "v1.2.0")]);
            for (style, lane, text) in chips {
                let look = contrast::ref_chip(style, chip_in(lane, &p), &p);
                if x > rect.right() - 90.0 {
                    x = rect.left();
                    y += 26.0;
                }
                x += widgets::chip_with_edge(painter, x, y, text, look.fill, look.edge, look.text) + 6.0;
            }
        },
        form,
    );
    h.run();
    h.check_snapshot(name);
}

#[test]
fn controls_dark() {
    render_controls("controls_dark", super::DARK);
}

#[test]
fn controls_light() {
    render_controls("controls_light", super::LIGHT);
}

#[test]
fn controls_high_contrast_dark() {
    render_controls("controls_high_contrast_dark", super::HIGH_CONTRAST_DARK);
}

#[test]
fn controls_high_contrast_light() {
    render_controls("controls_high_contrast_light", super::HIGH_CONTRAST_LIGHT);
}

/// A window of text rows under a modal holding a label, a field and a checkbox.
fn render_modal(name: &str, p: Palette) {
    let (mut text, mut on) = (String::from("release/2.0"), true);
    let mut h = crate::ui::snapshot_check::builder().with_size(vec2(520.0, 300.0)).build(move |ctx| {
        color::set_palette(&p);
        apply_style(ctx, &p);
        ctx.set_theme(egui_theme(&p));
        egui::CentralPanel::default().show(ctx, |ui| {
            for row in ["Fix the login redirect", "Add the release notes", "Bump the version", "Merge branch 'topic'"] {
                ui.label(egui::RichText::new(row).color(p.text));
                ui.label(egui::RichText::new("2 hours ago").color(p.weak));
            }
        });
        super::show_modal(ctx, super::modal(ctx, egui::Id::new("snapshot-modal")), |ui| {
            ui.set_width(300.0);
            ui.heading("New branch");
            controls::field(ui, egui::TextEdit::singleline(&mut text).desired_width(f32::INFINITY));
            controls::checkbox(ui, &mut on, "Check out after creating");
        });
    });
    h.run();
    h.check_snapshot(name);
}

#[test]
fn modal_backdrop_dark() {
    render_modal("modal_backdrop_dark", super::DARK);
}

#[test]
fn modal_backdrop_light() {
    render_modal("modal_backdrop_light", super::LIGHT);
}
