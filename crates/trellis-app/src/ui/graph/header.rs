//! The graph's column header: the column titles with their draggable dividers (resize.rs), the
//! least width each column gets so its title fits (for the rows as well), and
//! the history notes (how many commits a truncated history shows, Load more, and a shallow
//! clone's note with its Fetch full history link). The notes sit at the right end of the message
//! column; a shallow clone's notes that don't fit there get their own line under the titles, so
//! the note is always visible.

use super::LANE_W;
use super::paint::Columns;
use super::resize;
use crate::msg::{JobMsg, LoadMsg, Msg};
use crate::state::ColumnDrag;
use crate::state::shallow::{header_tooltip, history_remote};
use crate::ui::theme::color;
use crate::ui::theme::type_scale;
use crate::ui::widgets::paint_cut_text;
use crate::ui::widgets::truncate::{cut_galley, cut_job};
use eframe::egui::{self, Align2, FontId, Pos2, Rect, Response, Sense, Stroke, pos2, vec2};
use trellis_core::repo::Snapshot;
use trellis_core::settings::{ColumnWidths, GraphColumns, ResizableColumn};
use trellis_core::shallow_ops::Deepen;

/// Space between the notes at the right of the message column.
const GAP: f32 = 10.0;
/// Height of the header row, and of the notes line under it.
const ROW_H: f32 = 22.0;

/// Space between a column's edges and its title.
const TITLE_PAD: f32 = 6.0;
/// The titles' font size.
const TITLE_FONT: f32 = type_scale::CAPTION;

/// A graph column, for its title.
#[derive(Clone, Copy, PartialEq)]
enum Title {
    Refs,
    Graph,
    Msg,
    Changes,
    Commit,
    Date,
}

/// Each column's title, left to right.
const TITLES: [(&str, Title); 6] = [
    ("Branch / tag", Title::Refs),
    ("Graph", Title::Graph),
    ("Message", Title::Msg),
    ("Changes", Title::Changes),
    ("Commit", Title::Commit),
    ("Date", Title::Date),
];

impl Title {
    fn text(self) -> &'static str {
        TITLES.iter().find(|(_, t)| *t == self).map_or("", |(text, _)| text)
    }

    fn shown(self, c: &GraphColumns) -> bool {
        match self {
            Self::Refs => c.refs,
            Self::Graph => c.tree,
            Self::Msg => c.message,
            Self::Changes => c.changes,
            Self::Commit => c.commit,
            Self::Date => c.date,
        }
    }

    /// The column's extent in the header (the message column's starts at its divider).
    fn extent(self, cols: &Columns) -> (f32, f32) {
        match self {
            Self::Refs => cols.refs,
            Self::Graph => cols.graph,
            Self::Msg => (cols.msg.0 - 6.0, cols.msg.1),
            Self::Changes => cols.changes,
            Self::Commit => cols.commit,
            Self::Date => cols.date,
        }
    }

    /// The resizable column, with the gap its stored width includes beside what is drawn
    /// (`Columns::new`).
    fn resizable(self) -> Option<(ResizableColumn, f32)> {
        match self {
            Self::Refs => Some((ResizableColumn::Refs, 0.0)),
            Self::Changes => Some((ResizableColumn::Changes, 8.0)),
            Self::Commit => Some((ResizableColumn::Commit, 8.0)),
            Self::Date => Some((ResizableColumn::Date, 0.0)),
            Self::Graph | Self::Msg => None,
        }
    }
}

/// The GRAPH column's width for `lanes`, at least its title's width (`title_w`) and padding.
fn graph_width_for(lanes: usize, title_w: f32) -> f32 {
    (lanes as f32 * LANE_W + 10.0).max(title_w + 2.0 * TITLE_PAD)
}

/// The GRAPH column's width for `lanes` lanes, wide enough for its title (one lane, or a
/// filtered search, is narrower than "Graph").
pub(super) fn graph_width(ui: &egui::Ui, lanes: usize) -> f32 {
    graph_width_for(lanes, text_w(ui, Title::Graph.text(), &FontId::proportional(TITLE_FONT)))
}

/// The least width of each resizable column: its title with padding, plus the gap the width
/// includes. COMMIT also fits a seven-character SHA.
fn floors(ui: &egui::Ui, font: &FontId) -> [(ResizableColumn, f32); 4] {
    let sha_w = text_w(ui, "0000000", &type_scale::mono(type_scale::SMALL)) + 8.0 + 2.0;
    let mut out = ResizableColumn::ALL.map(|c| (c, 0.0));
    for (text, title) in TITLES {
        let Some((col, gap)) = title.resizable() else { continue };
        let mut floor = text_w(ui, text, font) + 2.0 * TITLE_PAD + gap;
        if col == ResizableColumn::Commit {
            floor = floor.max(sha_w);
        }
        if let Some(slot) = out.iter_mut().find(|(c, _)| *c == col) {
            // Whole pixels, as stored widths are.
            slot.1 = floor.ceil();
        }
    }
    out
}

