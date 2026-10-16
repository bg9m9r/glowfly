//! Drawing helpers for the commit graph: column layout, lanes, nodes and ref labels.

use super::LANE_W;
use crate::ui::avatars::Avatars;
use crate::ui::build_status;
use crate::ui::common::lane_color;
use crate::ui::theme::color;
use crate::ui::widgets;
use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, Rect, Stroke, epaint::CubicBezierShape, epaint::RectShape, pos2, vec2};
use trellis_core::forge::CheckState;
use trellis_core::graph::GraphRow;
use trellis_core::repo::{self, RefKind};
use trellis_core::settings::{ColumnWidths, GraphColumns, ResizableColumn};

/// The narrowest a drag leaves the message column.
const MIN_MSG_W: f32 = 120.0;
/// Room a ref chip adds at its right end for a build marker.
const MARK_W: f32 = 14.0;

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

fn solid(lane: Color32, dark_alpha: f32) -> Color32 {
    if color::is_dark() { lane.gamma_multiply(dark_alpha) } else { lane }
}

/// Draws the first ref label (plus a +N count); returns (label rect, right edge).
/// The first label as a chip (with the tip's build `marker` inside its right end when known),
/// then a "+N" chip for the rest.
pub fn draw_ref_labels(
    p: &egui::Painter,
    x: f32,
    max_x: f32,
    y: f32,
    labels: &[repo::RefLabel],
    lane: Color32,
    marker: Option<CheckState>,
) -> (Rect, f32) {
    let first = &labels[0];
    let (fill, fg, text) = match first.kind {
        RefKind::Head => (Color32::from_gray(200), Color32::BLACK, "HEAD".to_owned()),
        // Translucent fills read well on dark backgrounds; light backgrounds need solid color.
        RefKind::Local if first.is_head => (solid(lane, 0.75), Color32::WHITE, format!("✔ {}", first.name)),
        RefKind::Local => (solid(lane, 0.65), Color32::WHITE, first.name.clone()),
        RefKind::Remote => (lane.gamma_multiply(0.25), lane, first.name.clone()),
        RefKind::Tag => (color::tag_bg(), color::tag_fg(), first.name.clone()),
    };
    let extra = labels.len() - 1;
    let reserve = if extra > 0 { 30.0 } else { 0.0 };
    let g = p.layout_no_wrap(text, FontId::proportional(12.5), fg);
    let mark_w = if marker.is_some() { MARK_W } else { 0.0 };
    let w = (g.size().x + 14.0 + mark_w).min(max_x - x - reserve);
    let r = Rect::from_min_size(pos2(x, y - 11.0), vec2(w, 22.0));
    p.rect_filled(r, CornerRadius::same(3), fill);
    p.rect_stroke(r, CornerRadius::same(3), Stroke::new(1.0_f32, lane), egui::StrokeKind::Inside);
    let gh = g.size().y;
    let mut text_clip = r.shrink(2.0);
    text_clip.max.x -= mark_w;
    p.with_clip_rect(text_clip).galley(pos2(x + 7.0, y - gh / 2.0), g, fg);
    if let Some(state) = marker {
        build_status::paint_marker_on(&p.with_clip_rect(r), pos2(r.right() - 9.0, y), state);
    }
    let mut end = r.right();
    if extra > 0 {
        end += 4.0;
        end += widgets::chip(p, end, y, &format!("+{extra}"), color::toolbar(), color::weak());
    }
    (r, end)
}

pub fn paint_pencil(p: &egui::Painter, c: egui::Pos2, col: Color32) {
    let s = Stroke::new(2.0_f32, col);
    p.line_segment([pos2(c.x - 4.0, c.y + 4.0), pos2(c.x + 4.0, c.y - 4.0)], s);
    p.line_segment([pos2(c.x - 5.0, c.y + 5.0), pos2(c.x - 3.0, c.y + 5.0)], Stroke::new(1.5_f32, col));
}
