//! Other worktrees' WIP rows in the graph (a dashed node in its lane's colour joined to the
//! worktree's HEAD commit, a "// WIP · <folder>" box and the change counts). This tab's own
//! WIP row is drawn by `wip_row`.

use super::nodes;
use super::paint::{Columns, draw_lanes, lane_x, paint_pencil};
use crate::state::worktrees::wip_label;
use crate::ui::common::{added_color, lane_color, modified_color, removed_color};
use crate::ui::theme::color;
use eframe::egui::{CornerRadius, FontId, Painter, Rect, pos2, vec2};
use trellis_core::graph::WorktreeWipRow;
use trellis_core::repo::{ChangeCounts, WorktreeInfo};

/// The narrowest another worktree's WIP box is.
const MIN_BOX_W: f32 = 96.0;

/// Draws a rounded WIP box holding `text`, starting at `x` and centred on `mid`; returns its
/// right edge.
fn wip_box(mp: &Painter, x: f32, mid: f32, text: &str) -> f32 {
    let g = mp.layout_no_wrap(text.to_owned(), FontId::proportional(13.0), color::weak());
    let w = (g.size().x + 16.0).max(MIN_BOX_W);
    let r = Rect::from_min_size(pos2(x, mid - 11.0), vec2(w, 22.0));
    mp.rect_filled(r, CornerRadius::same(3), color::input());
    mp.galley(pos2(r.left() + 8.0, mid - g.size().y / 2.0), g, color::weak());
    r.right()
}

/// Draws the change counts from `x`: a pencil and the modified count in amber, "+ N" added in
/// green and "− N" deleted in red, leaving out zeros.
fn draw_counts(mp: &Painter, x: f32, mid: f32, c: ChangeCounts) {
    let mut x = x;
    for (n, sym, col) in [(c.modified, "", modified_color()), (c.added, "+", added_color()), (c.deleted, "−", removed_color())] {
        if n > 0 {
            if sym.is_empty() {
                paint_pencil(mp, pos2(x + 5.0, mid), col);
                x += 14.0;
            }
            let g = mp.layout_no_wrap(format!("{sym} {n}").trim().to_owned(), FontId::proportional(13.0), col);
            let w = g.size().x;
            mp.galley(pos2(x, mid - g.size().y / 2.0), g, col);
            x += w + 12.0;
        }
    }
}

/// Draws worktree `wt`'s WIP row `w` in `rect`: its lanes and dashed node when the tree column
/// is shown, then its box and counts in the message column.
pub(super) fn draw(painter: &Painter, gp: &Painter, cols: &Columns, rect: Rect, w: &WorktreeWipRow, wt: &WorktreeInfo, tree: bool) {
    let mid = rect.center().y;
    if tree {
        draw_lanes(gp, cols.graph.0, rect, &w.row);
        nodes::wip_ring(gp, pos2(lane_x(cols.graph.0, w.row.col), mid), lane_color(w.row.col));
    }
    let mp = painter.with_clip_rect(cols.rect(cols.msg, rect));
    let right = wip_box(&mp, cols.msg.0, mid, &wip_label(wt));
    draw_counts(&mp, right + 12.0, mid, wt.changes);
}
