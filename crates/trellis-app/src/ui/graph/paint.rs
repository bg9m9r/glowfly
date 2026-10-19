//! Drawing helpers for the commit graph: column layout, lanes, nodes and ref labels.

use super::LANE_W;
use super::ref_group::Chip;
use crate::ui::avatars::Avatars;
use crate::ui::build_status;
use crate::ui::common::lane_color;
use crate::ui::theme::color;
use crate::ui::theme::contrast::{self, ChipStyle};
use crate::ui::widgets;
use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, Rect, Stroke, epaint::CubicBezierShape, epaint::RectShape, pos2, vec2};
use trellis_core::forge::CheckState;
use trellis_core::graph::GraphRow;
use trellis_core::repo::RefKind;
use trellis_core::settings::{ColumnWidths, GraphColumns, ResizableColumn};

/// The narrowest a drag leaves the message column.
const MIN_MSG_W: f32 = 120.0;
/// Room a ref chip adds at its right end for a build marker.
const MARK_W: f32 = 14.0;
/// Room a ref chip adds after its name for the remote mark of an upstream in sync with it.
const CLOUD_W: f32 = 17.0;

/// Horizontal extent of each column for one row (empty ranges for hidden columns).
pub struct Columns {
    pub refs: (f32, f32),
    pub graph: (f32, f32),
    pub sha: (f32, f32),
    pub msg: (f32, f32),
    pub changes: (f32, f32),
    pub author: (f32, f32),
    pub date: (f32, f32),
}

impl Columns {
    pub fn new(rect: Rect, graph_w: f32, c: &GraphColumns, w: &ColumnWidths) -> Self {
        use ResizableColumn::*;
        let mut x = rect.left();
        let refs = if c.refs { (x, x + w.get(Refs)) } else { (x, x) };
        x = refs.1;
        let graph = if c.tree { (x, x + graph_w) } else { (x, x) };
        x = graph.1;
        let sha = if c.sha { (x + 10.0, x + w.get(Sha)) } else { (x, x) };
        x = sha.1;
        let right = rect.right() - 6.0;
        let date = if c.date { (right - w.get(Date), right) } else { (right, right) };
        let author = if c.author { (date.0 - w.get(Author), date.0 - 8.0) } else { (date.0, date.0) };
        let changes = if c.changes { (author.0 - w.get(Changes), author.0 - 8.0) } else { (author.0, author.0) };
        let msg = (x + 10.0, changes.0 - 12.0);
        Self { refs, graph, sha, msg, changes, author, date }
    }

    /// Each shown resizable column with the x of the divider that resizes it: the one on the
    /// right of BRANCH / TAG and SHA, the one on the left of the right-aligned columns.
    pub fn grips(&self, c: &GraphColumns) -> impl Iterator<Item = (ResizableColumn, f32)> {
        let refs = if c.tree {
            self.graph.0
        } else if c.sha {
            self.sha.0
        } else {
            self.msg.0 - 6.0
        };
        [
            (c.refs, ResizableColumn::Refs, refs),
            (c.sha, ResizableColumn::Sha, self.msg.0 - 6.0),
            (c.changes, ResizableColumn::Changes, self.changes.0),
            (c.author, ResizableColumn::Author, self.author.0),
            (c.date, ResizableColumn::Date, self.date.0),
        ]
        .into_iter()
        .filter(|(on, ..)| *on)
        .map(|(_, col, x)| (col, x))
    }

    /// How much wider the columns may grow before the message column drops below `MIN_MSG_W`.
    pub fn msg_room(&self) -> f32 {
        (self.msg.1 - self.msg.0 - MIN_MSG_W).max(0.0)
    }

    pub fn rect(&self, col: (f32, f32), row: Rect) -> Rect {
        Rect::from_x_y_ranges(col.0..=col.1.max(col.0), row.y_range())
    }
}

pub fn lane_x(graph_x: f32, lane: usize) -> f32 {
    graph_x + 6.0 + lane as f32 * LANE_W + LANE_W / 2.0
}

fn edge(p: &egui::Painter, a: egui::Pos2, b: egui::Pos2, c: Color32) {
    let stroke = Stroke::new(2.0_f32, c);
    if (a.x - b.x).abs() < 0.5 {
        p.line_segment([a, b], stroke);
    } else {
        let my = (a.y + b.y) / 2.0;
        p.add(CubicBezierShape::from_points_stroke([a, pos2(a.x, my), pos2(b.x, my), b], false, Color32::TRANSPARENT, stroke));
    }
}

