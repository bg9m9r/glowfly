//! Colors: the built-in palettes, palettes built from theme files, and egui widget styling.
//!
//! Views read colors through `color::text()` etc. The active palette is a per-thread copy: the
//! shell's [`ThemeCache`] works out the palette for the chosen theme (again only when the
//! choice, the theme files or the OS setting change) and sets it by reference with
//! `color::set_palette` at the start of each frame, on the thread that draws it. Tests rendering
//! different palettes in parallel don't affect each other.

use crate::themes::ThemeState;
use eframe::egui::{self, Stroke, vec2};
use trellis_core::settings::ThemeChoice;

mod builtin;
mod builtin_extra;
pub mod color;
pub mod custom;

pub use builtin::{DARK, LIGHT, builtin};
#[cfg(test)]
pub use builtin::{HIGH_CONTRAST_DARK, HIGH_CONTRAST_LIGHT, SAND};
use color::Palette;

/// egui's light or dark style for a palette.
pub fn egui_theme(p: &Palette) -> egui::Theme {
    if p.dark { egui::Theme::Dark } else { egui::Theme::Light }
}

/// Styles egui's built-in widgets from `p`, in the egui theme `p` uses. The other egui theme
/// gets DARK or LIGHT, so a stray switch never shows egui's own defaults.
pub fn apply_style(ctx: &egui::Context, p: &Palette) {
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
            v.selection.stroke = Stroke::new(1.0_f32, p.accent);
            v.hyperlink_color = p.accent;
            v.window_stroke = Stroke::new(1.0_f32, p.border);
            v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, p.border);
            v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, p.text);
            v.widgets.inactive.bg_fill = p.toolbar;
            v.widgets.inactive.weak_bg_fill = p.toolbar;
            v.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, p.text);
            v.widgets.hovered.bg_fill = p.hover;
            v.widgets.hovered.weak_bg_fill = p.hover;
            v.widgets.active.weak_bg_fill = p.selected;
            v.widgets.open.weak_bg_fill = p.hover;
            style.spacing.item_spacing = vec2(6.0, 4.0);
            style.spacing.button_padding = vec2(8.0, 3.0);
        });
    }
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
}

impl ThemeCache {
    /// Makes the chosen theme's palette active for this frame and styles egui when it changed.
    pub fn select(&mut self, ctx: &egui::Context, choice: &ThemeChoice, themes: &ThemeState, system_dark: bool) {
        let fresh = matches!(&self.key, Some((c, g, s)) if c == choice && *g == themes.generation && *s == system_dark);
        if !fresh {
            self.palette = Some(custom::resolve(choice, themes, system_dark));
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
}

#[cfg(test)]
mod tests;
