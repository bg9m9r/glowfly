//! Colors: the built-in palettes, palettes built from theme files, and egui widget styling,
//! with the bundled fonts (`fonts.rs`), the type scale (`type_scale.rs`), the spacing grid
//! (`spacing.rs`), the design tokens (`tokens.rs`: corners, elevations, focus, motion) and the
//! colour tokens worked out from a palette (`surfaces.rs`: overlay, state layers, tones).
//!
//! Views read colors through `color::text()` etc.; the terminal reads `color::term()`, which
//! Preferences > Terminal > Colours can point at another theme's or an imported scheme
//! (`terminal_scheme.rs`). The active palette is a per-thread copy: the
//! shell's [`ThemeCache`] works out the palette for the chosen theme (again only when the
//! choice, the theme files or the OS setting change) and sets it by reference with
//! `color::set_palette` at the start of each frame, on the thread that draws it. Tests rendering
//! different palettes in parallel don't affect each other.

use crate::state::window_native::MicaTint;
use crate::themes::ThemeState;
use eframe::egui::{self, Stroke};
use glowfly_core::settings::{BranchPalette, BuiltinTheme, TerminalColours, ThemeChoice};

pub mod ansi;
pub mod branch_colours;
pub mod branch_names;
mod branch_palette;
mod branch_palette_gold;
mod branch_palette_more;
mod builtin;
mod builtin_cobalt;
mod builtin_dusk;
mod builtin_extra;
mod builtin_garden;
mod builtin_sea;
mod builtin_warm;
pub mod color;
pub mod contrast;
pub mod controls;
pub mod custom;
pub mod families;
pub mod fonts;
pub mod spacing;
pub mod surfaces;
pub mod terminal_scheme;
pub mod tokens;
pub mod type_scale;

#[cfg(test)]
pub use branch_palette::applied;
#[cfg(test)]
pub use builtin::{
    DARK, DAWN, DUSK, EMBER, GARDEN_DARK, GARDEN_LIGHT, GRAPHITE, HIGH_CONTRAST_DARK, HIGH_CONTRAST_LIGHT, LINEN, MIDNIGHT, MIST, PINE,
    SAND,
};
pub use builtin::{DEFAULT_DARK, LIGHT, builtin};
use color::Palette;
use eframe::egui::CornerRadius;
use tokens::{Elevation, radius, shadow};

/// egui's light or dark style for a palette.
pub fn egui_theme(p: &Palette) -> egui::Theme {
    if p.dark { egui::Theme::Dark } else { egui::Theme::Light }
}

/// Styles egui's built-in widgets from `p`, in the egui theme `p` uses, and installs the bundled
/// fonts (`fonts::install`, a cheap no-op once they are in). The other egui theme gets
/// DEFAULT_DARK or LIGHT, so a stray switch never shows egui's own defaults.
pub fn apply_style(ctx: &egui::Context, p: &Palette) {
    fonts::install(ctx);
    let other = if p.dark { &LIGHT } else { &DEFAULT_DARK };
    for p in [p, other] {
        ctx.style_mut_of(egui_theme(p), |style| {
            let v = &mut style.visuals;
            v.panel_fill = p.panel;
            v.window_fill = p.overlay();
            v.extreme_bg_color = p.input;
            v.faint_bg_color = p.hover;
            v.override_text_color = None;
            v.selection.bg_fill = p.selected;
            // egui draws a selected selectable_label's text in this colour; text fields set
            // their own focus ring in `controls::Field`.
            v.selection.stroke = Stroke::new(1.0_f32, p.strong);
            v.hyperlink_color = p.accent;
            v.window_stroke = Stroke::new(1.0_f32, p.overlay_edge());
            v.menu_corner_radius = CornerRadius::same(radius::LG);
            v.window_corner_radius = CornerRadius::same(radius::XL);
            v.popup_shadow = shadow(Elevation::Float, p);
            v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, p.border);
            v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, p.text);
            // egui fades hint text and disabled widgets toward this colour.
            v.widgets.noninteractive.weak_bg_fill = p.panel;
            v.widgets.inactive.bg_fill = p.raised;
            v.widgets.inactive.weak_bg_fill = p.raised;
            v.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, p.text);
            // Menu items sit on the overlay; egui's own buttons on the panel still lighten.
            let hovered = p.hover_on(p.overlay());
            v.widgets.hovered.bg_fill = hovered;
            v.widgets.hovered.weak_bg_fill = hovered;
            v.widgets.active.weak_bg_fill = p.selected;
            v.widgets.open.weak_bg_fill = hovered;
            for w in
                [&mut v.widgets.noninteractive, &mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open]
            {
                w.corner_radius = CornerRadius::same(radius::MD);
            }
            style.spacing.item_spacing = spacing::ITEM_SPACING;
            style.spacing.button_padding = spacing::BUTTON_PADDING;
            type_scale::install(style);
            super::widgets::tooltip::style(style);
        });
    }
}

/// The colour a modal dims the window with: lighter on light palettes, so both dim alike.
pub fn backdrop(p: &Palette) -> egui::Color32 {
    egui::Color32::from_black_alpha(if p.dark { 140 } else { 64 })
}

