//! This tab's own WIP row: "Uncommitted changes", the M / A / D chips and, right-aligned, the
//! working tree's total `+N −M`. The row has no commit, author or date, so it spans the message
//! column and the empty columns after it, up to the table's inset (`Columns::wip_span`). Short
//! of room (`fit`), the total hides first, then the label is cut down to `LABEL_KEEP`
//! characters, then the chips hide, and only then is the label cut further.

use super::layout::Columns;
use crate::line_counts;
use crate::theme::color;
use crate::theme::type_scale;
use crate::widgets::paint_cut_text;
use eframe::egui::{Painter, pos2};
use glowfly_core::repo::{ChangeCounts, LineCount};

/// Between the label and the chips.
const LABEL_GAP: f32 = 10.0;
/// The least space between the chips and the total; closer, the total is hidden.
const TOTAL_GAP: f32 = 12.0;
/// The characters of the label the chips leave it before they hide.
const LABEL_KEEP: usize = 6;

/// The label's colour: quiet until the row is chosen.
fn label_ink(selected: bool) -> eframe::egui::Color32 {
    if selected { color::text() } else { color::weak() }
}

/// What the row shows in its room.
#[derive(Debug, PartialEq)]
struct Fit {
    chips: bool,
    total: bool,
    /// The most the label may take; it is cut only past this.
    label_max: f32,
}

/// Fits the row into `room` px: the total goes first (it shows only beside the whole label and
/// the chips), then the label is cut down to `kept_w` (its first `LABEL_KEEP` characters and
/// "…", or the whole label when shorter), then the chips go, and only then is the label cut
/// further. `chips_w` is 0 when there are none; `total_w` is None while the total isn't known.
fn fit(room: f32, (label_w, kept_w): (f32, f32), chips_w: f32, total_w: Option<f32>) -> Fit {
    let chips_room = if chips_w > 0.0 { LABEL_GAP + chips_w } else { 0.0 };
    let total = total_w.is_some_and(|t| label_w + chips_room + TOTAL_GAP + t <= room);
    let chips = chips_w > 0.0 && kept_w.min(label_w) + chips_room <= room;
    let label_max = if chips { room - chips_room } else { room };
    Fit { chips, total, label_max: label_max.max(0.0) }
}

/// The width of `label`'s first `LABEL_KEEP` characters and "…" (the whole label when it is no
/// longer), measured as `measure` lays text out.
fn kept_width(label: &str, measure: impl Fn(String) -> f32) -> f32 {
    if label.chars().count() <= LABEL_KEEP {
        return measure(label.to_owned());
    }
    measure(label.chars().take(LABEL_KEEP).chain(['…']).collect())
}

