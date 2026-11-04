//! Snapshots of the form controls (with a segmented control and quiet chips) and ref chips in
//! every built-in theme, the same sheet with the Strong focus indicator and a row of each focus
//! ring's targets in a dark and a light theme, and of a modal over a window in a dark and a light theme (the backdrop
//! and the dialog border).

use super::color::{self, Palette};
use super::controls::{self, Check};
use super::{apply_style, egui_theme};
use crate::access;
use crate::snapshot_check::CheckSnapshot;
use crate::theme::branch_colours::swatches;
use crate::theme::ref_chip::{self, BranchTone, ChipStyle};
use crate::theme::sizes::ControlSize;
use crate::theme::tokens::{self, radius};
use crate::widgets::button_split::split;
use crate::widgets::segmented::{Segment, segmented_opts};
use crate::widgets::{self, Icon, buttons, row_fill};
use eframe::egui::{self, vec2};
use glowfly_core::branch_colour::Swatch;
use glowfly_core::settings::FocusStyle;

struct Form {
    empty: String,
    filled: String,
    focused: String,
    first_frame: bool,
    choice: u8,
}

/// Draws one of every control in `p`, then a row of ref chips.
fn render_controls(name: &str, p: Palette) {
    render_controls_in(name, p, FocusStyle::Standard);
}

/// `render_controls` with the focus rings in `style`; Strong adds a row of focus targets.
fn render_controls_in(name: &str, p: Palette, style: FocusStyle) {
    let strong = style == FocusStyle::Strong;
    let form = Form { empty: String::new(), filled: "feature/login".into(), focused: "Focused".into(), first_frame: true, choice: 1 };
    let mut h = crate::snapshot_check::builder().with_size(vec2(620.0, if strong { 370.0 } else { 300.0 })).build_ui_state(
        move |ui, f: &mut Form| {
            tokens::set_focus_style(ui.ctx(), style);
            color::set_palette(&p);
            apply_style(ui.ctx(), &p);
            ui.ctx().set_theme(egui_theme(&p));
            ui.painter().rect_filled(ui.max_rect(), 0.0, p.panel);
            if strong {
                // Room for the focused field's Strong ring.
                ui.add_space(8.0);
            }
            ui.horizontal(|ui| {
                let empty = controls::field(ui, egui::TextEdit::singleline(&mut f.empty).hint_text("Filter branches").desired_width(180.0));
                access::name_field(&empty, "Empty field");
                let filled = controls::field(ui, egui::TextEdit::singleline(&mut f.filled).desired_width(180.0));
                access::name_field(&filled, "Filled field");
                let r = controls::field(ui, egui::TextEdit::singleline(&mut f.focused).desired_width(180.0));
                access::name_field(&r, "Focused field");
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
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let options = [
                    Segment::new(0u8, "Graph", "The graph"),
                    Segment::new(1, "List", "The list"),
                    Segment::new(2, "Tree", "The tree").why(Some("Not here")),
                ];
                segmented_opts(ui, ControlSize::Regular, 0, &options);
                ui.add_space(12.0);
                let _ = widgets::quiet_chip(ui, "a1b2c3d", "Copy the SHA");
                let _ = widgets::quiet_chip(ui, "Grace Hopper", "The committer");
            });
            ui.add_space(10.0);
            let (_, rect) = ui.allocate_space(vec2(ui.available_width(), 60.0));
            let painter = ui.painter();
            let mut x = rect.left();
            // The first row's halos reach 14 px above its middle.
            let mut y = rect.top() + 14.0;
            let mut chips = vec![(ChipStyle::Head, Swatch::Trunk, "HEAD"), (ChipStyle::CurrentBranch, Swatch::Trunk, "✔ main")];
            chips.extend(swatches().skip(1).map(|lane| (ChipStyle::Branch, lane, "topic")));
            chips.extend([(ChipStyle::Remote, Swatch::Trunk, "origin/main"), (ChipStyle::Tag, Swatch::Trunk, "v1.2.0")]);
            for (style, lane, text) in chips {
                let look = ref_chip::ref_chip(style, BranchTone::of(lane, &p), &p);
                if x > rect.right() - 90.0 {
                    x = rect.left();
                    y += 26.0;
                }
                let room = if look.halo.is_some() { widgets::chip::HALO_ROOM } else { 0.0 };
                let r = widgets::chip::Chip::new(widgets::chip::ChipSize::Chip, text).look(look).paint(painter, x + room, y);
                if let Some(halo) = look.halo {
                    widgets::chip::paint_halo(painter, r, widgets::chip::ChipSize::Chip, halo);
                }
                x = r.right() + room + 6.0;
            }
            if strong {
                focus_targets(ui, &p);
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

/// One of each focus ring's targets, each painted focused (egui focuses one widget a frame): a
/// framed button's outside ring, a quiet icon button's inset ring, a primary split's caret
/// (the ink edged by its fill) and a selected list row's square inset ring.
fn focus_targets(ui: &mut egui::Ui, p: &Palette) {
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 12.0;
        ui.label(egui::RichText::new("Focus targets").color(p.weak));
        let cancel = ui.add(buttons::secondary("Cancel"));
        tokens::focus_ring(ui.painter(), cancel.rect, radius::MD);
        let refresh = ui.add(buttons::icon_only(Icon::Refresh, "Refresh"));
        tokens::focus_ring_inset(ui.painter(), refresh.rect, radius::MD);
        let (_, caret) = split(buttons::primary("Pull"), "Pull options").show(ui);
        tokens::focus_ring_inset_in(ui.painter(), caret.rect, radius::segment(radius::MD, false, true), p.on_primary, p.primary);
    });
    ui.add_space(8.0);
    let (_, row) = ui.allocate_space(vec2(ui.available_width(), 32.0));
    row_fill::paint(ui.painter(), row, true, false);
    tokens::focus_ring_inset(ui.painter(), row, 0);
    let at = egui::pos2(row.left() + 12.0, row.center().y);
    ui.painter().text(at, egui::Align2::LEFT_CENTER, "feature/login", super::type_scale::font(super::type_scale::BODY), p.text);
}

#[test]
fn controls_focus_strong_dark() {
    render_controls_in("controls_focus_strong_dark", super::DARK, FocusStyle::Strong);
}

#[test]
fn controls_focus_strong_light() {
    render_controls_in("controls_focus_strong_light", super::LIGHT, FocusStyle::Strong);
}

/// A window of text rows under a modal holding a label, a field and a checkbox.
fn render_modal(name: &str, p: Palette) {
    let (mut text, mut on) = (String::from("release/2.0"), true);
    let mut h = crate::snapshot_check::builder().with_size(vec2(520.0, 300.0)).build(move |ctx| {
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
