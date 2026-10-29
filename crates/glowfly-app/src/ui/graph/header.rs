//! The graph's column header: the column titles with their draggable dividers (resize.rs), the
//! least width each column gets so its title fits (for the rows as well), the columns fitted to
//! the graph's width (`fit.rs`) and, when that hid some, a ⋯ mark at the message column's end
//! naming them. The history's notes (Load more, a shallow clone's Fetch full history) are in the
//! end row (`more.rs`).

use super::LANE_W;
use super::fit::{self, Fitted};
use super::layout::{CELL_PAD, Columns, EDGE_INSET};
use super::resize;
use crate::msg::Msg;
use crate::state::ColumnDrag;
use crate::ui::theme::color;
use crate::ui::theme::type_scale;
use crate::ui::widgets::truncate::{cut_galley, cut_job};
use crate::ui::widgets::{Icon, paint_cut_text, paint_icon};
use eframe::egui::{self, Align2, FontId, Rect, Sense, Stroke, WidgetInfo, WidgetType, pos2, vec2};
use glowfly_core::i18n::Lang;
use glowfly_core::settings::{ColumnWidths, GraphColumns, ResizableColumn};

/// Height of the header row.
const ROW_H: f32 = 22.0;

/// The titles' font size.
const TITLE_FONT: f32 = type_scale::CAPTION;

/// A graph column, for its title.
#[derive(Clone, Copy, PartialEq)]
#[repr(u8)]
enum Title {
    Refs,
    Graph,
    Msg,
    Changes,
    Commit,
    Date,
}

/// The columns, left to right.
const ORDER: [Title; 6] = [Title::Refs, Title::Graph, Title::Msg, Title::Changes, Title::Commit, Title::Date];

impl Title {
    /// The column's title in `t`.
    fn text(self, t: Lang) -> &'static str {
        match self {
            Self::Refs => t.tr("graph.header.refs"),
            Self::Graph => t.tr("graph.header.graph"),
            Self::Msg => t.tr("graph.header.message"),
            Self::Changes => t.tr("graph.header.changes"),
            Self::Commit => t.tr("graph.header.commit"),
            Self::Date => t.tr("graph.header.date"),
        }
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

    /// The column's content extent, where its title goes (`ORDER` is the columns' order).
    fn extent(self, cols: &Columns) -> (f32, f32) {
        cols.content()[self as usize]
    }

    /// The column's cell, divider to divider.
    fn cell(self, cols: &Columns) -> (f32, f32) {
        cols.cell.all()[self as usize]
    }

    fn resizable(self) -> Option<ResizableColumn> {
        match self {
            Self::Refs => Some(ResizableColumn::Refs),
            Self::Changes => Some(ResizableColumn::Changes),
            Self::Commit => Some(ResizableColumn::Commit),
            Self::Date => Some(ResizableColumn::Date),
            Self::Graph | Self::Msg => None,
        }
    }
}

/// The GRAPH column's content width for `lanes`, at least its title's width (`title_w`).
fn graph_width_for(lanes: usize, title_w: f32) -> f32 {
    (lanes as f32 * LANE_W).max(title_w)
}

/// The GRAPH column's width for `lanes` lanes, wide enough for its title (one lane, or a
/// filtered search, is narrower than "Graph").
pub(super) fn graph_width(ui: &egui::Ui, t: Lang, lanes: usize) -> f32 {
    graph_width_for(lanes, text_w(ui, Title::Graph.text(t), &FontId::proportional(TITLE_FONT)))
}

/// The least width of each resizable column: its title with the cell padding on one side and
/// the edge inset on the other (the most either side takes). COMMIT also fits a seven-character
/// SHA.
fn floors(ui: &egui::Ui, t: Lang, font: &FontId) -> [(ResizableColumn, f32); 4] {
    let sha_w = text_w(ui, "0000000", &type_scale::mono(type_scale::SMALL));
    let mut out = ResizableColumn::ALL.map(|c| (c, 0.0));
    for title in ORDER {
        let Some(col) = title.resizable() else { continue };
        let mut content = text_w(ui, title.text(t), font);
        if col == ResizableColumn::Commit {
            content = content.max(sha_w);
        }
        let floor = content + CELL_PAD + EDGE_INSET;
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

/// Draws the header row (the titles and dividers only) and returns the columns to lay the rows
/// out with this frame: the saved widths, or the preview of a divider being dragged, fitted to
/// the header's width, so header and rows line up.
pub(super) fn column_header(
    ui: &mut egui::Ui,
    t: Lang,
    graph_w: f32,
    cols_cfg: &GraphColumns,
    saved: ColumnWidths,
    drag: &mut Option<ColumnDrag>,
    msgs: &mut Vec<Msg>,
) -> Fitted {
    let (hdr, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::hover());
    let font = FontId::proportional(TITLE_FONT);
    let floors = floors(ui, t, &font);
    let graph = (graph_w, text_w(ui, Title::Graph.text(t), &font));
    let fitted = |d: &Option<ColumnDrag>| {
        let w = floored(d.as_ref().map_or(saved, |d| saved.with(d.col, d.width)), &floors);
        fit::fit(hdr.width(), graph, cols_cfg, w, &floors)
    };
    let f = fitted(drag);
    let cols = Columns::new(hdr, f.graph_w, &f.cfg, &f.widths);
    // A drag starts from the width shown, which may be a column's floor or narrowed to fit.
    let active = resize::grips(ui, hdr, &cols, &f.cfg, &f.widths, drag, msgs);
    // Lay out again with this frame's drag, so the titles don't lag a frame behind the rows.
    let f = fitted(drag);
    let cols = Columns::new(hdr, f.graph_w, &f.cfg, &f.widths);
    titles(ui, hdr, &cols, &f.cfg, &font, t);
    hidden_mark(ui, hdr, &cols, &f, t);
    resize::paint_active(ui, hdr, &cols, &f.cfg, active);
    f
}

/// The ⋯ at the message column's end while columns are hidden to fit, naming them on hover.
fn hidden_mark(ui: &egui::Ui, hdr: Rect, cols: &Columns, f: &Fitted, t: Lang) {
    if f.hidden.is_empty() {
        return;
    }
    let names: Vec<&str> = ORDER.into_iter().filter(|c| c.resizable().is_some_and(|r| f.hidden.contains(&r))).map(|c| c.text(t)).collect();
    let text = t.trf("graph.header.hidden", &[("columns", &names.join(", "))]);
    let c = pos2(cols.cell.msg.1 - CELL_PAD - 8.0, hdr.center().y);
    let rect = Rect::from_center_size(c, vec2(20.0, ROW_H));
    paint_icon(ui.painter(), c, Icon::More, color::weak());
    let r = ui.interact(rect, ui.id().with("columns-hidden"), Sense::hover());
    r.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, &text));
    r.on_hover_text(text);
}