/// `w` with each column at least its floor.
fn floored(w: ColumnWidths, floors: &[(ResizableColumn, f32)]) -> ColumnWidths {
    floors.iter().fold(w, |w, &(col, f)| if w.get(col) < f { w.with(col, f) } else { w })
}

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
    let font = FontId::proportional(TITLE_FONT);
    let floors = floors(ui, &font);
    let effective = |d: &Option<ColumnDrag>| floored(d.as_ref().map_or(saved, |d| saved.with(d.col, d.width)), &floors);
    let cols = Columns::new(hdr, graph_w, cols_cfg, &effective(drag));
    // A drag starts from the width shown, which may be a column's floor.
    let active = resize::grips(ui, hdr, &cols, cols_cfg, &floored(saved, &floors), drag, msgs);
    // Lay out again with this frame's drag, so the titles don't lag a frame behind the rows.
    let widths = effective(drag);
    let cols = Columns::new(hdr, graph_w, cols_cfg, &widths);
    titles(ui, hdr, &cols, cols_cfg, &font);
    resize::paint_active(ui, hdr, &cols, cols_cfg, active);

    let notes = Notes::of(snap);
    if !notes.any() {
        return widths;
    }
    let title_end = cols.msg.0 + text_w(ui, Title::Msg.text(), &font) + 12.0;
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

/// The column titles, each cut to its column (whole on hover when cut; DATE right-aligned like
/// its dates), then the dividers between them, so no title covers one.
fn titles(ui: &egui::Ui, hdr: Rect, cols: &Columns, cols_cfg: &GraphColumns, font: &FontId) {
    let p = ui.painter();
    let weak = color::weak();
    p.line_segment([hdr.left_bottom(), hdr.right_bottom()], Stroke::new(1.0_f32, color::border()));
    let shown = || TITLES.into_iter().filter(|(_, t)| t.shown(cols_cfg)).map(|(text, t)| (text, t, t.extent(cols)));
    for (text, title, col) in shown() {
        let clip = p.with_clip_rect(Rect::from_x_y_ranges(col.0..=col.1.max(col.0), hdr.y_range()));
        let max_w = (col.1 - col.0) - 2.0 * TITLE_PAD;
        let cut = if title == Title::Date {
            let g = cut_galley(ui.ctx(), cut_job(text, font.clone(), weak), max_w);
            let rect = Align2::RIGHT_CENTER.anchor_size(pos2(col.1 - TITLE_PAD, hdr.center().y), g.size());
            let cut = g.elided;
            clip.galley(rect.min, g, weak);
            crate::ui::widgets::Cut { rect, cut }
        } else {
            paint_cut_text(&clip, pos2(col.0 + TITLE_PAD, hdr.center().y), text, font.clone(), weak, max_w)
        };
        // Only over the title itself, clear of the dividers' grips.
        if cut.cut && cut.rect.width() > 0.0 {
            let _ = ui.interact(cut.rect, ui.id().with(("column-title", text)), Sense::hover()).on_hover_text(text);
        }
    }
    for (_, _, col) in shown() {
        if col.0 > hdr.left() + 1.0 {
            p.line_segment([pos2(col.0, hdr.top() + 4.0), pos2(col.0, hdr.bottom() - 4.0)], Stroke::new(1.0_f32, color::border()));
        }
    }
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
            load_more: crate::state::refresh::can_load_more(snap),
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

#[cfg(test)]
mod tests {
    use super::*;
    use ResizableColumn::*;

    const NARROW: ColumnWidths = ColumnWidths { refs: 100.0, changes: 60.0, commit: 80.0, date: 80.0 };

    #[test]
    fn floors_widen_narrow_columns_only() {
        let floors = [(Refs, 90.0), (Changes, 66.0), (Commit, 84.0), (Date, 40.0)];
        let w = floored(NARROW, &floors);
        assert_eq!((w.get(Refs), w.get(Date)), (100.0, 80.0), "wider columns are unchanged");
        assert_eq!((w.get(Changes), w.get(Commit)), (66.0, 84.0));
    }

    #[test]
    fn floors_fit_titles_and_a_short_sha() {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let font = FontId::proportional(TITLE_FONT);
                let floors = floors(ui, &font);
                let floor = |c: ResizableColumn| floors.iter().find(|(x, _)| *x == c).map_or(0.0, |f| f.1);
                assert!(floor(Commit) >= text_w(ui, "Commit", &font) + 2.0 * TITLE_PAD + 8.0);
                assert!(floor(Commit) >= text_w(ui, "0000000", &FontId::monospace(11.5)) + 8.0);
                let w = floored(NARROW, &floors);
                for (col, f) in floors {
                    assert!(w.get(col) >= f && w.get(col) >= NARROW.get(col), "{col:?}");
                }
                assert!(graph_width(ui, 1) >= text_w(ui, "Graph", &font) + 2.0 * TITLE_PAD);
            });
        });
        assert_eq!(graph_width_for(3, 30.0), 3.0 * LANE_W + 10.0, "lanes wider than the title keep their width");
        assert_eq!(graph_width_for(1, 40.0), 40.0 + 2.0 * TITLE_PAD);
    }
}
