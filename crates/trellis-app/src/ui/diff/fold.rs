//! The whole-file view's folds: the "N unchanged lines" row of a folded run of unchanged lines,
//! which opens in place when clicked, and the fold-back control on the first line of an opened
//! run, which folds it into that row again.

use super::DiffView;
use crate::msg::DiffMsg;
use crate::msg::Msg;
use crate::ui::theme::color;
use eframe::egui::{Align2, CornerRadius, CursorIcon, Painter, Pos2, Rect, Sense, Stroke, Ui, WidgetInfo, WidgetType, pos2, vec2};
use trellis_core::whole_file::Fold;

/// The row's text for a fold of `n` lines.
fn label(n: usize) -> String {
    if n == 1 { "1 unchanged line".to_owned() } else { format!("{n} unchanged lines") }
}

/// Draws fold `f` across the full row `rect`: a quiet fill, the unfold mark in the sign column
/// (at `sign_x` from the row's left) and the label in the text column (at `text_x`), pinned to
/// the visible left edge when scrolled sideways. A click asks to show the lines.
pub(super) fn fold_row(ui: &mut Ui, rect: Rect, v: &DiffView, f: Fold, sign_x: f32, text_x: f32, msgs: &mut Vec<Msg>) {
    let text = label(f.len());
    let resp = ui.interact(rect, ui.id().with(("diff-fold", f.key)), Sense::click());
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, &text));
    let p = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    p.rect_filled(rect, 0.0, if resp.hovered() { color::hover() } else { color::panel() });
    let hairline = Stroke::new(1.0_f32, color::border());
    p.hline(rect.x_range(), rect.top() + 0.5, hairline);
    p.hline(rect.x_range(), rect.bottom() - 0.5, hairline);

    // The unfold mark: chevrons pointing apart.
    let k = v.style.k;
    let c = pos2(rect.left() + sign_x + 4.0 * k, rect.center().y);
    paint_chevrons(&p, c, k, Stroke::new(1.4_f32, color::weak()), true);

    let x = (rect.left() + text_x).max(ui.clip_rect().left() + 8.0);
    p.text(pos2(x, rect.center().y), Align2::LEFT_CENTER, &text, v.style.font.clone(), color::weak());

    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    if resp.on_hover_text("Show these lines").clicked() {
        msgs.push(Msg::Diff(DiffMsg::ExpandUnchanged(f.key)));
    }
}

/// A small chevron above one below, centred on `c` at scale `k`: tips pointing away from the
/// centre when `apart` (unfold), towards it otherwise (fold back). Painted, since the bundled
/// fonts lack such glyphs.
fn paint_chevrons(p: &Painter, c: Pos2, k: f32, stroke: Stroke, apart: bool) {
    let (tip, arms) = if apart { (4.5, 1.5) } else { (1.5, 4.5) };
    for dir in [-1.0, 1.0] {
        let t = pos2(c.x, c.y + dir * tip * k);
        for side in [-1.0, 1.0] {
            p.line_segment([t, pos2(c.x + side * 3.5 * k, c.y + dir * arms * k)], stroke);
        }
    }
}

/// The opened fold that starts at whole-file line `line`, if any. `opened` is in file order.
pub(super) fn opened_at(opened: &[Fold], line: usize) -> Option<Fold> {
    opened.binary_search_by_key(&line, |f| f.start).ok().map(|i| opened[i])
}

/// The fold-back control on the first line of opened fold `f`, in the sign column (at `sign_x`
/// from the left of `row`, where the fold row paints its unfold mark): chevrons pointing
/// together, highlighted on hover. A click folds the run into one row again.
pub(super) fn fold_back(ui: &mut Ui, row: Rect, v: &DiffView, f: Fold, sign_x: f32, msgs: &mut Vec<Msg>) {
    let k = v.style.k;
    let r = Rect::from_center_size(pos2(row.left() + sign_x + 4.0 * k, row.center().y), vec2(14.0 * k, v.style.row_h - 2.0));
    let resp = ui.interact(r, ui.id().with(("diff-fold-back", f.key)), Sense::click());
    let text = format!("Fold {} unchanged lines", f.len());
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, &text));
    let p = ui.painter().with_clip_rect(r.intersect(ui.clip_rect()));
    let hovered = resp.hovered();
    if hovered {
        p.rect_filled(r, CornerRadius::same(3), color::hover());
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let ink = if hovered { color::text() } else { color::faint() };
    paint_chevrons(&p, r.center(), k, Stroke::new(1.4_f32, ink), false);
    let tip = format!("Fold these {} unchanged lines back into one row", f.len());
    if resp.on_hover_text(tip).clicked() {
        msgs.push(Msg::Diff(DiffMsg::CollapseUnchanged(f.key)));
    }
}

#[cfg(test)]
mod tests {
    use trellis_core::whole_file::Fold;

    #[test]
    fn label_counts_lines() {
        assert_eq!(super::label(1), "1 unchanged line");
        assert_eq!(super::label(11), "11 unchanged lines");
    }

    #[test]
    fn opened_at_finds_folds_by_start() {
        let f = |start: usize| Fold { start, end: start + 5, key: start as u32 };
        let opened = [f(2), f(20)];
        assert_eq!(super::opened_at(&opened, 20), Some(f(20)));
        assert_eq!(super::opened_at(&opened, 21), None, "only the first line");
        assert_eq!(super::opened_at(&opened, 3), None);
        assert_eq!(super::opened_at(&[], 2), None);
    }
}
