//! The graph's column header: the column titles with their draggable dividers (resize.rs), and
//! the history notes (how many commits a truncated history shows, Load more, and a shallow
//! clone's note with its Fetch full history link). The notes sit at the right end of the message
//! column; a shallow clone's notes that don't fit there get their own line under the titles, so
//! the note is always visible.

use super::paint::Columns;
use super::resize;
use crate::msg::{JobMsg, LoadMsg, Msg};
use crate::state::ColumnDrag;
use crate::state::shallow::{header_tooltip, history_remote};
use crate::ui::theme::color;
use eframe::egui::{self, Align2, FontId, Pos2, Rect, Response, Sense, Stroke, pos2, vec2};
use trellis_core::repo::Snapshot;
use trellis_core::settings::{ColumnWidths, GraphColumns};
use trellis_core::shallow_ops::Deepen;

/// Space between the notes at the right of the message column.
const GAP: f32 = 10.0;
/// Height of the header row, and of the notes line under it.
const ROW_H: f32 = 22.0;

const FETCH_FULL: &str = "Fetch full history";
const SHALLOW: &str = "Shallow clone";
const LOAD_MORE: &str = "Load more";

/// Draws the header row and returns the column widths to lay the rows out with this frame (the
/// saved widths, or the preview of a divider being dragged), so header and rows line up.
pub(super) fn column_header(
    ui: &mut egui::Ui,
    snap: &Snapshot,
    graph_w: f32,
    cols_cfg: &GraphColumns,
    saved: ColumnWidths,
    drag: &mut Option<ColumnDrag>,
    msgs: &mut Vec<Msg>,
) -> ColumnWidths {
    let (hdr, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::hover());
    let effective = |d: &Option<ColumnDrag>| d.as_ref().map_or(saved, |d| saved.with(d.col, d.width));
    let cols = Columns::new(hdr, graph_w, cols_cfg, &effective(drag));
    let active = resize::grips(ui, hdr, &cols, cols_cfg, &saved, drag, msgs);
    // Lay out again with this frame's drag, so the titles don't lag a frame behind the rows.
    let widths = effective(drag);
    let cols = Columns::new(hdr, graph_w, cols_cfg, &widths);
    let font = FontId::proportional(10.5);
    titles(ui, hdr, &cols, cols_cfg, &font);
    resize::paint_active(ui, hdr, &cols, cols_cfg, active);

    let notes = Notes::of(snap);
    if !notes.any() {
        return widths;
    }
    let title_end = cols.msg.0 + text_w(ui, "COMMIT MESSAGE", &font) + 12.0;
    if notes.shallow && cols.msg.1 - notes.width(ui, snap, &font) <= title_end {
        // Too narrow for the whole set beside the title: a line of its own, under the titles.
        let (line, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::hover());
        ui.painter().line_segment([line.left_bottom(), line.right_bottom()], Stroke::new(1.0_f32, color::border()));
        notes.draw(ui, snap, &font, pos2(line.right() - 6.0, line.center().y), line.left() + 6.0, msgs);
    } else {
        notes.draw(ui, snap, &font, pos2(cols.msg.1, hdr.center().y), title_end, msgs);
    }
    widths
}

/// The column titles and the dividers between them.
fn titles(ui: &egui::Ui, hdr: Rect, cols: &Columns, cols_cfg: &GraphColumns, font: &FontId) {
    let p = ui.painter();
    let weak = color::weak();
    p.line_segment([hdr.left_bottom(), hdr.right_bottom()], Stroke::new(1.0_f32, color::border()));
    let header = |col: (f32, f32), text: &str, on: bool| {
        if on {
            p.text(pos2(col.0 + 6.0, hdr.center().y), Align2::LEFT_CENTER, text, font.clone(), weak);
            if col.0 > hdr.left() + 1.0 {
                p.line_segment([pos2(col.0, hdr.top() + 4.0), pos2(col.0, hdr.bottom() - 4.0)], Stroke::new(1.0_f32, color::border()));
            }
        }
    };
    header(cols.refs, "BRANCH / TAG", cols_cfg.refs);
    header(cols.graph, "GRAPH", cols_cfg.tree);
    header(cols.sha, "SHA", cols_cfg.sha);
    header((cols.msg.0 - 6.0, cols.msg.1), "COMMIT MESSAGE", cols_cfg.message);
    header(cols.changes, "CHANGES", cols_cfg.changes);
    header(cols.author, "AUTHOR", cols_cfg.author);
    header(cols.date, "DATE", cols_cfg.date);
}

