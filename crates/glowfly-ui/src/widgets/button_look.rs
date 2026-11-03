//! The colours of the button family (`buttons.rs`, `button_split.rs`) in each state, from the
//! active palette's tokens (`theme::surfaces`): filled kinds (primary, secondary, danger) shade
//! their fill toward white on dark palettes and black on light ones, the tonal kind moves its
//! accent tint toward the accent and recomputes its ink, the quiet kind lays the translucent
//! state layers over whatever is under it (the ghost controls share that look through
//! `paint_target`), and the quiet danger kind has danger text over a danger wash. `paint_quiet`
//! is left only for the tab glyphs' small squares (`tab_glyph.rs`).
//!
//! Every target's state becomes pixels here. `paint_target` paints a quiet target (the
//! toolbar's buttons, splits and segment bars, ghost controls, quiet chips, segmented options):
//! the hover, pressed or latched layer over the target's own shape, a single framed control's
//! border going `weak` while lit, and the focus ring on the same corners. `paint_button` paints a
//! button of any kind (`buttons.rs`, either half of `button_split.rs`): its look's fill on its
//! own shape and, when it stands alone, its edge. The focus ring (`paint_focus`) is inset for a
//! quiet or packed target (a split's half) and every Compact one, on the target's exact
//! corners, and outside the edge of a framed one that stands alone.

use super::buttons::Kind;
use crate::ui::theme::color::{self, Palette};
use crate::ui::theme::sizes::ControlSize;
use crate::ui::theme::tokens::{self, DANGER_QUIET_HOVER, DANGER_QUIET_PRESSED, FILL_HOVER, FILL_PRESSED, HIT_MIN};
use eframe::egui::{Color32, CornerRadius, Painter, Rect, Stroke, StrokeKind};

/// A button's interaction state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Press {
    Rest,
    Hover,
    /// The pointer is held down on it.
    Pressed,
    /// It stays on (an open menu's caret).
    Latched,
}

impl Press {
    /// The state of a control `hovered`, held `down` and `latched`: held first, then latched,
    /// then hovered.
    pub(super) fn of(hovered: bool, down: bool, latched: bool) -> Self {
        if down {
            Self::Pressed
        } else if latched {
            Self::Latched
        } else if hovered {
            Self::Hover
        } else {
            Self::Rest
        }
    }
}

/// A button's fill, text and edge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Look {
    pub fill: Color32,
    pub ink: Color32,
    pub edge: Stroke,
}

/// The look of a `kind` button, `enabled` or not, in state `press`, in the active palette.
pub(super) fn look(kind: Kind, enabled: bool, press: Press) -> Look {
    look_in(&color::current(), kind, enabled, press)
}

fn look_in(p: &Palette, kind: Kind, enabled: bool, press: Press) -> Look {
    let edge = |c: Color32| Stroke::new(1.0_f32, c);
    if !enabled {
        return match kind {
            Kind::Quiet | Kind::DangerQuiet => Look { fill: Color32::TRANSPARENT, ink: p.faint, edge: Stroke::NONE },
            _ => Look { fill: p.raised, ink: p.weak, edge: Stroke::NONE },
        };
    }
    let shade = |fill: Color32| match press {
        Press::Rest => fill,
        Press::Hover => p.shade_fill(fill, FILL_HOVER),
        Press::Pressed | Press::Latched => p.shade_fill(fill, FILL_PRESSED),
    };
    match kind {
        Kind::Primary => Look { fill: shade(p.primary), ink: p.on_primary, edge: edge(p.primary_border) },
        Kind::Secondary => Look { fill: shade(p.panel), ink: p.text, edge: edge(p.input_border) },
        Kind::Danger => Look { fill: shade(p.danger), ink: Color32::WHITE, edge: Stroke::NONE },
        Kind::Tonal => {
            let fill = match press {
                Press::Rest => p.tonal_fill(),
                Press::Hover => p.tonal_state(false),
                Press::Pressed | Press::Latched => p.tonal_state(true),
            };
            Look { fill, ink: p.tonal_ink(fill), edge: Stroke::NONE }
        }
        Kind::Quiet => {
            let (fill, ink) = quiet(p, press);
            Look { fill, ink, edge: Stroke::NONE }
        }
        Kind::DangerQuiet => {
            let fill = match press {
                Press::Rest => Color32::TRANSPARENT,
                Press::Hover => p.wash(p.danger, DANGER_QUIET_HOVER),
                Press::Pressed | Press::Latched => p.wash(p.danger, DANGER_QUIET_PRESSED),
            };
            Look { fill, ink: p.danger_ink(), edge: Stroke::NONE }
        }
    }
}

/// The quiet look's (fill, ink): no fill at rest, then the hover, pressed or latched layer;
/// `text` at rest and `strong` otherwise.
fn quiet(p: &Palette, press: Press) -> (Color32, Color32) {
    match press {
        Press::Rest => (Color32::TRANSPARENT, p.text),
        Press::Hover => (p.hover_layer(), p.strong),
        Press::Pressed => (p.pressed_layer(), p.strong),
        Press::Latched => (p.latched_layer(), p.strong),
    }
}

/// `fill` with the state layer for `press` over it: a control that keeps its own fill in every
/// state (a split's caret while its action is disabled, or tinted with a tone).
pub(super) fn layer_on(fill: Color32, press: Press) -> Color32 {
    let p = color::current();
    match press {
        Press::Rest => fill,
        Press::Hover => p.hover_on(fill),
        Press::Pressed => p.pressed_on(fill),
        Press::Latched => p.latched_on(fill),
    }
}