/// A modal in the active palette: its backdrop and egui's popup frame on the panel, with a
/// `dialog_border` edge and the Dialog shadow (margins and `LG` corners as egui's own modal).
pub fn modal(ctx: &egui::Context, id: egui::Id) -> egui::Modal {
    let p = color::current();
    egui::Modal::new(id).backdrop_color(backdrop(&p)).frame(modal_frame(ctx, &p))
}

fn modal_frame(ctx: &egui::Context, p: &Palette) -> egui::Frame {
    egui::Frame::popup(&ctx.style()).fill(p.panel).stroke(Stroke::new(1.0_f32, p.dialog_border)).shadow(shadow(Elevation::Dialog, p))
}

/// A dialog's padding on all sides (`dialog`); `dialog_frame` works out its caps from it.
pub const DIALOG_PAD: i8 = 24;

/// The frame of every dialog (`dialog_frame`): `modal` with `DIALOG_PAD` of padding on all sides
/// and `XL` corners. The command palette keeps `modal`'s look.
pub fn dialog(ctx: &egui::Context, id: egui::Id) -> egui::Modal {
    let p = color::current();
    egui::Modal::new(id).backdrop_color(backdrop(&p)).frame(dialog_frame_for(ctx, &p))
}

/// The dialog frame in palette `p`.
fn dialog_frame_for(ctx: &egui::Context, p: &Palette) -> egui::Frame {
    modal_frame(ctx, p).inner_margin(egui::Margin::same(DIALOG_PAD)).corner_radius(radius::XL)
}

/// Styles egui from this thread's active palette (tests that draw without the shell).
#[cfg(test)]
pub fn apply_theme(ctx: &egui::Context) {
    apply_style(ctx, &color::current());
}

/// The shell's palette for the chosen theme and branch preset, worked out again only when an
/// input changes.
#[derive(Default)]
pub struct ThemeCache {
    /// (choice, theme files generation, OS is dark, branch preset) the palette was resolved for.
    key: Option<(ThemeChoice, u64, bool, BranchPalette)>,
    palette: Option<Palette>,
    /// The palette egui's style was last set from.
    applied: Option<Palette>,
    /// The palette's Mica tints (`backdrop::mica_tint`); None for a high-contrast theme.
    mica: Option<MicaTint>,
    /// (terminal colours setting, theme files generation, palette) the terminal's colours were
    /// worked out for, and those colours (None: the palette's own).
    terminal_key: Option<(TerminalColours, u64, Palette)>,
    terminal: Option<families::Ansi>,
}

impl ThemeCache {
    /// `select` for the theme in the settings, with the OS's light or dark setting from egui.
    pub fn apply(&mut self, ctx: &egui::Context, settings: &glowfly_core::settings::Settings, themes: &ThemeState) {
        let system_dark = ctx.system_theme().is_none_or(|t| t == egui::Theme::Dark);
        self.select(ctx, &settings.theme, settings.branch_palette, themes, system_dark);
        self.select_terminal(&settings.terminal.colours, themes);
    }

    /// Makes the terminal colours `choice` picks active for this frame (`color::set_terminal`).
    pub fn select_terminal(&mut self, choice: &TerminalColours, themes: &ThemeState) {
        let palette = self.palette.unwrap_or(DEFAULT_DARK);
        let fresh = matches!(&self.terminal_key, Some((c, g, p)) if c == choice && *g == themes.generation && *p == palette);
        if !fresh {
            self.terminal = terminal_scheme::override_for(choice, &palette, &themes.catalog.terminal);
            self.terminal_key = Some((choice.clone(), themes.generation, palette));
        }
        color::set_terminal(self.terminal);
    }

    /// Makes the chosen theme's palette, with the branch preset `branches`, active for this frame
    /// and styles egui when it changed.
    pub fn select(&mut self, ctx: &egui::Context, choice: &ThemeChoice, branches: BranchPalette, themes: &ThemeState, system_dark: bool) {
        let fresh =
            matches!(&self.key, Some((c, g, s, b)) if c == choice && *g == themes.generation && *s == system_dark && *b == branches);
        if !fresh {
            let palette = branch_palette::applied(custom::resolve(choice, themes, system_dark), branches);
            let high_contrast =
                matches!(choice.builtin(system_dark), Some(BuiltinTheme::HighContrastDark | BuiltinTheme::HighContrastLight));
            self.mica = if high_contrast { None } else { super::backdrop::mica_tint(&palette) };
            self.palette = Some(palette);
            self.key = Some((choice.clone(), themes.generation, system_dark, branches));
        }
        let palette = self.palette.unwrap_or(DEFAULT_DARK);
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
mod bands;
#[cfg(test)]
mod branch_colour_cb_tests;
#[cfg(test)]
mod branch_colour_tests;
#[cfg(test)]
mod controls_tests;
#[cfg(test)]
mod diff_select_tests;
#[cfg(test)]
mod family_tests;
#[cfg(test)]
mod more_themes_tests;
#[cfg(test)]
mod snapshot_tests_controls;
#[cfg(test)]
mod snapshot_tests_fonts;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tint_tests;
#[cfg(test)]
mod token_tests;
#[cfg(test)]
mod vivid_tests;
