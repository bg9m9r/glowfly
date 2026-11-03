//! The design sheets: the button family in a dark, a light, a high-contrast and the Sand theme
//! (every kind at rest, focused in each focus style and disabled, with each kind's hover and
//! pressed look), the split
//! buttons' states in a dark and a light theme, and the foundations in a dark and a light theme
//! (the surface ladder with the overlay's Float shadow, a Dialog-elevation card, the corner
//! scale, the tone tints and the state layers on the panel and the overlay). Declared under
//! `widgets` so it can paint each state's look.

use super::button_look::{Press, Target, look, paint_button};
use super::button_split::split;
use super::buttons::{self, Kind, StyledButton};
use super::{Icon, paint_icon_sized};
use crate::ui::snapshot_check::CheckSnapshot;
use crate::ui::theme::color::{self, Palette};
use crate::ui::theme::sizes::ControlSize;
use crate::ui::theme::surfaces::Tone;
use crate::ui::theme::tokens::{self, Elevation, radius, shadow};
use crate::ui::theme::{self, type_scale};
use eframe::egui::{self, Align2, CornerRadius, Rect, Shape, StrokeKind, Ui, pos2, vec2};
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::FocusStyle;

/// The kinds that carry text, with a label each.
const KINDS: [(Kind, &str); 6] = [
    (Kind::Primary, "Commit"),
    (Kind::Tonal, "Stage all"),
    (Kind::Secondary, "Cancel"),
    (Kind::Quiet, "Details"),
    (Kind::Danger, "Discard"),
    (Kind::DangerQuiet, "Discard hunk"),
];

fn button(kind: Kind, label: &'static str) -> StyledButton<'static> {
    match kind {
        Kind::Primary => buttons::primary(label),
        Kind::Tonal => buttons::tonal(label),
        Kind::Secondary => buttons::secondary(label),
        Kind::Quiet => buttons::quiet(label),
        Kind::Danger => buttons::danger(label),
        Kind::DangerQuiet => buttons::danger_quiet(label),
    }
}

/// Styles egui and the palette for one frame, and fills the panel.
fn frame(ui: &Ui, p: &Palette) {
    color::set_palette(p);
    theme::apply_style(ui.ctx(), p);
    ui.ctx().set_theme(theme::egui_theme(p));
    ui.painter().rect_filled(ui.max_rect(), 0.0, p.panel);
}

fn caption(ui: &mut Ui, text: &str) {
    ui.add_sized(vec2(72.0, 28.0), egui::Label::new(egui::RichText::new(text).size(type_scale::SMALL).color(color::weak())));
}

/// One row of real buttons, `enabled` or not; the first frame focuses the one named `focus`.
fn live_row(ui: &mut Ui, enabled: bool, focus: Option<&str>, first: bool) {
    ui.horizontal(|ui| {
        caption(ui, if enabled { "Rest" } else { "Disabled" });
        for (kind, label) in KINDS {
            let r = ui.add(button(kind, label).enabled(enabled).why(Some("Why not")));
            if first && focus == Some(label) {
                r.request_focus();
            }
        }
        ui.add(buttons::icon_only(Icon::Refresh, "Refresh").enabled(enabled));
        let _ = split(buttons::primary("Pull").enabled(enabled), "Pull options").show(ui);
    });
}

/// The rest row again, 8 px clear of its neighbours; returns Cancel's rect, whose focus ring
/// `strong_ring` paints once every row is filled (egui's live focus is the rest row's).
fn strong_row(ui: &mut Ui) -> Rect {
    let mut cancel = Rect::NOTHING;
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        caption(ui, "Strong");
        for (kind, label) in KINDS {
            let r = ui.add(button(kind, label));
            if label == "Cancel" {
                cancel = r.rect;
            }
        }
        ui.add(buttons::icon_only(Icon::Refresh, "Refresh"));
        let _ = split(buttons::primary("Pull"), "Pull options").show(ui);
    });
    ui.add_space(8.0);
    cancel
}

/// Paints the Strong focus ring around `rect`, then sets the style back to Standard.
fn strong_ring(ui: &Ui, rect: Rect) {
    tokens::set_focus_style(ui.ctx(), FocusStyle::Strong);
    tokens::focus_ring(ui.painter(), rect, ControlSize::Regular.metrics().radius);
    tokens::set_focus_style(ui.ctx(), FocusStyle::Standard);
}