/// Draws the row's cells, centred on `mid`: the label `weak`, or `text` while the row is drawn
/// `selected` (chosen, `RepoSession::wip_drawn_selected`). `total` is None while the counts
/// aren't known (or are incomplete).
pub(super) fn draw(
    mp: &Painter,
    cols: &Columns,
    mid: f32,
    (label, selected): (&str, bool),
    changes: ChangeCounts,
    total: Option<LineCount>,
) {
    let ctx = mp.ctx();
    let font = type_scale::font(type_scale::BODY);
    // The span already ends at the table's inset.
    let (left, right) = cols.wip_span();
    let measure = |s: String| ctx.fonts(|f| f.layout_no_wrap(s, font.clone(), color::text()).size().x);
    let widths = (measure(label.to_owned()), kept_width(label, measure));
    let chips_w = line_counts::chips_width(ctx, changes);
    let fits = fit(right - left, widths, chips_w, total.map(|t| line_counts::total_width(ctx, t)));
    let cut = paint_cut_text(mp, pos2(left, mid), label, font, label_ink(selected), fits.label_max);
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
    use crate::widgets::truncate::{cut_galley, cut_job};
    use eframe::egui::{self, Rect, vec2};
    use glowfly_core::i18n::Lang;
    use glowfly_core::settings::{ColumnWidths, GraphColumns};

    // Roughly "Uncommitted changes" (and "Uncomm…"), three chips and "+38 −1" at the row's sizes.
    const LABEL: (f32, f32) = (125.0, 55.0);
    const CHIPS: f32 = 110.0;
    const TOTAL: f32 = 45.0;
    const SNUG: f32 = LABEL.0 + LABEL_GAP + CHIPS;

    #[test]
    fn everything_fits_in_a_roomy_row() {
        assert_eq!(fit(400.0, LABEL, CHIPS, Some(TOTAL)), Fit { chips: true, total: true, label_max: 400.0 - LABEL_GAP - CHIPS });
    }

    #[test]
    fn the_total_goes_first_then_the_label_is_cut_to_six_characters_then_the_chips_go() {
        let whole = fit(SNUG + TOTAL_GAP + TOTAL, LABEL, CHIPS, Some(TOTAL));
        assert!(whole.total && whole.chips);
        // One px short of the total: it hides, the label stays whole beside the chips.
        assert!(!fit(SNUG + TOTAL_GAP + TOTAL - 1.0, LABEL, CHIPS, Some(TOTAL)).total);
        assert_eq!(fit(SNUG, LABEL, CHIPS, Some(TOTAL)), Fit { chips: true, total: false, label_max: LABEL.0 });
        // Narrower: the chips stay and the label is cut, down to its six characters.
        let cut = fit(SNUG - 40.0, LABEL, CHIPS, Some(TOTAL));
        assert_eq!(cut, Fit { chips: true, total: false, label_max: LABEL.0 - 40.0 });
        let kept = LABEL.1 + LABEL_GAP + CHIPS;
        assert_eq!(fit(kept, LABEL, CHIPS, None), Fit { chips: true, total: false, label_max: LABEL.1 });
        // Past that the chips hide and the label takes the whole room again.
        assert_eq!(fit(kept - 1.0, LABEL, CHIPS, None), Fit { chips: false, total: false, label_max: kept - 1.0 });
        assert_eq!(fit(30.0, LABEL, CHIPS, Some(TOTAL)), Fit { chips: false, total: false, label_max: 30.0 });
    }

    #[test]
    fn no_chips_or_no_total() {
        assert_eq!(fit(200.0, LABEL, 0.0, Some(TOTAL)), Fit { chips: false, total: true, label_max: 200.0 });
        assert_eq!(fit(400.0, LABEL, CHIPS, None), Fit { chips: true, total: false, label_max: 400.0 - LABEL_GAP - CHIPS });
        assert_eq!(fit(-5.0, LABEL, CHIPS, None).label_max, 0.0);
    }

    /// The label cut to `kept_width` keeps exactly its first six characters, in English and in
    /// German (whose label is longer), and a short label is kept whole.
    #[test]
    fn the_kept_label_shows_six_characters() {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |_| {});
        let font = type_scale::font(type_scale::BODY);
        let measure = |s: String| ctx.fonts(|f| f.layout_no_wrap(s, font.clone(), color::text()).size().x);
        for lang in [Lang::En, Lang::De] {
            let label = lang.tr("graph.wip.label");
            let g = cut_galley(&ctx, cut_job(label, font.clone(), color::text()), kept_width(label, measure));
            let shown: String = g.rows.iter().flat_map(|r| r.glyphs.iter().map(|gl| gl.chr)).collect();
            let want: String = label.chars().take(LABEL_KEEP).chain(['…']).collect();
            assert_eq!(shown, want, "{lang:?}");
        }
        assert_eq!(kept_width("WIP", measure), measure("WIP".into()));
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
        assert_eq!(right, cols.inner.1);
        assert!(right >= cols.date.1 && right >= cols.commit.1);
        assert!(cols.msg.1 - cols.msg.0 < SNUG);
        let room = right - left;
        assert_eq!(fit(room, LABEL, CHIPS, Some(TOTAL)), Fit { chips: true, total: true, label_max: room - LABEL_GAP - CHIPS });
    }

    /// The row drawn alone in a strip whose room for it is `room(kept, chips)` px (the label's
    /// six-character floor and the chips' width), in English: at the floor the label shows
    /// "Uncomm…" beside its chips; a px less, the chips hide and the label takes the room.
    #[test]
    fn graph_wip_row_floor() {
        use crate::snapshot_check::CheckSnapshot;
        let label = Lang::En.tr("graph.wip.label");
        let changes = ChangeCounts { modified: 2, added: 2, deleted: 1 };
        let total = Some(LineCount { added: 38, removed: 1 });
        let floor = |kept: f32, chips: f32| kept + LABEL_GAP + chips;
        for (name, less) in [("graph_wip_row_floor", 0.0), ("graph_wip_row_floor_no_chips", 1.0)] {
            let mut h = crate::snapshot_check::builder().with_size(vec2(360.0, 40.0)).build_ui(|ui| {
                crate::theme::apply_theme(ui.ctx());
                let ctx = ui.ctx().clone();
                let font = type_scale::font(type_scale::BODY);
                let measure = |s: String| ctx.fonts(|f| f.layout_no_wrap(s, font.clone(), color::text()).size().x);
                let room = floor(kept_width(label, measure), line_counts::chips_width(&ctx, changes)) - less;
                let cols = |w: f32| {
                    let shown = GraphColumns { refs: false, author: false, commit: false, date: false, ..GraphColumns::default() };
                    Columns::new(Rect::from_min_size(ui.max_rect().min, vec2(w, 40.0)), 20.0, &shown, &ColumnWidths::default())
                };
                let span = |c: &Columns| c.wip_span().1 - c.wip_span().0;
                let first = cols(300.0);
                let cols = cols(300.0 + room - span(&first));
                assert!((span(&cols) - room).abs() < 0.01, "{} for {room}", span(&cols));
                ui.painter().rect_filled(ui.max_rect(), 0.0, color::bg());
                draw(ui.painter(), &cols, ui.max_rect().center().y, (label, false), changes, total);
            });
            h.run();
            h.check_snapshot(name);
        }
    }
}