/// Paints the quiet look's fill for `press` at `rect` (nothing at rest), for a glyph that paints
/// its own small square (`tab_glyph.rs`).
pub(super) fn paint_quiet(p: &Painter, rect: Rect, r: impl Into<CornerRadius>, press: Press) {
    let (fill, _) = quiet(&color::current(), press);
    if fill != Color32::TRANSPARENT {
        p.rect_filled(rect, r, fill);
    }
}

/// A target's shape: its rect (its hit area too, except a Compact one's: `ghost::ghost_at` grows
/// its own hit area, while a quiet chip still hits on its 22 px until chips take a size), its own
/// corners (a segment's square where it meets a neighbour) and, for a single
/// framed control, its border: `paint_target` paints that stroke, `paint_button` the look's edge.
/// A target without one is quiet or packed (a split's half, framed as a whole by its caller).
#[derive(Clone, Copy, Debug)]
pub(super) struct Target {
    pub rect: Rect,
    pub corners: CornerRadius,
    pub edge: Option<Stroke>,
}

/// Paints target `t` of size `size` in state `press`, `focus`ed or not: the quiet state layer
/// over its whole shape (nothing at rest), its border (`weak` while lit) when it is framed, and
/// the focus ring on its corners (`paint_focus`). The size only picks the focus style and checks
/// the hit area; it never paints.
pub(super) fn paint_target(p: &Painter, t: Target, size: ControlSize, press: Press, focus: bool) {
    debug_assert!(t.edge.is_none() || size.allows_frame(), "a Compact control is quiet only");
    // A Compact control paints less than its hit area: `ghost::ghost_at` grows its own, and a
    // quiet chip still hits on its 22 px until chips take a size.
    debug_assert!(
        size == ControlSize::Compact || (t.rect.width() >= HIT_MIN - 0.5 && t.rect.height() >= HIT_MIN - 0.5),
        "a target smaller than the hit area: {:?}",
        t.rect
    );
    let pal = color::current();
    let (fill, _) = quiet(&pal, press);
    let edge = t.edge.map(|edge| if press == Press::Rest { edge } else { Stroke::new(edge.width, pal.weak) });
    paint_face(p, t.rect, t.corners, fill, edge);
    paint_focus(p, t, size, focus, InsetRing::Accent);
}

/// Paints target `t`, a button of size `size`, in `look` (`look` for its state, or a look the
/// caller adjusted from it), `focus`ed or not: the fill over its own shape, the look's edge when
/// `t` stands alone (`t.edge` is `Some`), and the focus ring on its corners. Debug builds check
/// that a button of any size but Compact is at least `HIT_MIN` on each side.
pub(super) fn paint_button(p: &Painter, t: Target, size: ControlSize, look: Look, focus: bool) {
    // As in `paint_target`: a Compact button paints less than its hit area.
    debug_assert!(
        size == ControlSize::Compact || (t.rect.width() >= HIT_MIN - 0.5 && t.rect.height() >= HIT_MIN - 0.5),
        "a button smaller than the hit area: {:?}",
        t.rect
    );
    match t.edge {
        // One shape, so the fill and its edge share their anti-aliased corners.
        Some(_) => {
            p.rect(t.rect, t.corners, look.fill, look.edge, StrokeKind::Inside);
        }
        None => paint_face(p, t.rect, t.corners, look.fill, None),
    }
    paint_focus(p, t, size, focus, InsetRing::Accent);
}

/// How a packed button's inset focus ring is drawn: in the accent, or on an enabled primary
/// fill (the accent is the primary fill in most palettes) in the primary ink, edged in Strong
/// by that `fill` (`tokens::focus_ring_inset_in`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum InsetRing {
    Accent,
    OnPrimary { fill: Color32 },
}

impl InsetRing {
    /// The ring of a half filled with `fill`, `on_primary_fill` or not.
    pub(super) fn on(on_primary_fill: bool, fill: Color32) -> Self {
        if on_primary_fill { Self::OnPrimary { fill } } else { Self::Accent }
    }
}

/// Paints `fill` (none when transparent) and `edge` inside `rect` on `corners`.
fn paint_face(p: &Painter, rect: Rect, corners: CornerRadius, fill: Color32, edge: Option<Stroke>) {
    if fill != Color32::TRANSPARENT {
        p.rect_filled(rect, corners, fill);
    }
    if let Some(edge) = edge.filter(|e| e.width > 0.0) {
        p.rect_stroke(rect, corners, edge, StrokeKind::Inside);
    }
}

/// Paints target `t`'s focus ring when it is `focus`ed: 2 px outside the edge of a framed target
/// that stands alone, else inset on its exact corners in `inset` (the accent, or a primary
/// fill's ink), so a packed target's ring never reaches its neighbour.
pub(super) fn paint_focus(p: &Painter, t: Target, size: ControlSize, focus: bool, inset: InsetRing) {
    if !focus {
        return;
    }
    if t.edge.is_some() && size.allows_frame() {
        tokens::focus_ring(p, t.rect, t.corners);
    } else {
        match inset {
            InsetRing::Accent => tokens::focus_ring_inset(p, t.rect, t.corners),
            InsetRing::OnPrimary { fill } => tokens::focus_ring_inset_in(p, t.rect, t.corners, color::current().on_primary, fill),
        }
    }
}

#[cfg(test)]
pub(super) fn look_for(p: &Palette, kind: Kind, enabled: bool, press: Press) -> Look {
    look_in(p, kind, enabled, press)
}
