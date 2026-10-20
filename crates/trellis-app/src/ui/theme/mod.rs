//! Colors: the built-in palettes, palettes built from theme files, and egui widget styling,
//! with the bundled fonts (`fonts.rs`), the type scale (`type_scale.rs`) and the spacing grid
//! (`spacing.rs`).
//!
//! Views read colors through `color::text()` etc. The active palette is a per-thread copy: the
//! shell's [`ThemeCache`] works out the palette for the chosen theme (again only when the
//! choice, the theme files or the OS setting change) and sets it by reference with
//! `color::set_palette` at the start of each frame, on the thread that draws it. Tests rendering
//! different palettes in parallel don't affect each other.

use crate::state::window_native::MicaTint;
use crate::themes::ThemeState;
use eframe::egui::{self, Stroke};
use trellis_core::settings::{BuiltinTheme, ThemeChoice};

mod ansi;
mod builtin;
mod builtin_extra;
pub mod color;
pub mod contrast;
pub mod controls;
pub mod custom;
pub mod families;
pub mod fonts;
pub mod spacing;
pub mod type_scale;

pub use builtin::{DARK, LIGHT, builtin};
#[cfg(test)]
pub use builtin::{HIGH_CONTRAST_DARK, HIGH_CONTRAST_LIGHT, SAND};
use color::Palette;

/// egui's light or dark style for a palette.
pub fn egui_theme(p: &Palette) -> egui::Theme {
    if p.dark { egui::Theme::Dark } else { egui::Theme::Light }
}

/// Styles egui's built-in widgets from `p`, in the egui theme `p` uses, and installs the bundled
/// fonts (`fonts::install`, a cheap no-op once they are in). The other egui theme gets DARK or
/// LIGHT, so a stray switch never shows egui's own defaults.
pub fn apply_style(ctx: &egui::Context, p: &Palette) {
    fonts::install(ctx);
    let other = if p.dark { &LIGHT } else { &DARK };
    for p in [p, other] {
        ctx.style_mut_of(egui_theme(p), |style| {
            let v = &mut style.visuals;
            v.panel_fill = p.panel;
            v.window_fill = p.panel;
            v.extreme_bg_color = p.input;
            v.faint_bg_color = p.hover;
            v.override_text_color = None;
            v.selection.bg_fill = p.selected;
            // egui draws a selected selectable_label's text in this colour; text fields set
            // their own focus ring in `controls::Field`.
            v.selection.stroke = Stroke::new(1.0_f32, p.strong);
            v.hyperlink_color = p.accent;
            v.window_stroke = Stroke::new(1.0_f32, p.border);
            v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, p.border);
            v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, p.text);
            // egui fades hint text and disabled widgets toward this colour.
            v.widgets.noninteractive.weak_bg_fill = p.panel;
            v.widgets.inactive.bg_fill = p.raised;
            v.widgets.inactive.weak_bg_fill = p.raised;
            v.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, p.text);
            v.widgets.hovered.bg_fill = p.hover;
            v.widgets.hovered.weak_bg_fill = p.hover;
            v.widgets.active.weak_bg_fill = p.selected;
            v.widgets.open.weak_bg_fill = p.hover;
            style.spacing.item_spacing = spacing::ITEM_SPACING;
            style.spacing.button_padding = spacing::BUTTON_PADDING;
            type_scale::install(style);
            super::widgets::tooltip::style(style, p);
        });
    }
}

/// The colour a modal dims the window with: lighter on light palettes, so both dim alike.
pub fn backdrop(p: &Palette) -> egui::Color32 {
    egui::Color32::from_black_alpha(if p.dark { 140 } else { 64 })
}

/// A modal in the active palette: its backdrop and egui's popup frame with a `dialog_border`
/// edge (margins, shadow and corners as egui's own modal).
pub fn modal(ctx: &egui::Context, id: egui::Id) -> egui::Modal {
    let p = color::current();
    let frame = egui::Frame::popup(&ctx.style()).stroke(Stroke::new(1.0_f32, p.dialog_border));
    egui::Modal::new(id).backdrop_color(backdrop(&p)).frame(frame)
}

/// The frame of every dialog (`dialog_frame`): `modal` with 18 px of padding on all sides and
/// 8 px corners. The command palette and the welcome flow keep `modal`'s look.
pub fn dialog(ctx: &egui::Context, id: egui::Id) -> egui::Modal {
    let p = color::current();
    let frame = egui::Frame::popup(&ctx.style())
        .stroke(Stroke::new(1.0_f32, p.dialog_border))
        .inner_margin(egui::Margin::same(18))
        .corner_radius(8);
    egui::Modal::new(id).backdrop_color(backdrop(&p)).frame(frame)
}

/// Styles egui from this thread's active palette (tests that draw without the shell).
#[cfg(test)]
pub fn apply_theme(ctx: &egui::Context) {
    apply_style(ctx, &color::current());
}

/// The shell's palette for the chosen theme, worked out again only when an input changes.
#[derive(Default)]
pub struct ThemeCache {
    /// (choice, theme files generation, OS is dark) the palette was resolved for.
    key: Option<(ThemeChoice, u64, bool)>,
    palette: Option<Palette>,
    /// The palette egui's style was last set from.
    applied: Option<Palette>,
    /// The palette's Mica tints (`backdrop::mica_tint`); None for a high-contrast theme.
    mica: Option<MicaTint>,
}

impl ThemeCache {
    /// `select` for the theme in the settings, with the OS's light or dark setting from egui.
    pub fn apply(&mut self, ctx: &egui::Context, settings: &trellis_core::settings::Settings, themes: &ThemeState) {
        let system_dark = ctx.system_theme().is_none_or(|t| t == egui::Theme::Dark);
        self.select(ctx, &settings.theme, themes, system_dark);
    }

    /// Makes the chosen theme's palette active for this frame and styles egui when it changed.
    pub fn select(&mut self, ctx: &egui::Context, choice: &ThemeChoice, themes: &ThemeState, system_dark: bool) {
        let fresh = matches!(&self.key, Some((c, g, s)) if c == choice && *g == themes.generation && *s == system_dark);
        if !fresh {
            let palette = custom::resolve(choice, themes, system_dark);
            let high_contrast =
                matches!(choice.builtin(system_dark), Some(BuiltinTheme::HighContrastDark | BuiltinTheme::HighContrastLight));
            self.mica = if high_contrast { None } else { super::backdrop::mica_tint(&palette) };
            self.palette = Some(palette);
            self.key = Some((choice.clone(), themes.generation, system_dark));
        }
        let palette = self.palette.unwrap_or(DARK);
        color::set_palette(&palette);
        if self.applied != Some(palette) {
            apply_style(ctx, &palette);
            self.applied = Some(palette);
        }
        let theme = egui_theme(&palette);
        if ctx.options(|o| o.theme_preference) != theme.into() {
            ctx.set_theme(theme);
        }
    }

    /// The selected palette's Mica tints: None when it must stay solid.
    pub fn mica_tint(&self) -> Option<MicaTint> {
        self.mica
    }
}

#[cfg(test)]
mod ansi_tests;
#[cfg(test)]
mod controls_tests;
#[cfg(test)]
mod family_tests;
#[cfg(test)]
mod snapshot_tests_controls;
#[cfg(test)]
mod snapshot_tests_fonts;
#[cfg(test)]
mod tests;
