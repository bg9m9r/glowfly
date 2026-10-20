//! Line counts drawn the same everywhere: a file row's `+N −M` with its bar, a total, and the
//! WIP row's status chips. The counts and letters carry the status colours (added is
//! `added_color`, removed `removed_color`, modified `modified_color`, as the status letters
//! are); the bar is neutral: the added share in `weak`, the removed share in `faint`.

use super::common::{added_color, modified_color, removed_color};
use super::theme::{color, contrast};
use super::widgets;
use crate::ui::theme::type_scale;
use eframe::egui::{Context, CornerRadius, FontId, Painter, Pos2, Rect, RichText, Sense, Ui, pos2, vec2};
use trellis_core::repo::{ChangeCounts, LineCount};

/// A file row's bar: its width and height.
pub(super) const BAR_W: f32 = 36.0;
const BAR_H: f32 = 4.0;
/// The least a non-zero change fills, so a one-line change still shows.
const BAR_MIN: f32 = 2.0;
/// Between the counts and the bar, and between two chips.
const GAP: f32 = 6.0;
/// The graph's total, painted.
const TOTAL_PX: f32 = type_scale::SMALL;

/// The `+N` and `−M` texts (U+2212 for the minus).
fn texts(c: LineCount) -> (String, String) {
    (format!("+{}", c.added), format!("\u{2212}{}", c.removed))
}

/// The green and red lengths of a change of `count` in a list whose largest is `largest`: the
/// filled share of `BAR_W` (at least `BAR_MIN` when not zero), split by added and removed.
pub(super) fn bar_fill(count: LineCount, largest: usize) -> (f32, f32) {
    let total = count.total();
    if total == 0 || largest == 0 {
        return (0.0, 0.0);
    }
    let filled = (BAR_W * total as f32 / largest.max(total) as f32).max(BAR_MIN);
    let green = filled * count.added as f32 / total as f32;
    (green, filled - green)
}

/// A file row's end, in a right-to-left `ui`: the bar at the edge, then `−M` and `+N` as labels
/// (a side that is zero left out). Nothing at all for a change of no lines.
pub(super) fn row_end(ui: &mut Ui, count: LineCount, largest: usize) {
    if count.total() == 0 {
        return;
    }
    let (rect, _) = ui.allocate_exact_size(vec2(BAR_W, BAR_H), Sense::hover());
    paint_bar(ui.painter(), rect, bar_fill(count, largest));
    ui.add_space((GAP - ui.spacing().item_spacing.x).max(0.0));
    let (added, removed) = texts(count);
    if count.removed > 0 {
        ui.label(RichText::new(removed).size(type_scale::SMALL).color(removed_color()));
    }
    if count.added > 0 {
        ui.label(RichText::new(added).size(type_scale::SMALL).color(added_color()));
    }
}

/// A file row's end, in a right-to-left `ui`: the bar alone (the counts follow the path,
/// `inline`). Nothing for a change of no lines.
pub(super) fn bar_end(ui: &mut Ui, count: LineCount, largest: usize) {
    if count.total() == 0 {
        return;
    }
    let (rect, _) = ui.allocate_exact_size(vec2(BAR_W, BAR_H), Sense::hover());
    paint_bar(ui.painter(), rect, bar_fill(count, largest));
}

/// The `+N` and `−M` labels of `count` that show (a zero side left out).
fn inline_parts(count: LineCount) -> impl Iterator<Item = (String, eframe::egui::Color32)> {
    let (added, removed) = texts(count);
    [(count.added > 0).then(|| (added, added_color())), (count.removed > 0).then(|| (removed, removed_color()))].into_iter().flatten()
}

/// A file's counts beside its path, in a left-to-right `ui`: `+N` then `−M`, `GAP` apart, a
/// zero side left out. Nothing at all for a change of no lines. Takes `inline_width`.
pub(super) fn inline(ui: &mut Ui, count: LineCount) {
    if count.total() == 0 {
        return;
    }
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.x = GAP;
        for (text, col) in inline_parts(count) {
            ui.label(RichText::new(text).size(type_scale::SMALL).color(col));
        }
    });
}

/// The width `inline` takes.
pub(super) fn inline_width(ctx: &Context, count: LineCount) -> f32 {
    let w = |t: String| ctx.fonts(|f| f.layout_no_wrap(t, type_scale::font(type_scale::SMALL), color::text()).size().x);
    let widths: Vec<f32> = inline_parts(count).map(|(t, _)| w(t)).collect();
    widths.iter().sum::<f32>() + GAP * widths.len().saturating_sub(1) as f32
}

fn paint_bar(p: &Painter, rect: Rect, (green, red): (f32, f32)) {
    let round = CornerRadius::same(2);
    p.rect_filled(rect, round, color::border());
    if green + red <= 0.0 {
        return;
    }
    let filled = Rect::from_min_size(rect.min, vec2(green + red, rect.height()));
    p.rect_filled(filled, round, color::faint());
    if green > 0.0 {
        // Square on the right where red follows it.
        let r = if red > 0.0 { CornerRadius { ne: 0, se: 0, ..round } } else { round };
        p.rect_filled(Rect::from_min_size(rect.min, vec2(green, rect.height())), r, color::weak());
    }
}

