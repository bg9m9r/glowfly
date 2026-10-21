//! The conflict view's folds, drawn as the whole-file diff draws its own: the "N unchanged
//! lines" row of a folded clean run, which opens it in the panes and the output alike, and the
//! fold-back control on the first line of an opened run in the panes' checkbox gutter.

use super::conflict_cells::CHECK_W;
use super::diff::fold::paint_chevrons;
use super::diff::{ROW_H, mono};
use super::theme::color;
use crate::msg::{ConflictMsg, Msg};
use crate::ui::theme::tokens::radius;
use eframe::egui::{Align2, CornerRadius, CursorIcon, Rect, Sense, Stroke, Ui, WidgetInfo, WidgetType, pos2, vec2};
use trellis_core::conflict_folds::CleanFold;
use trellis_core::i18n::Lang;

/// The row's text for a fold of `n` lines.
fn label(t: Lang, n: usize) -> String {
    t.trn("conflict.fold.label", n, &[])
}

/// Draws fold `f` across the row `rect`: a quiet fill between hairlines, the unfold mark and
/// the label, pinned to the visible left edge when scrolled sideways. A click opens it.
pub(super) fn fold_row(ui: &mut Ui, t: Lang, rect: Rect, f: CleanFold, msgs: &mut Vec<Msg>) {
    let text = label(t, f.len());
    let resp = ui.interact(rect, ui.id().with(("conflict-fold", f.seg)), Sense::click());
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, &text));
    let p = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    p.rect_filled(rect, 0.0, if resp.hovered() { color::hover() } else { color::panel() });
    let hairline = Stroke::new(1.0_f32, color::border());
    p.hline(rect.x_range(), rect.top() + 0.5, hairline);
    p.hline(rect.x_range(), rect.bottom() - 0.5, hairline);
    let left = rect.left().max(ui.clip_rect().left());
    // The unfold mark: chevrons pointing apart.
    paint_chevrons(&p, pos2(left + 12.0, rect.center().y), 1.0, Stroke::new(1.4_f32, color::weak()), true);
    p.text(pos2(left + 26.0, rect.center().y), Align2::LEFT_CENTER, &text, mono(), color::weak());
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    if resp.on_hover_text(t.tr("conflict.fold.show.tip")).clicked() {
        msgs.push(Msg::Conflict(ConflictMsg::ConflictFold { seg: f.seg, open: true }));
    }
}

/// The fold-back control for opened fold `f`, in the checkbox gutter of `row` (its first line):
/// chevrons pointing together, highlighted on hover. A click folds the run again.
pub(super) fn fold_back(ui: &mut Ui, t: Lang, row: Rect, f: CleanFold, msgs: &mut Vec<Msg>) {
    let r =
        Rect::from_center_size(pos2(row.left() + 2.0 + (CHECK_W - 2.0) / 2.0, row.top() + ROW_H / 2.0), vec2(CHECK_W - 2.0, ROW_H - 2.0));
    let resp = ui.interact(r, ui.id().with(("conflict-fold-back", f.seg)), Sense::click());
    let text = t.trn("conflict.fold.back.button", f.len(), &[]);
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, &text));
    let p = ui.painter().with_clip_rect(r.intersect(ui.clip_rect()));
    let hovered = resp.hovered();
    if hovered {
        p.rect_filled(r, CornerRadius::same(radius::SM), color::hover());
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let ink = if hovered { color::text() } else { color::faint() };
    paint_chevrons(&p, r.center(), 1.0, Stroke::new(1.4_f32, ink), false);
    let tip = t.trn("conflict.fold.back.tip", f.len(), &[]);
    if resp.on_hover_text(tip).clicked() {
        msgs.push(Msg::Conflict(ConflictMsg::ConflictFold { seg: f.seg, open: false }));
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn label_counts_lines() {
        use trellis_core::i18n::Lang;
        assert_eq!(super::label(Lang::En, 1), "1 unchanged line");
        assert_eq!(super::label(Lang::En, 12), "12 unchanged lines");
        assert_eq!(super::label(Lang::De, 12), "12 unveränderte Zeilen");
    }
}