/// The column titles, each cut to its column (whole on hover when cut; DATE right-aligned like
/// its dates), then the dividers between them, so no title covers one.
fn titles(ui: &egui::Ui, hdr: Rect, cols: &Columns, cols_cfg: &GraphColumns, font: &FontId, t: Lang) {
    let p = ui.painter();
    let weak = color::weak();
    p.line_segment([hdr.left_bottom(), hdr.right_bottom()], Stroke::new(1.0_f32, color::border()));
    let shown = || ORDER.into_iter().filter(|c| c.shown(cols_cfg)).map(|c| (c.text(t), c, c.extent(cols)));
    for (text, title, col) in shown() {
        let clip = p.with_clip_rect(cols.rect(title.cell(cols), hdr));
        let max_w = col.1 - col.0;
        let cut = if title == Title::Date {
            let g = cut_galley(ui.ctx(), cut_job(text, font.clone(), weak), max_w);
            let rect = Align2::RIGHT_CENTER.anchor_size(pos2(col.1, hdr.center().y), g.size());
            let cut = g.elided;
            clip.galley(rect.min, g, weak);
            crate::ui::widgets::Cut { rect, cut }
        } else {
            paint_cut_text(&clip, pos2(col.0, hdr.center().y), text, font.clone(), weak, max_w)
        };
        // Only over the title itself, clear of the dividers' grips.
        if cut.cut && cut.rect.width() > 0.0 {
            let _ = ui.interact(cut.rect, ui.id().with(("column-title", title as u8)), Sense::hover()).on_hover_text(text);
        }
    }
    for (_, title, _) in shown() {
        let x = title.cell(cols).0;
        if x > hdr.left() + 1.0 {
            p.line_segment([pos2(x, hdr.top() + 4.0), pos2(x, hdr.bottom() - 4.0)], Stroke::new(1.0_f32, color::border()));
        }
    }
}

fn text_w(ui: &egui::Ui, text: &str, font: &FontId) -> f32 {
    ui.painter().layout_no_wrap(text.into(), font.clone(), color::weak()).size().x
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
                let floors = floors(ui, Lang::En, &font);
                let floor = |c: ResizableColumn| floors.iter().find(|(x, _)| *x == c).map_or(0.0, |f| f.1);
                let pads = CELL_PAD + EDGE_INSET;
                for title in ORDER.into_iter().filter(|t| t.resizable().is_some()) {
                    assert!(floor(title.resizable().unwrap_or(Refs)) >= text_w(ui, title.text(Lang::En), &font) + pads);
                }
                assert!(floor(Commit) >= text_w(ui, "0000000", &type_scale::mono(type_scale::SMALL)) + pads);
                let w = floored(NARROW, &floors);
                for (col, f) in floors {
                    assert!(w.get(col) >= f && w.get(col) >= NARROW.get(col), "{col:?}");
                }
                assert!(graph_width(ui, Lang::En, 1) >= text_w(ui, "Graph", &font));
            });
        });
        assert_eq!(graph_width_for(3, 30.0), 3.0 * LANE_W, "lanes wider than the title keep their width");
        assert_eq!(graph_width_for(1, 40.0), 40.0);
    }

    #[test]
    fn german_titles_widen_floors() {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let font = FontId::proportional(TITLE_FONT);
                let floor = |t: Lang, c: ResizableColumn| floors(ui, t, &font).iter().find(|(x, _)| *x == c).map_or(0.0, |f| f.1);
                let changes = Lang::De.tr("graph.header.changes");
                assert_eq!(changes, "Änderungen");
                assert!(floor(Lang::De, Changes) >= text_w(ui, changes, &font) + CELL_PAD + EDGE_INSET);
                assert!(floor(Lang::De, Changes) > floor(Lang::En, Changes), "the longer German title widens the floor");
                let graph = Lang::De.tr("graph.header.graph");
                assert!(graph_width(ui, Lang::De, 1) >= text_w(ui, graph, &font));
            });
        });
    }
}
