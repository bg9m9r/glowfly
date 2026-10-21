//! The colour tokens worked out from a palette, so every theme (custom theme files too) gets
//! them with no theme-file keys of its own: the field, guide and toolbar-group fills, the
//! overlay surface that menus, tooltips and toasts float on, the state layers (hover, pressed,
//! latched), the semantic tones with their tints, the tonal button's fill and ink, and the
//! shading of filled buttons. See docs/COLOUR.md for what each is for.
//!
//! The surface ladder, from the window's edge in: chrome (`toolbar`), base (`bg`), `panel`,
//! `raised` and `overlay`.

use super::color::{self, Palette};
use super::contrast::{self, mix};
use super::tokens::{FILL_HOVER, FILL_PRESSED, HOVER, LATCHED, PRESSED};
use eframe::egui::Color32;

/// How far a text field's fill moves from `input` toward the accent.
const FIELD_TINT: f32 = 0.04;
/// How far a dialog's side pane moves from `panel` toward the accent.
const GUIDE_TINT: f32 = 0.07;
/// How far the toolbar groups' backing moves from `toolbar` toward the accent.
const TOOL_TINT: f32 = 0.09;
/// How far the dark overlay moves from `raised` toward `text`: one step lighter.
const OVERLAY_STEP: f32 = 0.06;
/// How far the dark overlay's edge moves from the overlay toward `text`.
const OVERLAY_EDGE: f32 = 0.12;
/// The least contrast of the tonal ink on its fills, and in a high-contrast palette (one whose
/// text reaches `HIGH_CONTRAST_TEXT` on the panel).
const INK_MIN: f64 = 4.5;
const INK_MIN_HC: f64 = 7.0;
const HIGH_CONTRAST_TEXT: f64 = 18.0;

/// A semantic tone: the colour of an outcome or an emphasis, with a soft tint of it to fill a
/// chip, a toast's edge or a menu caret.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Accent,
    Success,
    Warning,
    Danger,
    Info,
}

#[cfg(test)]
impl Tone {
    pub const ALL: [Tone; 5] = [Self::Accent, Self::Success, Self::Warning, Self::Danger, Self::Info];
}

impl Palette {
    /// The fill of a dialog's text field: `input` with a faint accent tint.
    pub fn field_fill(&self) -> Color32 {
        mix(self.input, self.accent, FIELD_TINT)
    }

    /// The fill of a dialog's side pane (`dialog_frame`'s guide): `panel` tinted toward the
    /// accent.
    pub fn guide_fill(&self) -> Color32 {
        mix(self.panel, self.accent, GUIDE_TINT)
    }

    /// The neutral backing of the toolbar's groups: `toolbar` with a quiet accent tint.
    pub fn tool_backing(&self) -> Color32 {
        mix(self.toolbar, self.accent, TOOL_TINT)
    }

    /// A hovered tool button on the backing.
    pub fn tool_hover(&self) -> Color32 {
        self.hover_on(self.tool_backing())
    }

    /// A tool button held down on the backing.
    pub fn tool_press(&self) -> Color32 {
        self.pressed_on(self.tool_backing())
    }

    /// A toggle that is on, or a caret whose menu is open, on the backing (latched).
    pub fn tool_pressed(&self) -> Color32 {
        self.latched_on(self.tool_backing())
    }

    /// The surface menus, tooltips, popups and toasts float on. Dark palettes: a step lighter
    /// than `raised`. Light ones: `panel`, and the shadow does the lifting.
    pub fn overlay(&self) -> Color32 {
        if self.dark { mix(self.raised, self.text, OVERLAY_STEP) } else { self.panel }
    }

    /// The 1 px edge of the overlay.
    pub fn overlay_edge(&self) -> Color32 {
        if self.dark { mix(self.overlay(), self.text, OVERLAY_EDGE) } else { self.border }
    }

    /// The fill of a block set into an overlay (a tooltip's preview list): `panel` on dark
    /// palettes, `raised` on light ones.
    pub fn inset(&self) -> Color32 {
        if self.dark { self.panel } else { self.raised }
    }

    /// Surface `s` hovered: 8% toward `text`.
    pub fn hover_on(&self, s: Color32) -> Color32 {
        mix(s, self.text, HOVER)
    }

    /// Surface `s` pressed: 12% toward `text`.
    pub fn pressed_on(&self, s: Color32) -> Color32 {
        mix(s, self.text, PRESSED)
    }

    /// Surface `s` latched (a toggle that stays on, an open menu's caret): 16% toward `text`.
    pub fn latched_on(&self, s: Color32) -> Color32 {
        mix(s, self.text, LATCHED)
    }

    /// The hover layer as translucent `text`, for a control that doesn't know the surface under
    /// it; over a surface it looks as `hover_on` of it.
    pub fn hover_layer(&self) -> Color32 {
        layer(self.text, HOVER)
    }

    /// The pressed layer, translucent as `hover_layer`.
    pub fn pressed_layer(&self) -> Color32 {
        layer(self.text, PRESSED)
    }

    /// The latched layer, translucent as `hover_layer`.
    pub fn latched_layer(&self) -> Color32 {
        layer(self.text, LATCHED)
    }

    /// Tone `t`'s colour: accent, or the success, warning, danger or info status colour.
    pub fn tone(&self, t: Tone) -> Color32 {
        match t {
            Tone::Accent => self.accent,
            Tone::Success => self.status.added,
            Tone::Warning => self.attention.warn,
            Tone::Danger => self.status.removed,
            Tone::Info => self.status.info,
        }
    }

    /// Tone `t`'s tint: (a soft opaque fill of it, its colour pushed to read at 4.5:1 there).
    pub fn tone_tint(&self, t: Tone) -> (Color32, Color32) {
        contrast::change_chip(self.tone(t), self)
    }

    /// The tonal button's fill: the accent tint.
    pub fn tonal_fill(&self) -> Color32 {
        self.tone_tint(Tone::Accent).0
    }

    /// The tonal button's text on `fill` (its rest, hover or pressed fill): the accent, pushed
    /// until it reads at 4.5:1 (7:1 in a high-contrast palette).
    pub fn tonal_ink(&self, fill: Color32) -> Color32 {
        let min = if contrast::ratio(self.text, self.panel) >= HIGH_CONTRAST_TEXT { INK_MIN_HC } else { INK_MIN };
        contrast::readable_on(self.accent, fill, self.dark, min)
    }

    /// `fill` shaded `t` of the way toward white on dark palettes, or black on light ones (a
    /// filled button's hover and press).
    pub fn shade_fill(&self, fill: Color32, t: f32) -> Color32 {
        mix(fill, if self.dark { Color32::WHITE } else { Color32::BLACK }, t)
    }

    /// The tonal fill hovered or pressed: moved toward the accent.
    pub fn tonal_state(&self, pressed: bool) -> Color32 {
        mix(self.tonal_fill(), self.accent, if pressed { FILL_PRESSED } else { FILL_HOVER })
    }
}

/// `c` at `amount` opacity, premultiplied in gamma space as egui blends, so over a surface it
/// looks as `mix(surface, c, amount)`.
fn layer(c: Color32, amount: f32) -> Color32 {
    c.gamma_multiply(amount)
}

/// Declares an accessor on the active palette for each `Palette` method named.
macro_rules! active {
    ($($name:ident),* $(,)?) => {
        $(
            #[doc = concat!("[`Palette::", stringify!($name), "`] of the active palette.")]
            pub fn $name() -> Color32 {
                color::with(Palette::$name)
            }
        )*
    };
}

active!(field_fill, guide_fill, tool_backing, tool_hover, tool_press, tool_pressed, inset, hover_layer);
