//! A graph row's background: a faint band in its node's lane colour and the hovered branch's
//! highlight. The selection (a fill with an accent bar along the left edge) and the lifted look
//! of a hovered row come from `widgets::row_fill`.

use crate::ui::theme::color::{self, Palette};
use crate::ui::theme::contrast::{mix, ratio};
use eframe::egui::{Color32, Painter, Rect};

/// How far the band moves from the background toward the lane colour, at most.
const BAND_T: f32 = 0.07;

/// How one row looks.
pub(super) struct RowLook {
    /// The node's lane colour.
    pub lane: Color32,
    /// The row belongs to the hovered branch.
    pub highlighted: bool,
    pub selected: bool,
    pub hovered: bool,
}

/// Paints the row's background in `rect`, under everything else in the row.
pub(super) fn paint(p: &Painter, rect: Rect, look: RowLook) {
    let pal = color::current();
    p.rect_filled(rect, 0.0, band_color(look.lane, &pal));
    if look.highlighted {
        p.rect_filled(rect, 0.0, look.lane.gamma_multiply(0.16));
    }
    crate::ui::widgets::row_fill::paint(p, rect, look.selected, look.hovered);
}

/// The opaque band for a row on a `lane`: the strongest tint up to `BAND_T` that keeps `weak`
/// text at 4.5:1, `text` as readable as on the background (up to 7:1) and `faint` text as
/// readable as on the background (up to 4.5:1). The background itself when no tint does.
pub(in crate::ui) fn band_color(lane: Color32, pal: &Palette) -> Color32 {
    let steps = (BAND_T * 100.0).round() as u32;
    let keeps = |band: Color32| {
        ratio(pal.weak, band) >= 4.5
            && ratio(pal.text, band) >= ratio(pal.text, pal.bg).min(7.0)
            && ratio(pal.faint, band) >= ratio(pal.faint, pal.bg).min(4.5)
    };
    (1..=steps).rev().map(|i| mix(pal.bg, lane, i as f32 / 100.0)).find(|&b| keeps(b)).unwrap_or(pal.bg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::branch_colours::{line_in, swatches};
    use crate::ui::theme::{DARK, LIGHT, builtin};
    use trellis_core::settings::BuiltinTheme;

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