/// A total, in a right-to-left `ui`: `+N` then `−M`, both always.
pub(super) fn total(ui: &mut Ui, count: LineCount) {
    let (added, removed) = texts(count);
    ui.label(RichText::new(removed).size(type_scale::SMALL).color(removed_color()));
    ui.label(RichText::new(added).size(type_scale::SMALL).color(added_color()));
}

/// The width `paint_total_right` takes.
pub(super) fn total_width(ctx: &Context, count: LineCount) -> f32 {
    let (added, removed) = texts(count);
    let w = |t: String| ctx.fonts(|f| f.layout_no_wrap(t, FontId::proportional(TOTAL_PX), color::text()).size().x);
    w(added) + GAP + w(removed)
}

/// Paints `+N −M` with its right end at `right` (centred on `right.y`); returns where it went.
pub(super) fn paint_total_right(p: &Painter, right: Pos2, count: LineCount) -> Rect {
    let (added, removed) = texts(count);
    let font = FontId::proportional(TOTAL_PX);
    let minus = p.layout_no_wrap(removed, font.clone(), removed_color());
    let plus = p.layout_no_wrap(added, font, added_color());
    let x = right.x - minus.size().x;
    let top = |h: f32| right.y - h / 2.0;
    let left = x - GAP - plus.size().x;
    let rect = Rect::from_min_max(pos2(left, top(plus.size().y)), pos2(right.x, top(plus.size().y) + plus.size().y));
    p.galley(pos2(left, top(plus.size().y)), plus, added_color());
    p.galley(pos2(x, top(minus.size().y)), minus, removed_color());
    rect
}

/// The chips' letters, counts and colours, zeros left out: modified, added, deleted.
fn chip_parts(c: ChangeCounts) -> impl Iterator<Item = (String, eframe::egui::Color32)> {
    [('M', c.modified, modified_color()), ('A', c.added, added_color()), ('D', c.deleted, removed_color())]
        .into_iter()
        .filter(|(_, n, _)| *n > 0)
        .map(|(letter, n, col)| (format!("{letter} {n}"), col))
}

/// Paints the WIP row's chips from `x`, centred on `mid`; returns their right edge (`x` when
/// there are none).
pub(super) fn paint_chips(p: &Painter, x: f32, mid: f32, c: ChangeCounts) -> f32 {
    let palette = color::current();
    let mut right = x;
    for (i, (text, col)) in chip_parts(c).enumerate() {
        let start = if i == 0 { right } else { right + GAP };
        let (fill, fg) = contrast::change_chip(col, &palette);
        right = start + widgets::chip(p, start, mid, &text, fill, fg);
    }
    right
}

/// The width `paint_chips` takes.
pub(super) fn chips_width(ctx: &Context, c: ChangeCounts) -> f32 {
    let widths: Vec<f32> = chip_parts(c)
        .map(|(text, _)| ctx.fonts(|f| f.layout_no_wrap(text, type_scale::font(type_scale::CAPTION), color::text()).size().x) + 12.0)
        .collect();
    widths.iter().sum::<f32>() + GAP * widths.len().saturating_sub(1) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(added: usize, removed: usize) -> LineCount {
        LineCount { added, removed }
    }

    #[test]
    fn bar_fill_splits_by_share() {
        assert_eq!(bar_fill(n(0, 0), 10), (0.0, 0.0));
        assert_eq!(bar_fill(n(30, 10), 40), (27.0, 9.0), "the largest fills the bar");
        assert_eq!(bar_fill(n(10, 0), 40), (9.0, 0.0));
        let (g, r) = bar_fill(n(0, 1), 1000);
        assert_eq!((g, r), (0.0, BAR_MIN), "a small change still shows");
        let (g, r) = bar_fill(n(5, 5), 4);
        assert_eq!(g + r, BAR_W, "never past the bar");
    }

    /// How far `inline(count)` moves the cursor of a left-to-right row with no spacing of its
    /// own, and what `inline_width` says it takes.
    fn advance(count: LineCount) -> (f32, f32) {
        let ctx = Context::default();
        let mut out = (0.0, 0.0);
        let _ = ctx.run(eframe::egui::RawInput::default(), |ctx| {
            eframe::egui::CentralPanel::default().show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    let x0 = ui.cursor().left();
                    inline(ui, count);
                    out = (ui.cursor().left() - x0, inline_width(ui.ctx(), count));
                });
            });
        });
        out
    }

    #[test]
    fn inline_counts_take_the_width_they_say() {
        for count in [n(3, 0), n(3, 1), n(0, 12), n(1200, 34)] {
            let (moved, said) = advance(count);
            assert!((moved - said).abs() < 0.5, "{count:?}: moved {moved}, said {said}");
        }
        assert!(advance(n(3, 0)).1 < advance(n(3, 1)).1, "a zero side is left out");
        assert_eq!(advance(n(0, 0)), (0.0, 0.0), "no change draws nothing");
    }
}
