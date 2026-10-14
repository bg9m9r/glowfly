//! The whole-file view's "N unchanged lines" row: a folded run of unchanged lines that opens in
//! place when clicked.

use super::DiffView;
use crate::msg::Msg;
use crate::ui::theme::color;
use eframe::egui::{Align2, CursorIcon, Rect, Sense, Stroke, Ui, WidgetInfo, WidgetType, pos2};
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

    // The unfold mark: a small chevron pointing up above one pointing down.
    let k = v.style.k;
    let c = pos2(rect.left() + sign_x + 4.0 * k, rect.center().y);
    let s = Stroke::new(1.4_f32, color::weak());
    for dir in [-1.0, 1.0] {
        let tip = pos2(c.x, c.y + dir * 4.5 * k);
        for side in [-1.0, 1.0] {
            p.line_segment([tip, pos2(c.x + side * 3.5 * k, c.y + dir * 1.5 * k)], s);
        }
    }

    let x = (rect.left() + text_x).max(ui.clip_rect().left() + 8.0);
    p.text(pos2(x, rect.center().y), Align2::LEFT_CENTER, &text, v.style.font.clone(), color::weak());

    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    if resp.on_hover_text("Show these lines").clicked() {
        msgs.push(Msg::ExpandUnchanged(f.key));
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn label_counts_lines() {
        assert_eq!(super::label(1), "1 unchanged line");
        assert_eq!(super::label(11), "11 unchanged lines");
    }
}
