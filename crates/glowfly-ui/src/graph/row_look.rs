//! A graph row's background: a faint band in its node's lane colour (none on a WIP row), the hovered branch's
//! highlight, and a search match's accent bar along the left edge (the selection's bar, when
//! selected). The selection (a fill with that bar) and the lifted look of a hovered row come
//! from `widgets::row_fill`. Also the inks of a row a search dims.

use crate::ui::theme::color::{self, Palette};
use crate::ui::theme::surfaces::{band_color, highlight_color};
use crate::ui::widgets::row_fill::ACCENT_BAR_W;
use eframe::egui::{Color32, Painter, Rect, vec2};

/// How one row looks.
#[derive(Clone, Copy)]
pub(super) struct RowLook {
    /// The node's lane colour.
    pub lane: Color32,
    /// A WIP row (this tab's or another worktree's): no band, the row's background shows.
    pub wip: bool,
    /// The row belongs to the hovered branch.
    pub highlighted: bool,
    pub selected: bool,
    /// The graph has the keyboard focus (or nothing has): the selection shows as focused.
    pub active: bool,
    pub hovered: bool,
    /// The row matches a search that highlights (rather than filters) the matches.
    pub marked: bool,
}

/// Paints the row's background in `rect`, under everything else in the row.
pub(super) fn paint(p: &Painter, rect: Rect, look: RowLook) {
    let pal = color::current();
    let band = band(look, &pal);
    p.rect_filled(rect, 0.0, band);
    if look.highlighted {
        p.rect_filled(rect, 0.0, highlight_color(band, look.lane));
    }
    crate::ui::widgets::row_fill::paint_in(p, rect, look.selected, look.active, look.hovered);
    if look.marked && !look.selected {
        p.rect_filled(Rect::from_min_size(rect.left_top(), vec2(ACCENT_BAR_W, rect.height())), 0.0, pal.accent);
    }
}

/// The row's top fill, under its text: the selection's, the hover's, the branch highlight's or
/// the band (the surface a search's find fill is worked out on).
pub(super) fn top_fill(look: RowLook) -> Color32 {
    let pal = color::current();
    let band = band(look, &pal);
    match look {
        RowLook { selected: true, active: true, .. } => pal.selected,
        RowLook { selected: true, .. } => crate::ui::theme::tokens::selected_inactive(&pal),
        RowLook { hovered: true, .. } => pal.hover,
        RowLook { highlighted: true, .. } => highlight_color(band, look.lane),
        _ => band,
    }
}

/// The row's band: its lane's, or the background on a WIP row.
fn band(look: RowLook, pal: &Palette) -> Color32 {
    if look.wip { pal.bg } else { band_color(look.lane, pal) }
}

/// Whether a commit row matches the search (`searching`: there is one, and this is a commit
/// row; `at`: where it is among the matches) and whether the search dims it: a row that doesn't
/// match while the search highlights, unless it is selected.
pub(super) fn found(searching: bool, filtered: bool, selected: bool, at: impl FnOnce() -> Option<usize>) -> (bool, bool) {
    let matched = searching && at().is_some();
    (matched, searching && !filtered && !selected && !matched)
}

/// How opaque a row a search dims draws its lane lines and node.
pub(super) const DIM_ALPHA: f32 = 0.35;

/// The message and metadata colours of a row a search dims: `weak` and `faint`, opaque, so both
/// still read at 4.5:1 (checked in `theme/text_floor_tests.rs`).
pub(crate) fn dim_inks(pal: &Palette) -> (Color32, Color32) {
    (pal.weak, pal.faint)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::branch_colours::{line_in, swatches};
    use crate::ui::theme::contrast::ratio;
    use crate::ui::theme::{DARK, LIGHT, builtin};
    use glowfly_core::settings::BuiltinTheme;

    #[test]
    fn band_keeps_text_readable() {
        let mut failures = Vec::new();
        for t in BuiltinTheme::ALL {
            let p = builtin(t);
            for lane in swatches() {
                let band = band_color(line_in(lane, p), p);
                let (weak, text) = (ratio(p.weak, band), ratio(p.text, band));
                if band.a() != 255 || weak < 4.5 || text < ratio(p.text, p.bg).min(7.0) {
                    failures.push(format!("{t:?} {lane:?}: weak {weak:.2}:1, text {text:.2}:1, alpha {}", band.a()));
                }
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
        for p in [DARK, LIGHT] {
            for lane in swatches() {
                assert_ne!(band_color(line_in(lane, &p), &p), p.bg, "{lane:?}: the band shows on the standard palettes");
            }
        }
    }
}
