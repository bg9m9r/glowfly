//! Code comment markers on a pull request file's diff lines: a painted speech bubble (and the
//! comment count when there are several) pinned to the visible right edge of the line, which
//! opens the thread in the comment pane when clicked.

use crate::msg::Msg;
use crate::pull_request::PullRequestMsg;
use crate::state::pull_request_threads::{Mark, Marks};
use crate::ui::theme::color;
use crate::ui::theme::type_scale;
use crate::ui::widgets::{Icon, paint_icon};
use eframe::egui::{self, Align2, CursorIcon, Id, Rect, Sense, Ui, pos2, vec2};
use trellis_core::diff_rows::{LineSide, line_keys};
use trellis_core::repo::DiffLine;

/// The marker for diff line `dl`: of any of its file lines, or only of the `side` file (one
/// cell of the split layout).
pub(super) fn find<'a>(m: &'a Marks, dl: &DiffLine, side: Option<LineSide>) -> Option<&'a Mark> {
    line_keys(dl).into_iter().flatten().filter(|(s, _)| side.is_none_or(|w| w == *s)).find_map(|k| m.get(&k))
}

/// Paints `mark` on the line drawn in `row`, ending at `right` (the visible right edge), on
/// its first visual line of height `row_h`.
pub(super) fn paint(ui: &Ui, row: Rect, right: f32, row_h: f32, mark: &Mark, msgs: &mut Vec<Msg>) {
    let w = if mark.count > 1 { 28.0 } else { 18.0 };
    let r = Rect::from_min_size(pos2(right - 6.0 - w, row.top()), vec2(w, row_h));
    let label = if mark.count == 1 { "1 comment on this line".to_owned() } else { format!("{} comments on this line", mark.count) };
    let resp = ui.interact(r, Id::new(("thread-mark", &mark.thread)), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &label));
    let col = if mark.resolved { color::weak() } else { color::accent() };
    let p = ui.painter().with_clip_rect(row.intersect(ui.clip_rect()));
    if resp.hovered() {
        p.rect_filled(r, 3.0, color::hover());
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let icon_x = r.left() + 9.0;
    paint_icon(&p, pos2(icon_x, r.center().y), Icon::Comment, col);
    if mark.count > 1 {
        p.text(pos2(icon_x + 8.0, r.center().y), Align2::LEFT_CENTER, mark.count.to_string(), type_scale::font(type_scale::CAPTION), col);
    }
    let resp = resp.on_hover_text("Show the comments on this line");
    if resp.clicked() {
        msgs.push(Msg::PullRequest(PullRequestMsg::FocusThread(mark.thread.clone())));
    }
}
