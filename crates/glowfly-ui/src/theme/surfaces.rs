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
use super::tokens::{DANGER_HOVER, DANGER_QUIET_HOVER, DANGER_QUIET_PRESSED, FILL_HOVER, FILL_PRESSED, HOVER, LATCHED, PRESSED};
use eframe::egui::Color32;

/// How far a text field's fill moves from `input` toward the accent.
const FIELD_TINT: f32 = 0.04;
/// How far a dialog's side pane moves from `panel` toward the accent.
const GUIDE_TINT: f32 = 0.07;
/// How far the toolbar groups' backing moves from `toolbar` toward the accent.
const TOOL_TINT: f32 = 0.09;
/// How far a light palette's framed toolbar bar edge moves from `border` toward `text`, so it
/// reads on the tool backing.
const TOOL_EDGE: f32 = 0.22;
/// How far the dark overlay moves from `raised` toward `text`: one step lighter.
const OVERLAY_STEP: f32 = 0.06;
/// How far the dark overlay's edge moves from the overlay toward `text`.
const OVERLAY_EDGE: f32 = 0.12;
/// The least contrast of the tonal ink on its fills, and in a high-contrast palette (one whose
/// text reaches `HIGH_CONTRAST_TEXT` on the panel).
const INK_MIN: f64 = 4.5;
const INK_MIN_HC: f64 = 7.0;
const HIGH_CONTRAST_TEXT: f64 = 18.0;
/// How far the find fill (a search's matched text) starts from its surface toward the accent, on
/// dark and light palettes, and the least it backs off to while it keeps text readable.
const FIND_T_DARK: f32 = 0.24;
const FIND_T_LIGHT: f32 = 0.18;
const FIND_T_MIN: f32 = 0.10;
/// The most the find fill leans in to stand `FIND_APART` from its surface (a light lane's
/// highlight, already a step toward the accent's hue).
const FIND_T_MAX: f32 = 0.30;
const FIND_APART: f64 = 1.25;

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
        contrast::tint(self.input, self.accent, FIELD_TINT, self.dark)
    }

    /// The fill of a dialog's side pane (`dialog_frame`'s guide): `panel` tinted toward the
    /// accent.
    pub fn guide_fill(&self) -> Color32 {
        contrast::tint(self.panel, self.accent, GUIDE_TINT, self.dark)
    }

    /// The fill behind a search's matched text on surface `under` (the row's top fill): `under`
    /// tinted toward the accent, backed off 1% at a time (to `FIND_T_MIN`) until `text` and
    /// `strong` read on it at 4.5:1 (7:1 in a high-contrast palette); where it would then sit
    /// under `FIND_APART` from `under`, leaned in 1% at a time (to `FIND_T_MAX`) while they still
    /// read. Opaque, so it never stacks.
    pub fn find_fill(&self, under: Color32) -> Color32 {
        let min = if self.high_contrast { INK_MIN_HC } else { INK_MIN };
        let start = if self.dark { FIND_T_DARK } else { FIND_T_LIGHT };
        let at = |t: f32| contrast::tint(under, self.accent, t, self.dark);
        let reads = |f: Color32| contrast::ratio(self.text, f) >= min && contrast::ratio(self.strong, f) >= min;
        let pct = |t: f32| (t * 100.0).round() as u32;
        let backed = (pct(FIND_T_MIN)..=pct(start)).rev().find(|&i| reads(at(i as f32 / 100.0))).unwrap_or(pct(FIND_T_MIN));
        let apart = |i: u32| contrast::ratio(at(i as f32 / 100.0), under) >= FIND_APART;
        let lean = (backed..=pct(FIND_T_MAX)).take_while(|&i| i == backed || reads(at(i as f32 / 100.0))).find(|&i| apart(i));
        at(lean.unwrap_or(backed) as f32 / 100.0)
    }

    /// The neutral backing of the toolbar's groups: `toolbar` with a quiet accent tint (on Dusk,
    /// a violet step: `contrast::tint` keeps it in the indigo family).
    pub fn tool_backing(&self) -> Color32 {
        contrast::tint(self.toolbar, self.accent, TOOL_TINT, self.dark)
    }

    /// The edge and dividers of a framed toolbar bar (the pickers) at rest: `border` on dark
    /// palettes; on light ones `border` 22% toward `text`, since `border` all but vanishes on the
    /// tool backing there.
    pub fn tool_edge(&self) -> Color32 {
        if self.dark { self.border } else { mix(self.border, self.text, TOOL_EDGE) }
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

    /// A destructive menu item's hover pill: the overlay's hover pill tinted `DANGER_HOVER`
    /// toward the danger colour (`contrast::tint`, so a dark field keeps its own hue).
    pub fn danger_hover(&self) -> Color32 {
        contrast::tint(self.hover_on(self.overlay()), self.tone(Tone::Danger), DANGER_HOVER, self.dark)
    }

    /// A destructive menu item's label: the danger colour, pushed until it reads at 4.5:1 (7:1
    /// in a high-contrast palette) on the overlay and on `danger_hover`. (`danger` itself is a
    /// button fill that carries white text, too dark or light to be text.)
    pub fn menu_danger(&self) -> Color32 {
        let min = if contrast::ratio(self.text, self.panel) >= HIGH_CONTRAST_TEXT { INK_MIN_HC } else { INK_MIN };
        let on_hover = contrast::readable_on(self.tone(Tone::Danger), self.danger_hover(), self.dark, min);
        contrast::readable_on(on_hover, self.overlay(), self.dark, min)
    }

    /// A quiet danger button's text (a hunk's Discard): the danger tone, pushed until it reads
    /// at 4.5:1 (7:1 in a high-contrast palette) on the panel and the hunk band, at rest and
    /// under its hover and pressed washes. (`danger` is a fill, too dark or light to be text.)
    pub fn danger_ink(&self) -> Color32 {
        let min = if contrast::ratio(self.text, self.panel) >= HIGH_CONTRAST_TEXT { INK_MIN_HC } else { INK_MIN };
        let mut ink = self.tone(Tone::Danger);
        // Every field is on the same side of the pole, so pushing for one never undoes another;
        // a second pass settles any that an earlier push left short.
        for _ in 0..2 {
            for s in self.danger_quiet_fields() {
                ink = contrast::readable_on(ink, s, self.dark, min);
            }
        }
        ink
    }

    /// What a quiet danger button's text sits on: the panel and the hunk band, bare and under
    /// its hover and pressed washes (blended as egui paints them).
    fn danger_quiet_fields(&self) -> Vec<Color32> {
        let mut fields = Vec::new();
        for s in [self.panel, self.hunk_bg] {
            fields.push(s);
            for t in [DANGER_QUIET_HOVER, DANGER_QUIET_PRESSED] {
                fields.push(over(self.wash(self.danger, t), s));
            }
        }
        fields
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

    /// The colour a wash of `c` at `t` is painted in: `c` (a brown one `contrast::clear`ed
    /// first, so a light theme's amber `warn` washes peach, not tan), unless over one of the
    /// palette's surfaces it would read brown (`contrast::brownish`: a warm wash on a dark
    /// field, or a strong one on paper); then a colour of `c`'s lightness in the background's
    /// own hue and chroma, so the wash lifts the surface without tinting it and the colour stays
    /// on the text, edge or mark beside it (docs/COLOUR.md, "No brown").
    fn wash_colour(&self, c: Color32, t: f32) -> Color32 {
        let c = contrast::clear(c);
        let surfaces = [self.bg, self.panel, self.toolbar, self.raised, self.input];
        if !surfaces.iter().any(|&s| contrast::brownish(mix(s, c, t))) {
            return c;
        }
        contrast::field_at(self.bg, contrast::oklch(c).0)
    }

    /// `c` washed over surface `s` at `t` (0 to 1): the opaque colour `wash` shows there.
    #[cfg(test)]
    pub(crate) fn wash_on(&self, s: Color32, c: Color32, t: f32) -> Color32 {
        mix(s, self.wash_colour(c, t), t)
    }

    /// A translucent wash of `c` at `t` (a badge's, a cell's or a selection's fill), for a view
    /// that paints it over the surface under it. Never `c.gamma_multiply(t)` on a status,
    /// accent or repository colour: that is how a gold or amber turns brown on a dark field.
    pub fn wash(&self, c: Color32, t: f32) -> Color32 {
        self.wash_colour(c, t).gamma_multiply(t)
    }

    /// `fill` shaded `t` of the way toward white on dark palettes, or black on light ones (a
    /// filled button's hover and press).
    pub fn shade_fill(&self, fill: Color32, t: f32) -> Color32 {
        mix(fill, if self.dark { Color32::WHITE } else { Color32::BLACK }, t)
    }

    /// The tonal fill hovered or pressed: moved toward the accent (`contrast::unbrown`ed).
    pub fn tonal_state(&self, pressed: bool) -> Color32 {
        let fill = self.tonal_fill();
        contrast::unbrown(fill, mix(fill, self.accent, if pressed { FILL_PRESSED } else { FILL_HOVER }))
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

active!(
    field_fill,
    guide_fill,
    tool_backing,
    tool_edge,
    tool_hover,
    tool_press,
    tool_pressed,
    inset,
    hover_layer,
    pressed_layer,
    danger_hover,
    menu_danger
);

/// [`Palette::find_fill`] in the active palette.
pub fn find_fill(under: Color32) -> Color32 {
    color::with(|p| p.find_fill(under))
}

/// [`Palette::wash`] in the active palette.
pub fn wash(c: Color32, t: f32) -> Color32 {
    color::with(|p| p.wash(c, t))
}

/// Premultiplied `top` laid over the opaque `under`, byte by byte in gamma space as egui blends.
pub(crate) fn over(top: Color32, under: Color32) -> Color32 {
    let a = f32::from(top.a()) / 255.0;
    let ch = |t: u8, u: u8| (f32::from(t) + f32::from(u) * (1.0 - a)).round() as u8;
    Color32::from_rgb(ch(top.r(), under.r()), ch(top.g(), under.g()), ch(top.b(), under.b()))
}

/// How far a graph row's band moves from the background toward its lane colour, at most.
const BAND_T: f32 = 0.07;

/// The opaque band for a graph row on a `lane`: the strongest tint (`contrast::tint`, so on Dusk
/// a warm lane gives a plum-violet step, never a grey-brown) up to `BAND_T` that keeps `weak`
/// text at 4.5:1, `text` as readable as on the background (up to 7:1) and `faint` text as
/// readable as on the background (up to 4.5:1). The background itself when no tint does.
pub(crate) fn band_color(lane: Color32, pal: &Palette) -> Color32 {
    let steps = (BAND_T * 100.0).round() as u32;
    let keeps = |band: Color32| {
        contrast::ratio(pal.weak, band) >= 4.5
            && contrast::ratio(pal.text, band) >= contrast::ratio(pal.text, pal.bg).min(7.0)
            && contrast::ratio(pal.faint, band) >= contrast::ratio(pal.faint, pal.bg).min(4.5)
    };
    (1..=steps).rev().map(|i| contrast::tint(pal.bg, lane, i as f32 / 100.0, pal.dark)).find(|&b| keeps(b)).unwrap_or(pal.bg)
}

/// How far a highlighted row (its branch hovered) moves from its band toward the lane colour.
const HIGHLIGHT_T: f32 = 0.16;

/// The opaque fill of a highlighted row on `band`, a row of a `lane`: the band moved toward the
/// lane colour, `contrast::unbrown`ed (a warm lane on a dark palette lifts the band instead).
pub(crate) fn highlight_color(band: Color32, lane: Color32) -> Color32 {
    contrast::unbrown(band, mix(band, lane, HIGHLIGHT_T))
}

thread_local! {
    /// The last palette's row surfaces: worked out once, not for every chip drawn.
    static ROW_SURFACES: std::cell::RefCell<Option<(Palette, Vec<Color32>)>> = const { std::cell::RefCell::new(None) };
}

/// Calls `f` with every surface a graph row's ref chip can sit on in `p`: the background, the
/// hover, the selection with and without the focus, and the band of every lane in both branch
/// presets and the trunk's, plain and highlighted (its branch hovered) (docs/COLOUR.md, "Ref
/// chips"). Worked out once per palette.
pub fn with_row_surfaces<R>(p: &Palette, f: impl FnOnce(&[Color32]) -> R) -> R {
    ROW_SURFACES.with(|cache| {
        if cache.borrow().as_ref().is_none_or(|(c, _)| c != p) {
            *cache.borrow_mut() = Some((*p, row_surfaces(p)));
        }
        f(&cache.borrow().as_ref().expect("just filled").1)
    })
}

fn row_surfaces(p: &Palette) -> Vec<Color32> {
    let mut rows = vec![p.bg, p.hover, p.selected, super::tokens::selected_inactive(p)];
    let lanes = p.branch.lanes.iter().chain(&p.colour_blind.lanes).copied().chain([super::branch_colours::trunk(p)]);
    for lane in lanes {
        let band = band_color(lane, p);
        for c in [band, highlight_color(band, lane)] {
            if !rows.contains(&c) {
                rows.push(c);
            }
        }
    }
    rows
}
