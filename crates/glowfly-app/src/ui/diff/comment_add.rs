//! Starting a code comment from a pull request file's diff: the painted comment button on the
//! hovered line, at its visible right edge (left of a thread marker), disabled with the reason
//! when the line can't take one; and the marks of the unposted draft, an accent bar on its
//! lines and a marker on its last line that focuses the comment box.

use super::DiffView;
use crate::msg::Msg;
use crate::pull_request::PullRequestMsg;
use crate::pull_request::line_comment::LineCommentMsg;
use crate::state::pull_request_line_comment::{ONE_HUNK, OUTSIDE, line_blocked};
use crate::ui::theme::color;
use crate::ui::widgets::{Icon, paint_icon};
use eframe::egui::{self, CursorIcon, Id, Rect, Sense, Ui, pos2, vec2};
use glowfly_core::diff_rows::{LineSide, line_index, line_keys};
use glowfly_core::forge::line_key;
use glowfly_core::repo::DiffLine;

/// The width of the button and of the draft marker.
const W: f32 = 18.0;
/// The draft bar's width.
const BAR: f32 = 3.0;

/// The comment button of line `dl`, drawn while the pointer is over `row` (a row in inline, a
/// cell in split, `cell` its side), ending `taken` px left of `right` (the visible right edge)
/// so it sits beside the markers there. `in_hunks` is false for a gap line filled in between
/// hunks, which the service's diff doesn't have.
#[allow(clippy::too_many_arguments)]
pub(super) fn button(
    ui: &Ui,
    v: &DiffView,
    row: Rect,
    right: f32,
    row_h: f32,
    dl: &DiffLine,
    cell: Option<LineSide>,
    in_hunks: bool,
    taken: f32,
    msgs: &mut Vec<Msg>,
) {
    let (Some(c), Some((side, n))) = (v.comments, line_key(dl)) else { return };
    if !ui.rect_contains_pointer(row) {
        return;
    }
    let shift = ui.input(|i| i.modifiers.shift);
    // Where the line is in the normal hunks, which the selection and the draft count in.
    let at = line_index(v.diff, side, n);
    let selected = at.is_some_and(|(h, i)| v.sel.is_some_and(|s| s.contains(v.target, h, i)));
    let why: Option<&str> = if let Some(b) = &c.blocked {
        Some(b)
    } else if !in_hunks || line_blocked(v.diff, dl).is_some() {
        // A filled-in gap line, or a whole-file line none of the normal hunks shows.
        Some(OUTSIDE)
    } else if shift && c.draft_hunk.is_some_and(|h| at.is_some_and(|(a, _)| a != h)) {
        Some(ONE_HUNK)
    } else {
        None
    };
    let enabled = why.is_none();
    let r = Rect::from_min_size(pos2(right - 6.0 - taken - W, row.top()), vec2(W, row_h));
    let tag = match cell {
        Some(LineSide::Old) => 1,
        Some(LineSide::New) => 2,
        None => 0,
    };
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let resp = ui.interact(r, Id::new(("comment-add", tag, side, n)), sense);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, "Comment on this line"));
    let p = ui.painter().with_clip_rect(row.intersect(ui.clip_rect()));
    let col = if !enabled {
        color::faint()
    } else if resp.hovered() {
        p.rect_filled(r, 3.0, color::hover());
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        color::accent()
    } else {
        color::weak()
    };
    paint_icon(&p, pos2(r.left() + 9.0, r.center().y), Icon::CommentAdd, col);
    let tip = if selected { "Comment on the selected lines" } else { "Comment on this line" };
    let resp = resp.on_hover_text(why.unwrap_or(tip));
    if enabled && resp.clicked() {
        msgs.push(Msg::PullRequest(PullRequestMsg::LineComment(LineCommentMsg::Start { side, line: n, extend: shift })));
    }
}

/// The draft's marks on line `dl` drawn in `row` (`cell`: its side in split): the accent bar
/// when the line is one of the draft's, and the draft marker on its last line, `taken` px left
/// of `right`. Returns the width the marker took (0 without one).
#[allow(clippy::too_many_arguments)]
pub(super) fn draft_marks(
    ui: &Ui,
    v: &DiffView,
    row: Rect,
    right: f32,
    row_h: f32,
    dl: &DiffLine,
    cell: Option<LineSide>,
    taken: f32,
    msgs: &mut Vec<Msg>,
) -> f32 {
    let Some(c) = v.comments else { return 0.0 };
    let keys: Vec<(LineSide, u32)> = line_keys(dl).into_iter().flatten().collect();
    if !keys.iter().any(|k| c.draft.contains(k)) {
        return 0.0;
    }
    let p = ui.painter().with_clip_rect(row.intersect(ui.clip_rect()));
    p.rect_filled(Rect::from_min_size(row.min, vec2(BAR, row.height())), 0.0, color::accent());
    let end = c.end.filter(|e| keys.contains(e) && cell.is_none_or(|s| s == e.0));
    if end.is_none() {
        return 0.0;
    }
    let r = Rect::from_min_size(pos2(right - 6.0 - taken - W, row.top()), vec2(W, row_h));
    let resp = ui.interact(r, Id::new(("draft-mark", cell.is_some())), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Your unposted comment"));
    p.rect_filled(r.shrink2(vec2(0.0, 1.0)), 3.0, if resp.hovered() { color::hover() } else { color::selected() });
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    paint_icon(&p, pos2(r.left() + 9.0, r.center().y), Icon::CommentAdd, color::accent());
    if resp.on_hover_text("Go to your unposted comment").clicked() {
        msgs.push(Msg::PullRequest(PullRequestMsg::LineComment(LineCommentMsg::Focus)));
    }
    W + 2.0
}
