//! This tab's own WIP row: "Uncommitted changes", the M / A / D chips and, right-aligned, the
//! working tree's total `+N −M`. The row has no commit, author or date, so it spans the message
//! column and the empty columns after it (`Columns::wip_span`).

use super::paint::Columns;
use crate::ui::line_counts;
use crate::ui::theme::color;
use crate::ui::widgets::paint_cut_text;
use eframe::egui::{FontId, Painter, pos2};
use trellis_core::repo::{ChangeCounts, LineCount};

/// Between the label and the chips.
const LABEL_GAP: f32 = 10.0;
/// The total's inset from the row's right edge.
const TOTAL_INSET: f32 = 8.0;
/// The least space between the chips and the total; closer, the total is hidden.
const TOTAL_GAP: f32 = 12.0;

/// What the row shows in its room.
#[derive(Debug, PartialEq)]
struct Fit {
    chips: bool,
    total: bool,
    /// The most the label may take; it is cut only past this.
    label_max: f32,
}

/// Fits the row into `room` px, keeping the label whole for as long as it can: the total goes
/// first, then the chips, and only then is the label cut. `chips_w` is 0 when there are none;
/// `total_w` is None while the total isn't known.
fn fit(room: f32, label_w: f32, chips_w: f32, total_w: Option<f32>) -> Fit {
    let with_chips = if chips_w > 0.0 { label_w + LABEL_GAP + chips_w } else { label_w };
    let chips = with_chips <= room;
    let total = chips && total_w.is_some_and(|t| with_chips + TOTAL_GAP + t <= room);
    let label_max = if chips { room - (with_chips - label_w) } else { room };
    Fit { chips: chips && chips_w > 0.0, total, label_max: label_max.max(0.0) }
}

/// Draws the row's cells, centred on `mid`. `total` is None while the counts aren't known (or
/// are incomplete).
pub(super) fn draw(mp: &Painter, cols: &Columns, mid: f32, label: &str, changes: ChangeCounts, total: Option<LineCount>) {
    let ctx = mp.ctx();
    let font = FontId::proportional(13.0);
    let (left, end) = cols.wip_span();
    let right = end - TOTAL_INSET;
    let label_w = ctx.fonts(|f| f.layout_no_wrap(label.to_owned(), font.clone(), color::text()).size().x);
    let chips_w = line_counts::chips_width(ctx, changes);
    let fits = fit(right - left, label_w, chips_w, total.map(|t| line_counts::total_width(ctx, t)));
    let cut = paint_cut_text(mp, pos2(left, mid), label, font, color::text(), fits.label_max);
    if fits.chips {
        line_counts::paint_chips(mp, cut.rect.right() + LABEL_GAP, mid, changes);
    }
    if let (true, Some(t)) = (fits.total, total) {
        line_counts::paint_total_right(mp, pos2(right, mid), t);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Rect, vec2};
    use trellis_core::settings::{ColumnWidths, GraphColumns};

    // Roughly "Uncommitted changes", three chips and "+38 −1" at the row's sizes.
    const LABEL: f32 = 125.0;
    const CHIPS: f32 = 110.0;
    const TOTAL: f32 = 45.0;

    #[test]
    fn everything_fits_in_a_roomy_row() {
        assert_eq!(fit(400.0, LABEL, CHIPS, Some(TOTAL)), Fit { chips: true, total: true, label_max: 400.0 - LABEL_GAP - CHIPS });
    }

    #[test]
    fn the_total_goes_first_then_the_chips_and_the_label_is_cut_last() {
        let snug = LABEL + LABEL_GAP + CHIPS;
        assert_eq!(fit(snug + 20.0, LABEL, CHIPS, Some(TOTAL)), Fit { chips: true, total: false, label_max: LABEL + 20.0 });
        assert_eq!(fit(snug, LABEL, CHIPS, Some(TOTAL)), Fit { chips: true, total: false, label_max: LABEL });
        let narrow = fit(snug - 1.0, LABEL, CHIPS, Some(TOTAL));
        assert_eq!(narrow, Fit { chips: false, total: false, label_max: snug - 1.0 }, "the label stays whole");
        assert!(narrow.label_max >= LABEL);
        assert_eq!(fit(80.0, LABEL, CHIPS, Some(TOTAL)), Fit { chips: false, total: false, label_max: 80.0 });
    }

    #[test]
    fn no_chips_or_no_total() {
        assert_eq!(fit(200.0, LABEL, 0.0, Some(TOTAL)), Fit { chips: false, total: true, label_max: 200.0 });
        assert_eq!(fit(400.0, LABEL, CHIPS, None), Fit { chips: true, total: false, label_max: 400.0 - LABEL_GAP - CHIPS });
        assert_eq!(fit(-5.0, LABEL, CHIPS, None).label_max, 0.0);
    }

    /// At the reference window's graph panel width (about 740 px) with the default columns, the
    /// message column alone is too narrow for the label and chips, but the WIP row, spanning the
    /// empty columns after it, has room for its label, chips and total.
    #[test]
    fn the_row_spans_the_empty_columns_at_default_widths() {
        let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(740.0, 40.0));
        let cols = Columns::new(rect, 60.0, &GraphColumns::default(), &ColumnWidths::default());
        let (left, right) = cols.wip_span();
        assert_eq!(left, cols.msg.0);
        assert!(right >= cols.date.1 && right >= cols.commit.1);
        assert!(cols.msg.1 - cols.msg.0 < LABEL + LABEL_GAP + CHIPS);
        let room = right - TOTAL_INSET - left;
        assert_eq!(fit(room, LABEL, CHIPS, Some(TOTAL)), Fit { chips: true, total: true, label_max: room - LABEL_GAP - CHIPS });
    }
}
