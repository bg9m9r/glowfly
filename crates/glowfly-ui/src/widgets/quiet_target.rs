//! A quiet target that a view lays out and paints itself (a repository tab, the tab list's
//! button, the title bar's menu button, a left-panel header's + and ⋯): its state this frame
//! (`State`), then the hover, pressed or latched layer over its whole shape and the inset focus
//! ring on the same corners, through `button_look::paint_target`, the one place a state
//! becomes pixels.

use super::button_look::{Press, Target, paint_target};
use crate::theme::sizes::ControlSize;
use eframe::egui::{CornerRadius, Painter, Rect, Response};

/// What a quiet target is doing this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct State {
    pub hovered: bool,
    /// The pointer is held down on it.
    pub down: bool,
    /// It stays on: its menu or popup is open.
    pub latched: bool,
    pub focused: bool,
}

impl State {
    /// The state of the target `r`, `latched` while its menu is open.
    pub fn of(r: &Response, latched: bool) -> Self {
        Self { hovered: r.hovered(), down: r.is_pointer_button_down_on(), latched, focused: r.has_focus() }
    }

    /// Whether it shows a layer (hovered, held or latched): its glyph then takes `strong` ink.
    pub fn lit(self) -> bool {
        self.hovered || self.down || self.latched
    }
}

/// Paints quiet target `rect` with `corners`, of `size` (its focus style and least hit area),
/// in state `s`: nothing at rest, the hover, pressed or latched layer over the whole shape
/// otherwise, and the inset focus ring on its corners while it has the keyboard focus.
pub fn paint(p: &Painter, rect: Rect, corners: impl Into<CornerRadius>, size: ControlSize, s: State) {
    let target = Target { rect, corners: corners.into(), edge: None };
    paint_target(p, target, size, Press::of(s.hovered, s.down, s.latched), s.focused);
}
