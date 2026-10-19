//! The shared tooltip: egui's tooltip frame and timing for every tooltip in the app (`style`,
//! `configure`), and the rich layout the toolbar's buttons use (`show`, `attach`): a title, a
//! weak subline of key caps or state, a sentence, an inset preview and a footer.
//!
//! Every `on_hover_text` gets the frame (rounded, softly shadowed, no pointer), the 360 px wrap
//! width and the timing (400 ms of stillness; at once when another tooltip was just showing)
//! without changes at its call site.

use super::tooltip_motion;
use super::truncate::{cut_left, truncate_label};
use crate::keymap::Keymap;
use crate::state::tip::{Tip, TipPreview, TipRow, TipSub};
use crate::ui::common::{status_color, status_label};
use crate::ui::shortcuts::Caps;
use crate::ui::theme::color::{self, Palette};
use crate::ui::theme::type_scale;
use eframe::egui::{self, Color32, CornerRadius, Frame, Label, Margin, Response, RichText, Sense, Shadow, Ui};
use trellis_core::i18n::Lang;

/// The rich tooltip's narrowest and widest.
const MIN_W: f32 = 220.0;
const MAX_W: f32 = 340.0;
/// Plain tooltips wrap at this width.
const TEXT_W: f32 = 360.0;

/// egui's tooltip (and menu) frame for palette `p`: 8 px corners and a soft shadow under it.
pub fn style(style: &mut egui::Style, p: &Palette) {
    style.visuals.menu_corner_radius = CornerRadius::same(8);
    let alpha = if p.dark { 64 } else { 28 };
    style.visuals.popup_shadow = Shadow { offset: [0, 4], blur: 16, spread: 0, color: Color32::from_black_alpha(alpha) };
    style.spacing.tooltip_width = TEXT_W;
}

/// The tooltip timing, in both egui themes: shown after 400 ms of stillness, at once within
/// 300 ms of another, fading in over 120 ms (no animations at all with `reduce_motion`).
/// Writes the style only when a value differs.
pub fn configure(ctx: &egui::Context, reduce_motion: bool) {
    let animation = if reduce_motion { 0.0 } else { tooltip_motion::FADE };
    for theme in [egui::Theme::Dark, egui::Theme::Light] {
        let s = ctx.style_of(theme);
        let i = &s.interaction;
        let same = i.tooltip_delay == 0.4
            && i.tooltip_grace_time == tooltip_motion::GRACE
            && i.show_tooltips_only_when_still
            && s.animation_time == animation;
        if !same {
            ctx.style_mut_of(theme, |s| {
                s.interaction.tooltip_delay = 0.4;
                s.interaction.tooltip_grace_time = tooltip_motion::GRACE;
                s.interaction.show_tooltips_only_when_still = true;
                s.animation_time = animation;
            });
        }
    }
}

/// Draws `tip` in a tooltip's `ui`, 220 to 340 px wide; its key caps follow `keys`, and "and N
/// more" is in `t`.
pub fn show(ui: &mut Ui, tip: &Tip, keys: &Keymap, t: Lang) {
    if tooltip_motion::switching(ui.ctx(), ui.layer_id()) {
        ui.set_opacity(1.0);
    }
    ui.set_min_width(MIN_W);
    ui.set_max_width(MAX_W);
    ui.spacing_mut().item_spacing.y = 4.0;
    ui.add(Label::new(RichText::new(&tip.title).size(type_scale::BODY).color(color::strong())).wrap());
    match &tip.sub {
        Some(TipSub::Keys(id)) => {
            if let Some(first) = keys.chords(*id).first() {
                let caps = Caps::new(ui, std::slice::from_ref(first), false);
                caps.paint(ui, keys.hint(*id).unwrap_or_default(), Sense::hover());
            }
        }
        Some(TipSub::State(line)) => weak(ui, line),
        None => {}
    }
    if let Some(body) = &tip.body {
        ui.add(Label::new(RichText::new(body).size(type_scale::BODY).color(color::text())).wrap());
    }
    if let Some(p) = &tip.preview {
        ui.add_space(2.0);
        preview(ui, p, t);
    }
    if let Some(footer) = &tip.footer {
        weak(ui, footer);
    }
}

