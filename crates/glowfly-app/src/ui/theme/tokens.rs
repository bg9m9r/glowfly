//! The design tokens that aren't colours: the corner scale (`radius`), the state-layer amounts,
//! the two elevations (`Elevation`, `shadow`), the focus ring and the text field's halo, and the
//! motion timings (`motion`). Every theme reads the same tokens; the colour tokens worked out
//! from the palette (surfaces, layers, tones) are in `surfaces.rs`.
//!
//! Views name a token (`radius::MD`, `motion::STANDARD`) instead of a number, and
//! `tests/tokens.rs` checks that no view file puts a literal radius, a shadow or a hand-made
//! accent focus ring in its place.

use super::color::{self, Palette};
use eframe::egui::{Color32, Painter, Rect, Shadow, Stroke, StrokeKind};

/// The corner scale, in px. Containers use nested radii: the outer corner is the inner one
/// plus the padding between them (`around`), so a group's corners and its contents' stay
/// parallel.
pub mod radius {
    use eframe::egui::CornerRadius;

    /// Tiny marks: a progress segment, a count bar.
    pub const XS: u8 = 2;
    /// Checkboxes and the inner bits of a control (a hover fill inside a row, a key cap).
    pub const SM: u8 = 4;
    /// Controls: buttons, fields, ghost and tool buttons, segments, menu items.
    pub const MD: u8 = 6;
    /// Cards, menus, tooltips, toasts and the repository tabs' tops.
    pub const LG: u8 = 8;
    /// Dialogs.
    pub const XL: u8 = 12;

    /// The corner of a container `pad` px around something with `inner` corners.
    pub const fn around(inner: u8, pad: u8) -> u8 {
        inner.saturating_add(pad)
    }

    /// The corner of something `pad` px inside a container with `outer` corners.
    pub const fn inside(outer: u8, pad: u8) -> u8 {
        outer.saturating_sub(pad)
    }

    /// The corner that makes a shape `h` px tall a pill (half its height).
    pub fn pill(h: f32) -> u8 {
        (h / 2.0).round().clamp(0.0, 255.0) as u8
    }

    /// `r` on the top corners only (a tab, a band across a frame's top).
    pub const fn top(r: u8) -> CornerRadius {
        CornerRadius { nw: r, ne: r, sw: 0, se: 0 }
    }

    /// `r` on the left corners only (a split button's first half).
    pub const fn left(r: u8) -> CornerRadius {
        CornerRadius { nw: r, sw: r, ne: 0, se: 0 }
    }

    /// `r` on the right corners only (a split button's caret half).
    pub const fn right(r: u8) -> CornerRadius {
        CornerRadius { nw: 0, sw: 0, ne: r, se: r }
    }
}

/// State layers: how far a hovered, pressed or latched (a toggle that stays on, an open menu's
/// caret) control moves from its surface toward `text`.
pub const HOVER: f32 = 0.08;
pub const PRESSED: f32 = 0.12;
pub const LATCHED: f32 = 0.16;
/// How far a filled button's fill is shaded toward white (dark palettes) or black (light ones)
/// on hover and while pressed.
pub const FILL_HOVER: f32 = 0.10;
pub const FILL_PRESSED: f32 = 0.18;

/// How high something floats above the surface under it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Elevation {
    /// Menus, tooltips, popups and toasts.
    Float,
    /// Dialogs and the command palette.
    Dialog,
}

/// The shadow under something at elevation `e` in palette `p`. egui draws one layer, so each
/// elevation is a single soft shadow: Float 4 px down with a 16 px blur, Dialog 12 px down with
/// a 40 px blur, lighter on light palettes.
pub fn shadow(e: Elevation, p: &Palette) -> Shadow {
    let (offset, blur, dark, light) = match e {
        Elevation::Float => (4, 16, 64, 28),
        Elevation::Dialog => (12, 40, 120, 40),
    };
    Shadow { offset: [0, offset], blur, spread: 0, color: Color32::from_black_alpha(if p.dark { dark } else { light }) }
}

/// The focus ring's width and its gap outside a standalone control.
pub const FOCUS_W: f32 = 2.0;
pub const FOCUS_GAP: f32 = 2.0;
/// The inset ring of a control packed tightly in a row or group, where an outside ring would
/// reach its neighbour.
pub const FOCUS_INSET_W: f32 = 1.5;
/// A focused text field's soft halo outside its accent edge.
pub const HALO_W: f32 = 3.0;
pub const HALO_ALPHA: f32 = 0.22;

/// The focus ring of a standalone control at `rect` with `r` corners: 2 px of accent, 2 px
/// outside it, its corners following the control's.
pub fn focus_ring(p: &Painter, rect: Rect, r: u8) {
    let stroke = Stroke::new(FOCUS_W, color::accent());
    p.rect_stroke(rect.expand(FOCUS_GAP), radius::around(r, FOCUS_GAP as u8), stroke, StrokeKind::Outside);
}

/// The focus ring of a control packed in a row or group: 1.5 px of accent inside its edge.
pub fn focus_ring_inset(p: &Painter, rect: Rect, r: u8) {
    p.rect_stroke(rect, r, Stroke::new(FOCUS_INSET_W, color::accent()), StrokeKind::Inside);
}

/// A focused text field's halo: a soft accent ring just outside its 2 px accent edge.
pub fn focus_halo(p: &Painter, rect: Rect, r: u8) {
    let stroke = Stroke::new(HALO_W, color::accent().gamma_multiply(HALO_ALPHA));
    p.rect_stroke(rect.expand(1.0), radius::around(r, 1), stroke, StrokeKind::Outside);
}

/// Motion: how long things take, the one easing curve, and Reduce motion.
pub mod motion {
    use eframe::egui::Context;

    /// Small state changes: a tab's close × fading in.
    pub const QUICK: f32 = 0.09;
    /// Tooltips and menus fading and rising into place, tabs sliding.
    pub const STANDARD: f32 = 0.15;
    /// How far something rises as it appears; nothing moves further.
    pub const RISE: f32 = 4.0;

    /// The easing curve cubic-bezier(.2, .8, .2, 1): a quick start that settles gently, with no
    /// overshoot. `t` and the result are 0 to 1.
    pub fn ease(t: f32) -> f32 {
        const X1: f32 = 0.2;
        const Y1: f32 = 0.8;
        const X2: f32 = 0.2;
        const Y2: f32 = 1.0;
        if t <= 0.0 {
            return 0.0;
        }
        if t >= 1.0 {
            return 1.0;
        }
        let bezier = |s: f32, a: f32, b: f32| {
            let u = 1.0 - s;
            3.0 * u * u * s * a + 3.0 * u * s * s * b + s * s * s
        };
        // x(s) rises from 0 to 1, so halving finds the s where it reaches t.
        let (mut lo, mut hi) = (0.0_f32, 1.0_f32);
        for _ in 0..24 {
            let mid = (lo + hi) / 2.0;
            if bezier(mid, X1, X2) < t {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        bezier((lo + hi) / 2.0, Y1, Y2).clamp(0.0, 1.0)
    }

    /// Whether Reduce motion is on: `tooltip::configure` sets egui's animation time to 0 then.
    pub fn reduced(ctx: &Context) -> bool {
        ctx.style().animation_time == 0.0
    }

    /// `d` seconds, or 0 with Reduce motion.
    pub fn duration(d: f32, ctx: &Context) -> f32 {
        if reduced(ctx) { 0.0 } else { d }
    }

    /// egui's animation time: `STANDARD`, or 0 with Reduce motion.
    pub fn animation_time(reduce: bool) -> f32 {
        if reduce { 0.0 } else { STANDARD }
    }
}