/// One row of each kind's look in state `press`, painted by the buttons' own painter.
fn painted_row(ui: &mut Ui, label: &str, press: Press) {
    ui.horizontal(|ui| {
        caption(ui, label);
        let size = ControlSize::Regular;
        for (kind, text) in KINDS {
            let w = button(kind, text).desired_width(ui);
            let (rect, _) = ui.allocate_exact_size(vec2(w, size.metrics().h), egui::Sense::hover());
            let l = look(kind, true, press);
            let target = Target { rect, corners: CornerRadius::same(size.metrics().radius), edge: kind.framed().then_some(l.edge) };
            paint_button(ui.painter(), target, size, l, false);
            ui.painter().text(rect.center(), Align2::CENTER_CENTER, text, type_scale::font(size.metrics().text), l.ink);
        }
        let side = size.metrics().h;
        let (rect, _) = ui.allocate_exact_size(vec2(side, side), egui::Sense::hover());
        let l = look(Kind::Quiet, true, press);
        paint_button(ui.painter(), Target { rect, corners: CornerRadius::same(size.metrics().radius), edge: None }, size, l, false);
        paint_icon_sized(ui.painter(), rect.center(), Icon::Refresh, l.ink, size.metrics().icon);
    });
}

fn render_buttons(name: &str, p: Palette) {
    let mut h = crate::ui::snapshot_check::builder().with_size(vec2(820.0, 342.0)).build_ui_state(
        move |ui, first: &mut bool| {
            frame(ui, &p);
            ui.add_space(8.0);
            live_row(ui, true, Some("Cancel"), *first);
            let cancel = strong_row(ui);
            live_row(ui, false, None, *first);
            painted_row(ui, "Hover", Press::Hover);
            painted_row(ui, "Pressed", Press::Pressed);
            ui.horizontal(|ui| {
                caption(ui, "Split");
                let _ = split(buttons::primary("Commit 2 files"), "More").open(true).show(ui);
                let _ = split(buttons::primary("Commit (skip hooks)"), "More").menu_tone(Some(Tone::Warning)).show(ui);
                let _ = split(buttons::secondary("Fetch").size(ControlSize::Small), "More").show(ui);
            });
            ui.horizontal(|ui| {
                caption(ui, "Small");
                for (kind, label) in KINDS {
                    ui.add(button(kind, label).size(ControlSize::Small));
                }
                ui.add(buttons::icon_only(Icon::Close, "Close").size(ControlSize::Small));
            });
            strong_ring(ui, cancel);
            *first = false;
        },
        true,
    );
    h.run();
    // The pointer on the small split's action: only that half lights.
    h.get_by_label("Fetch").hover();
    h.run();
    h.run();
    h.check_snapshot(name);
}

#[test]
fn design_buttons_dark() {
    render_buttons("design_buttons_dark", theme::DARK);
}

#[test]
fn design_buttons_light() {
    render_buttons("design_buttons_light", theme::LIGHT);
}

#[test]
fn design_buttons_high_contrast_dark() {
    render_buttons("design_buttons_high_contrast_dark", theme::HIGH_CONTRAST_DARK);
}

/// The split buttons' states, live: the pointer held on a secondary split's action, a primary
/// split's caret focused, a disabled action beside a caret that still works, and the danger and
/// tonal kinds, whose divider is a shade of their fill.
fn render_splits(name: &str, p: Palette) {
    let mut h = crate::ui::snapshot_check::builder().with_size(vec2(520.0, 140.0)).build_ui_state(
        move |ui, _: &mut ()| {
            frame(ui, &p);
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                caption(ui, "Pressed");
                let _ = split(buttons::secondary("Push"), "Push options").show(ui);
                caption(ui, "Focus");
                // Every frame: the press on Push would take the focus.
                split(buttons::primary("Commit"), "Commit options").show(ui).1.request_focus();
            });
            ui.horizontal(|ui| {
                caption(ui, "Disabled");
                let _ = split(buttons::primary("Commit 2 files").enabled(false), "More").show(ui);
            });
            ui.horizontal(|ui| {
                caption(ui, "Kinds");
                let _ = split(buttons::danger("Discard"), "Discard options").show(ui);
                let _ = split(buttons::tonal("Stage all"), "Stage options").show(ui);
            });
        },
        (),
    );
    h.run();
    let push = h.get_by_label("Push").bounding_box().expect("the Push action");
    let at = egui::pos2(((push.x0 + push.x1) / 2.0) as f32, ((push.y0 + push.y1) / 2.0) as f32);
    h.input_mut().events.push(egui::Event::PointerMoved(at));
    let press = egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() };
    h.input_mut().events.push(press);
    h.run();
    h.run();
    h.check_snapshot(name);
}