fn weak(ui: &mut Ui, text: &str) {
    ui.add(Label::new(RichText::new(text).size(type_scale::SMALL).color(color::weak())).wrap());
}

/// The inset list: heading, rows, "and N more" and the note.
fn preview(ui: &mut Ui, p: &TipPreview, t: Lang) {
    Frame::new().fill(color::raised()).corner_radius(CornerRadius::same(6)).inner_margin(Margin::same(8)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 3.0;
        ui.add(Label::new(type_scale::section_text(ui, &p.heading)).wrap());
        for row in &p.rows {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                match row {
                    TipRow::Commit { sha, summary } => {
                        ui.label(RichText::new(sha).font(type_scale::mono(type_scale::SMALL)).color(color::weak()));
                        truncate_label(ui, RichText::new(summary).size(type_scale::SMALL).color(color::text()), ui.available_width());
                    }
                    TipRow::File { status, path } => {
                        let letter = status_label(*status).to_string();
                        ui.label(RichText::new(letter).font(type_scale::mono(type_scale::SMALL)).color(status_color(*status)));
                        let font = type_scale::font(type_scale::SMALL);
                        let shown = cut_left(ui, path, &font, ui.available_width());
                        ui.label(RichText::new(shown).font(font).color(color::text()));
                    }
                    TipRow::Line(text) => {
                        ui.add(Label::new(RichText::new(text).size(type_scale::SMALL).color(color::weak())).wrap());
                    }
                }
            });
        }
        if p.more > 0 {
            weak(ui, &t.trn("toolbar.tip.more", p.more, &[]));
        }
        if let Some(note) = &p.note {
            weak(ui, note);
        }
    });
}

/// Gives `resp` the rich tooltip `build` makes (only while it shows): on hover, or under the
/// button while it has the keyboard focus and the pointer is elsewhere.
pub fn attach(resp: Response, keys: &Keymap, t: Lang, build: impl FnOnce() -> Tip) -> Response {
    if resp.has_focus() && !resp.hovered() {
        resp.show_tooltip_ui(|ui| show(ui, &build(), keys, t));
        resp
    } else if resp.enabled() {
        resp.on_hover_ui(|ui| show(ui, &build(), keys, t))
    } else {
        resp.on_disabled_hover_ui(|ui| show(ui, &build(), keys, t))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configure_sets_the_timing_in_both_themes() {
        let ctx = egui::Context::default();
        configure(&ctx, false);
        for theme in [egui::Theme::Dark, egui::Theme::Light] {
            let s = ctx.style_of(theme);
            assert_eq!((s.interaction.tooltip_delay, s.interaction.tooltip_grace_time, s.animation_time), (0.4, 0.3, 0.12));
            assert!(s.interaction.show_tooltips_only_when_still);
        }
        configure(&ctx, true);
        for theme in [egui::Theme::Dark, egui::Theme::Light] {
            assert_eq!(ctx.style_of(theme).animation_time, 0.0, "reduced motion");
        }
    }

    #[test]
    fn style_rounds_and_shadows_the_frame() {
        for (p, alpha) in [(crate::ui::theme::DARK, 64), (crate::ui::theme::LIGHT, 28)] {
            let mut s = egui::Style::default();
            style(&mut s, &p);
            assert_eq!(s.visuals.menu_corner_radius, CornerRadius::same(8));
            let want = Shadow { offset: [0, 4], blur: 16, spread: 0, color: Color32::from_black_alpha(alpha) };
            assert_eq!(s.visuals.popup_shadow, want);
            assert_eq!(s.spacing.tooltip_width, 360.0);
        }
    }
}