/// Which history notes the header shows.
struct Notes {
    load_more: bool,
    fetch_full: bool,
    count: bool,
    shallow: bool,
}

impl Notes {
    fn of(snap: &Snapshot) -> Self {
        let shallow = snap.is_shallow();
        Self {
            load_more: snap.truncated && snap.commits.len() < crate::state::MAX_COMMITS,
            fetch_full: shallow && history_remote(snap).is_some(),
            count: snap.truncated,
            shallow,
        }
    }

    fn any(&self) -> bool {
        self.count || self.shallow
    }

    /// Width of every note at full length, with the gaps between them.
    fn width(&self, ui: &egui::Ui, snap: &Snapshot, font: &FontId) -> f32 {
        let widths = [
            self.load_more.then(|| text_w(ui, LOAD_MORE, font)),
            self.fetch_full.then(|| text_w(ui, FETCH_FULL, font)),
            self.count.then(|| text_w(ui, &count_text(snap), font)),
            self.shallow.then(|| text_w(ui, SHALLOW, font)),
        ];
        let shown: Vec<f32> = widths.into_iter().flatten().collect();
        shown.iter().sum::<f32>() + GAP * shown.len().saturating_sub(1) as f32
    }

    /// Lays the notes out right to left from `right`. The links always show; the notes only
    /// where they stay right of `left`.
    fn draw(&self, ui: &egui::Ui, snap: &Snapshot, font: &FontId, right: Pos2, left: f32, msgs: &mut Vec<Msg>) {
        let p = ui.painter();
        let y = right.y;
        let mut x = right.x;
        if self.load_more {
            let (resp, w) = link(ui, font, pos2(x, y), "load-more", LOAD_MORE, LOAD_MORE);
            x -= w + GAP;
            if resp.clicked() {
                msgs.push(Msg::Load(LoadMsg::LoadMoreCommits));
            }
        }
        let tooltip = self.shallow.then(|| header_tooltip(snap));
        if let Some(tip) = tooltip.as_ref().filter(|_| self.fetch_full) {
            // Where even the line of its own is too narrow, the link shows the shorter text; the
            // tooltip and the link's accessible name still say what it does.
            let text = if x - text_w(ui, FETCH_FULL, font) > left { FETCH_FULL } else { "Full history" };
            let (resp, w) = link(ui, font, pos2(x, y), "fetch-full-history", FETCH_FULL, text);
            x -= w + GAP;
            if resp.on_hover_text(tip).clicked() {
                msgs.push(Msg::Job(JobMsg::Deepen(Deepen::Full)));
            }
        }
        if self.count {
            let count = p.layout_no_wrap(count_text(snap), font.clone(), color::faint());
            let count_x = x - count.size().x;
            if count_x > left {
                p.galley(pos2(count_x, y - count.size().y / 2.0), count, color::faint());
            }
            x = count_x - GAP;
        }
        if let Some(tip) = tooltip {
            let label = p.layout_no_wrap(SHALLOW.into(), font.clone(), color::faint());
            let rect = Align2::RIGHT_CENTER.anchor_size(pos2(x, y), label.size());
            if rect.left() > left {
                p.galley(rect.min, label, color::faint());
                let resp = ui.interact(rect, ui.id().with("shallow-clone"), Sense::hover());
                resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, SHALLOW));
                let _ = resp.on_hover_text(tip);
            }
        }
    }
}

fn count_text(snap: &Snapshot) -> String {
    format!("newest {} commits", snap.commits.len())
}

fn text_w(ui: &egui::Ui, text: &str, font: &FontId) -> f32 {
    ui.painter().layout_no_wrap(text.into(), font.clone(), color::weak()).size().x
}

/// An accent text link showing `text` with its right end at `right`, underlined on hover and
/// named `label` for accessibility. Returns its response and width.
fn link(ui: &egui::Ui, font: &FontId, right: Pos2, id: &str, label: &str, text: &str) -> (Response, f32) {
    let p = ui.painter();
    let g = p.layout_no_wrap(text.into(), font.clone(), color::accent());
    let rect = Align2::RIGHT_CENTER.anchor_size(right, g.size());
    let resp = ui.interact(rect, ui.id().with(id), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
    let name = label.to_owned();
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Link, true, &name));
    if resp.hovered() {
        p.line_segment([rect.left_bottom(), rect.right_bottom()], Stroke::new(1.0_f32, color::accent()));
    }
    p.galley(rect.min, g, color::accent());
    (resp, rect.width())
}