pub fn draw_lanes(p: &egui::Painter, graph_x: f32, rect: Rect, g: &GraphRow) {
    let (top, mid, bottom) = (rect.top(), rect.center().y, rect.bottom());
    let x = |i| lane_x(graph_x, i);
    for i in g.through.iter() {
        edge(p, pos2(x(i), top), pos2(x(i), bottom), lane_color(i));
    }
    for &(i, c) in &g.top {
        edge(p, pos2(x(i), top), pos2(x(c), mid), lane_color(i));
    }
    for &(c, j) in &g.bottom {
        edge(p, pos2(x(c), mid), pos2(x(j), bottom), lane_color(j));
    }
}

fn initials_color(name: &str) -> Color32 {
    let h = name.bytes().fold(5381u32, |h, b| h.wrapping_mul(33) ^ b as u32);
    const PALETTE: [Color32; 6] = [
        Color32::from_rgb(0x2d, 0x5f, 0x8a),
        Color32::from_rgb(0x6b, 0x3f, 0x8f),
        Color32::from_rgb(0x2f, 0x7a, 0x5a),
        Color32::from_rgb(0x8a, 0x4b, 0x2d),
        Color32::from_rgb(0x7a, 0x2f, 0x55),
        Color32::from_rgb(0x3d, 0x6b, 0x7a),
    ];
    PALETTE[h as usize % PALETTE.len()]
}

/// An author avatar inside a ring: the image when available, otherwise initials.
pub fn draw_avatar(p: &egui::Painter, c: egui::Pos2, r: f32, name: &str, email: &str, ring: Color32, avatars: Option<&mut Avatars>) {
    p.circle_filled(c, r, ring);
    let inner = r - 2.0;
    if let Some(tex) = avatars.and_then(|a| a.get(email)) {
        let rect = Rect::from_center_size(c, vec2(inner * 2.0, inner * 2.0));
        let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        p.add(RectShape::filled(rect, CornerRadius::same(inner as u8), Color32::WHITE).with_texture(tex.id(), uv));
    } else {
        p.circle_filled(c, inner, initials_color(name));
        p.text(c, Align2::CENTER_CENTER, widgets::initials(name), FontId::proportional(r * 0.85), Color32::WHITE);
    }
}

/// Where a row's ref chips were drawn.
pub struct RefChips {
    /// The first label's chip.
    pub rect: Rect,
    /// The right edge of the last chip.
    pub end: f32,
    /// The first label's name didn't fit in its chip and was cut off.
    pub cut: bool,
}

/// A label chip's width for text `text_w` wide (plus `marks_w` for the marks in its right end:
/// the remote mark, the build marker) in `room`, and whether the text had to be cut off to fit.
pub fn chip_width(text_w: f32, marks_w: f32, room: f32) -> (f32, bool) {
    let want = text_w + 14.0 + marks_w;
    (want.min(room), want > room)
}

/// Draws the first chip's label (with a cloud after the name when its upstream is in sync, and
/// the tip's build `marker` inside its right end when known), then a "+N" chip for the rest.
pub fn draw_ref_labels(
    p: &egui::Painter,
    x: f32,
    max_x: f32,
    y: f32,
    chips: &[Chip],
    lane: Color32,
    marker: Option<CheckState>,
) -> RefChips {
    let first = chips[0].label;
    let (style, text) = match first.kind {
        RefKind::Head => (ChipStyle::Head, "HEAD".to_owned()),
        RefKind::Local if first.is_head => (ChipStyle::CurrentBranch, format!("✔ {}", first.name)),
        RefKind::Local => (ChipStyle::Branch, first.name.clone()),
        RefKind::Remote => (ChipStyle::Remote, first.name.clone()),
        RefKind::Tag => (ChipStyle::Tag, first.name.clone()),
    };
    let (fill, fg) = contrast::ref_chip(style, lane, &color::current());
    let extra = chips.len() - 1;
    let reserve = if extra > 0 { 30.0 } else { 0.0 };
    let g = p.layout_no_wrap(text, FontId::proportional(12.5), fg);
    let mark_w = if marker.is_some() { MARK_W } else { 0.0 };
    let cloud_w = if chips[0].upstream.is_some() { CLOUD_W } else { 0.0 };
    let (w, cut) = chip_width(g.size().x, mark_w + cloud_w, max_x - x - reserve);
    let r = Rect::from_min_size(pos2(x, y - 11.0), vec2(w, 22.0));
    p.rect_filled(r, CornerRadius::same(3), fill);
    p.rect_stroke(r, CornerRadius::same(3), Stroke::new(1.0_f32, lane), egui::StrokeKind::Inside);
    let gh = g.size().y;
    let mut text_clip = r.shrink(2.0);
    text_clip.max.x -= mark_w + cloud_w;
    p.with_clip_rect(text_clip).galley(pos2(x + 7.0, y - gh / 2.0), g, fg);
    if cloud_w > 0.0 {
        paint_cloud(&p.with_clip_rect(r), pos2(r.right() - mark_w - 13.0, y), fg);
    }
    if let Some(state) = marker {
        build_status::paint_marker_on(&p.with_clip_rect(r), pos2(r.right() - 9.0, y), state);
    }
    let mut end = r.right();
    if extra > 0 {
        end += 4.0;
        end += widgets::chip(p, end, y, &format!("+{extra}"), color::toolbar(), color::weak());
    }
    RefChips { rect: r, end, cut }
}

