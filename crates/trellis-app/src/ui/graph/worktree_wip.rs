//! Other worktrees' WIP rows in the graph (a dashed node in its lane's colour joined to the
//! worktree's HEAD commit, an "Uncommitted changes · <folder>" box and the change counts). This
//! tab's own WIP row is drawn by `wip_row`.

use super::lane_colours::RowColours;
use super::layout::Columns;
use super::nodes;
use super::paint::{draw_lanes, lane_x, paint_pencil};
use crate::ui::common::{added_color, modified_color, removed_color};
use crate::ui::theme::color;
use crate::ui::theme::tokens::radius;
use crate::ui::theme::type_scale;
use crate::ui::widgets::truncate::cut_left_in;
use eframe::egui::{Color32, CornerRadius, Galley, Painter, Rect, pos2, vec2};
use std::sync::Arc;
use trellis_core::graph::WorktreeWipRow;
use trellis_core::repo::{ChangeCounts, WorktreeInfo};

/// The narrowest another worktree's WIP box is.
const MIN_BOX_W: f32 = 96.0;
/// The room on each side of the box's text.
const PAD: f32 = 8.0;
/// The gap after the box and after each count.
const GAP: f32 = 12.0;
/// The room the pencil before the modified count takes.
const PENCIL_W: f32 = 14.0;

/// One change count, laid out: a pencil before it (the modified count) and its text.
struct Count {
    pencil: bool,
    galley: Arc<Galley>,
    color: Color32,
}

/// The counts of `c` to draw: modified in amber (after a pencil), "+ N" added in green and
/// "− N" deleted in red, leaving out zeros.
fn counts(p: &Painter, c: ChangeCounts) -> Vec<Count> {
    [(c.modified, "", modified_color()), (c.added, "+", added_color()), (c.deleted, "−", removed_color())]
        .into_iter()
        .filter(|(n, _, _)| *n > 0)
        .map(|(n, sym, color)| Count {
            pencil: sym.is_empty(),
            galley: p.layout_no_wrap(format!("{sym} {n}").trim().to_owned(), type_scale::font(type_scale::BODY), color),
            color,
        })
        .collect()
}

/// How wide `counts` are when drawn in a row.
fn counts_width(counts: &[Count]) -> f32 {
    let w: f32 = counts.iter().map(|c| if c.pencil { PENCIL_W } else { 0.0 } + c.galley.size().x + GAP).sum();
    (w - GAP).max(0.0)
}

/// The box's text laid out for a message column `col_w` wide whose counts take `counts_w`,
/// and the box's width. The box leaves room for the counts but is never narrower than
/// `MIN_BOX_W`. Text that doesn't fit loses its start behind "…", keeping the folder that tells
/// the rows apart (the row's tooltip has the whole text).
fn box_layout(p: &Painter, text: &str, col_w: f32, counts_w: f32) -> (Arc<Galley>, f32) {
    let room = if counts_w > 0.0 { col_w - GAP - counts_w } else { col_w };
    let max_w = room.max(MIN_BOX_W) - 2.0 * PAD;
    let font = type_scale::font(type_scale::BODY);
    let shown = cut_left_in(p, text, &font, max_w);
    let g = p.layout_no_wrap(shown, font, color::weak());
    let w = (g.size().x + 2.0 * PAD).max(MIN_BOX_W);
    (g, w)
}

/// Draws a rounded WIP box holding `g`, `w` wide, starting at `x` and centred on `mid`;
/// returns its right edge.
fn wip_box(mp: &Painter, x: f32, mid: f32, g: Arc<Galley>, w: f32) -> f32 {
    let r = Rect::from_min_size(pos2(x, mid - 11.0), vec2(w, 22.0));
    mp.rect_filled(r, CornerRadius::same(radius::SM), color::input());
    mp.galley(pos2(r.left() + PAD, mid - g.size().y / 2.0), g, color::weak());
    r.right()
}

