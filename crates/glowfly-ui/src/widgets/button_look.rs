//! The colours of the button family (`buttons.rs`, `button_split.rs`) in each state, from the
//! active palette's tokens (`theme::surfaces`): filled kinds (primary, secondary, danger) shade
//! their fill toward white on dark palettes and black on light ones, the tonal kind moves its
//! accent tint toward the accent and recomputes its ink, the quiet kind lays the translucent
//! state layers over whatever is under it (the ghost controls share that look through
//! `paint_quiet`), and the quiet danger kind has danger text over a danger wash.
//!
//! `paint_target` is where a target's state becomes pixels for the toolbar's buttons, splits
//! and segment bars (`tool.rs`, `segment_bar.rs`): the hover, pressed or latched layer over the
//! target's own shape, a single framed control's border going `weak` while lit, and the focus
//! ring on the same corners (inset for quiet and packed targets and Compact ones, outside for a
//! framed one).

use super::buttons::Kind;
use crate::ui::theme::color::{self, Palette};
use crate::ui::theme::sizes::ControlSize;
use crate::ui::theme::tokens::{self, FILL_HOVER, FILL_PRESSED, HIT_MIN};
use eframe::egui::{Color32, CornerRadius, Painter, Rect, Stroke, StrokeKind};

/// How far the quiet danger kind's fill moves toward the danger colour on hover and while held.
const DANGER_QUIET_HOVER: f32 = 0.12;
const DANGER_QUIET_PRESSED: f32 = 0.18;

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
            Look { fill, ink: p.danger, edge: Stroke::NONE }
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

/// Paints the quiet look's fill for `press` at `rect` (nothing at rest), for controls that draw
/// their own content (the ghost controls).
pub(super) fn paint_quiet(p: &Painter, rect: Rect, r: impl Into<CornerRadius>, press: Press) {
    let (fill, _) = quiet(&color::current(), press);
    if fill != Color32::TRANSPARENT {
        p.rect_filled(rect, r, fill);
    }
}

/// A target's shape: its rect (its hit area too), its own corners (a segment's square where it
/// meets a neighbour) and, for a single framed control, the border `paint_target` paints over
/// its layer.
#[derive(Clone, Copy, Debug)]
pub(super) struct Target {
    pub rect: Rect,
    pub corners: CornerRadius,
    pub edge: Option<Stroke>,
}

/// Paints target `t` of size `size` in state `press`, `focus`ed or not: the quiet state layer
/// over its whole shape (nothing at rest), its border (`weak` while lit) when it is framed, and
/// the focus ring on its corners: inset for a quiet or packed target and every Compact one,
/// outside its edge for a framed one. The size only picks the focus style and checks the hit
/// area; it never paints.
pub(super) fn paint_target(p: &Painter, t: Target, size: ControlSize, press: Press, focus: bool) {
    debug_assert!(t.edge.is_none() || size.allows_frame(), "a Compact control is quiet only");
    debug_assert!(t.rect.width() >= HIT_MIN - 0.5 && t.rect.height() >= HIT_MIN - 0.5, "a target smaller than the hit area: {:?}", t.rect);
    let pal = color::current();
    let (fill, _) = quiet(&pal, press);
    if fill != Color32::TRANSPARENT {
        p.rect_filled(t.rect, t.corners, fill);
    }
    if let Some(edge) = t.edge {
        let edge = if press == Press::Rest { edge } else { Stroke::new(edge.width, pal.weak) };
        p.rect_stroke(t.rect, t.corners, edge, StrokeKind::Inside);
    }
    if focus {
        if t.edge.is_some() && size.allows_frame() {
            tokens::focus_ring(p, t.rect, t.corners);
        } else {
            tokens::focus_ring_inset(p, t.rect, t.corners);
        }
    }
}

#[cfg(test)]
pub(super) fn look_for(p: &Palette, kind: Kind, enabled: bool, press: Press) -> Look {
    look_in(p, kind, enabled, press)
}