#[test]
fn design_split_states_dark() {
    render_splits("design_split_states_dark", theme::DARK);
}

#[test]
fn design_split_states_light() {
    render_splits("design_split_states_light", theme::LIGHT);
}

/// A labelled swatch at `rect` in `fill`, with `ink` text (the label under it when `ink` is
/// None).
fn swatch(ui: &Ui, rect: Rect, r: impl Into<CornerRadius>, fill: egui::Color32, label: &str, ink: Option<egui::Color32>) {
    let p = ui.painter();
    p.rect(rect, r, fill, egui::Stroke::new(1.0_f32, color::border()), StrokeKind::Inside);
    let font = type_scale::font(type_scale::SMALL);
    match ink {
        Some(ink) => {
            p.text(rect.center(), Align2::CENTER_CENTER, label, font, ink);
        }
        None => {
            p.text(pos2(rect.center().x, rect.bottom() + 10.0), Align2::CENTER_CENTER, label, font, color::weak());
        }
    }
}

fn render_surfaces(name: &str, p: Palette) {
    let mut h = crate::ui::snapshot_check::builder().with_size(vec2(720.0, 360.0)).build_ui(move |ui| {
        frame(ui, &p);
        let o = ui.max_rect().min + vec2(16.0, 16.0);
        let at = |x: f32, y: f32, w: f32, h: f32| Rect::from_min_size(o + vec2(x, y), vec2(w, h));
        // The ladder, from the window's edge in.
        let ladder = [("chrome", p.toolbar), ("base", p.bg), ("panel", p.panel), ("raised", p.raised), ("overlay", p.overlay())];
        for (i, (label, fill)) in ladder.into_iter().enumerate() {
            let rect = at(i as f32 * 100.0, 0.0, 88.0, 56.0);
            if label == "overlay" {
                ui.painter().add(Shape::from(shadow(Elevation::Float, &p).as_shape(rect, radius::LG)));
            }
            swatch(ui, rect, radius::LG, fill, label, None);
        }
        // A dialog's elevation.
        let card = at(520.0, 0.0, 150.0, 90.0);
        ui.painter().add(Shape::from(shadow(Elevation::Dialog, &p).as_shape(card, radius::XL)));
        ui.painter().rect(card, radius::XL, p.panel, egui::Stroke::new(1.0_f32, p.dialog_border), StrokeKind::Inside);
        ui.painter().text(card.center(), Align2::CENTER_CENTER, "Dialog", type_scale::font(type_scale::BODY), p.text);
        // The corner scale.
        for (i, (label, r)) in [
            ("XS", radius::XS),
            ("SM", radius::SM),
            ("MD", radius::MD),
            ("LG", radius::LG),
            ("XL", radius::XL),
            ("pill", radius::pill(28.0)),
        ]
        .into_iter()
        .enumerate()
        {
            swatch(ui, at(i as f32 * 76.0, 110.0, 64.0, 28.0), r, p.raised, label, None);
        }
        // The tones' tints.
        for (i, t) in Tone::ALL.into_iter().enumerate() {
            let (fill, ink) = p.tone_tint(t);
            swatch(ui, at(i as f32 * 100.0, 170.0, 88.0, 28.0), radius::pill(28.0), fill, &format!("{t:?}"), Some(ink));
        }
        let tonal = p.tonal_fill();
        swatch(ui, at(500.0, 170.0, 88.0, 28.0), radius::MD, tonal, "Tonal", Some(p.tonal_ink(tonal)));
        // The state layers on the panel and on the overlay.
        for (row, (surface, under)) in [("panel", p.panel), ("overlay", p.overlay())].into_iter().enumerate() {
            let y = 220.0 + row as f32 * 50.0;
            let states =
                [("rest", under), ("hover", p.hover_on(under)), ("pressed", p.pressed_on(under)), ("latched", p.latched_on(under))];
            for (i, (label, fill)) in states.into_iter().enumerate() {
                swatch(ui, at(i as f32 * 116.0, y, 108.0, 32.0), radius::MD, fill, &format!("{surface} {label}"), Some(p.text));
            }
        }
    });
    h.run();
    h.check_snapshot(name);
}

#[test]
fn design_surfaces_dark() {
    render_surfaces("design_surfaces_dark", theme::DARK);
}

#[test]
fn design_surfaces_light() {
    render_surfaces("design_surfaces_light", theme::LIGHT);
}