/// Draws `counts` in a row from `x`, centred on `mid`.
fn draw_counts(mp: &Painter, x: f32, mid: f32, counts: Vec<Count>) {
    let mut x = x;
    for c in counts {
        if c.pencil {
            paint_pencil(mp, pos2(x + 5.0, mid), c.color);
            x += PENCIL_W;
        }
        let w = c.galley.size().x;
        mp.galley(pos2(x, mid - c.galley.size().y / 2.0), c.galley, c.color);
        x += w + GAP;
    }
}

/// Draws worktree `wt`'s WIP row `w` in `rect`: its lanes and dashed node (in `colours`) with
/// the tree column's painter `gp` (None while that column is hidden), then its box holding
/// `label` and its counts in the message column.
pub(super) fn draw(
    painter: &Painter,
    gp: Option<&Painter>,
    cols: &Columns,
    rect: Rect,
    row: (&WorktreeWipRow, &WorktreeInfo, &RowColours),
    label: &str,
) {
    let (w, wt, colours) = row;
    let mid = rect.center().y;
    if let Some(gp) = gp {
        draw_lanes(gp, cols.graph.0, rect, &w.row, colours);
        nodes::wip_ring(gp, pos2(lane_x(cols.graph.0, w.row.col), mid), colours.node());
    }
    let mp = painter.with_clip_rect(cols.rect(cols.msg, rect));
    let counts = counts(&mp, wt.changes);
    let (g, box_w) = box_layout(&mp, label, cols.msg.1 - cols.msg.0, counts_width(&counts));
    let right = wip_box(&mp, cols.msg.0, mid, g, box_w);
    draw_counts(&mp, right + GAP, mid, counts);
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{self, LayerId};

    fn painter() -> Painter {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |_| {});
        Painter::new(ctx, LayerId::background(), Rect::EVERYTHING)
    }

    const CHANGES: ChangeCounts = ChangeCounts { modified: 2, added: 1, deleted: 0 };

    #[test]
    fn a_long_label_keeps_its_folder_behind_an_ellipsis_and_leaves_room_for_the_counts() {
        let p = painter();
        let cw = counts_width(&counts(&p, CHANGES));
        assert!(cw > 0.0);
        for label in ["Uncommitted changes · feature-login", "Nicht committete Änderungen · feature-login"] {
            let col_w = 275.0;
            let (g, box_w) = box_layout(&p, label, col_w, cw);
            let shown = g.text();
            assert!(shown.starts_with('…') && shown.ends_with("· feature-login"), "{label}: {shown:?}");
            assert!(box_w + GAP + cw <= col_w, "{label}: box {box_w} + counts {cw} overflow {col_w}");
            assert!(g.size().x + 2.0 * PAD <= box_w, "{label}: the text stays inside its box");
        }
    }

    #[test]
    fn a_label_that_fits_is_whole() {
        let p = painter();
        let cw = counts_width(&counts(&p, CHANGES));
        let (g, box_w) = box_layout(&p, "Uncommitted changes · old", 600.0, cw);
        assert_eq!(g.text(), "Uncommitted changes · old");
        assert!(box_w + GAP + cw <= 600.0);
    }

    #[test]
    fn without_counts_the_box_may_use_the_whole_column_and_a_tiny_column_keeps_the_minimum_box() {
        let p = painter();
        assert_eq!(counts_width(&counts(&p, ChangeCounts::default())), 0.0);
        let label = "Uncommitted changes · feature-login";
        let natural = p.layout_no_wrap(label.into(), type_scale::font(type_scale::BODY), color::weak()).size().x;
        let (g, _) = box_layout(&p, label, natural + 2.0 * PAD + 1.0, 0.0);
        assert_eq!(g.text(), label, "without counts the label may use the whole column");
        let (_, box_w) = box_layout(&p, label, 40.0, 30.0);
        assert_eq!(box_w, MIN_BOX_W);
    }
}