/// A cloud outline about 12 px wide centred on `c`: the remote mark of a branch whose upstream
/// is in sync. Three bumps over a flat base, traced as the outline seen from a point inside.
pub fn paint_cloud(p: &egui::Painter, c: egui::Pos2, col: Color32) {
    // (centre offset, radius) of each bump, and the base box between the outer bumps' centres.
    const BUMPS: [((f32, f32), f32); 3] = [((-4.0, 1.5), 2.0), ((0.0, -0.8), 3.3), ((3.4, 1.0), 2.5)];
    const BASE: (f32, f32, f32, f32) = (-4.0, 1.0, 3.4, 3.5);
    let o = vec2(0.0, 2.0);
    let far = |d: egui::Vec2| {
        let circles = BUMPS.iter().filter_map(|&((cx, cy), r)| {
            let m = o - vec2(cx, cy);
            let b = d.dot(m);
            let disc = b * b - (m.length_sq() - r * r);
            (disc >= 0.0).then(|| -b + disc.sqrt())
        });
        let slab = |o: f32, d: f32, lo: f32, hi: f32| {
            if d > 1e-6 {
                (hi - o) / d
            } else if d < -1e-6 {
                (lo - o) / d
            } else {
                f32::INFINITY
            }
        };
        let base = slab(o.x, d.x, BASE.0, BASE.2).min(slab(o.y, d.y, BASE.1, BASE.3));
        circles.fold(base, f32::max)
    };
    let points = (0..72)
        .map(|i| {
            let a = i as f32 / 72.0 * std::f32::consts::TAU;
            let d = vec2(a.cos(), a.sin());
            c + o + d * far(d)
        })
        .collect();
    p.add(egui::epaint::PathShape::closed_line(points, Stroke::new(1.4_f32, col)));
}

pub fn paint_pencil(p: &egui::Painter, c: egui::Pos2, col: Color32) {
    let s = Stroke::new(2.0_f32, col);
    p.line_segment([pos2(c.x - 4.0, c.y + 4.0), pos2(c.x + 4.0, c.y - 4.0)], s);
    p.line_segment([pos2(c.x - 5.0, c.y + 5.0), pos2(c.x - 3.0, c.y + 5.0)], Stroke::new(1.5_f32, col));
}

#[cfg(test)]
mod tests {
    use super::chip_width;

    #[test]
    fn chip_width_cuts_only_names_that_dont_fit() {
        assert_eq!(chip_width(50.0, 0.0, 100.0), (64.0, false), "fits");
        assert_eq!(chip_width(86.0, 0.0, 100.0), (100.0, false), "fits exactly");
        assert_eq!(chip_width(200.0, 0.0, 100.0), (100.0, true), "too long");
        assert_eq!(chip_width(80.0, 16.0, 100.0), (100.0, true), "the build marker takes room too");
        assert_eq!(chip_width(50.0, super::CLOUD_W, 100.0), (81.0, false), "the remote mark fits beside the name");
        assert_eq!(chip_width(70.0, super::CLOUD_W, 100.0), (100.0, true), "and is kept when the name is cut");
    }
}
