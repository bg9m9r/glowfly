//! The graph's column header: the column titles with their draggable dividers (resize.rs), the
//! least width each column gets so its title fits (for the rows as well: COMMIT its SHA, DATE
//! its widest date, `cells::wide_dates`), BRANCH / TAG fitted to its widest chips while it has its default width, the
//! columns fitted to the graph's width (`fit.rs`) and, when that hid some, a ⋯ mark at the
//! message column's end naming them. The history's notes (Load more, a shallow clone's Fetch full history) are in the
//! end row (`more.rs`).

use super::LANE_W;
use super::fit::{self, Fitted};
use super::layout::{CELL_PAD, Columns, EDGE_INSET};
use super::resize;
use crate::theme::color;
use crate::theme::type_scale;
use crate::widgets::truncate::{cut_galley, cut_job};
use crate::widgets::{Icon, paint_cut_text, paint_icon};
use eframe::egui::{self, Align2, FontId, Rect, Sense, Stroke, WidgetInfo, WidgetType, pos2, vec2};
use glowfly_core::i18n::Lang;
use glowfly_core::settings::{ColumnWidths, GraphColumns, ResizableColumn};
use glowfly_model::msg::Msg;
use glowfly_model::state::ColumnDrag;

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
    Author,
    Commit,
    Date,
}

/// The columns, left to right.
const ORDER: [Title; 7] = [Title::Refs, Title::Graph, Title::Msg, Title::Changes, Title::Author, Title::Commit, Title::Date];

/// The widest BRANCH / TAG fits itself to its chips (while it has its default width).
const REFS_FIT_MAX: f32 = 200.0;

impl Title {
    /// The column's title in `t`.
    fn text(self, t: Lang) -> &'static str {
        match self {
            Self::Refs => t.tr("graph.header.refs"),
            Self::Graph => t.tr("graph.header.graph"),
            Self::Msg => t.tr("graph.header.message"),
            Self::Changes => t.tr("graph.header.changes"),
            Self::Author => t.tr("graph.header.author"),
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
            Self::Author => c.author,
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
            Self::Author => Some(ResizableColumn::Author),
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

/// The least width of each resizable column: its title with its padding (the edge inset on the
/// side at the table's edge: BRANCH / TAG's left, the last shown column's right). COMMIT also
/// fits a seven-character SHA, and DATE the widest of `dates`, dates as the column shows them.
fn floors(ui: &egui::Ui, t: Lang, font: &FontId, cfg: &GraphColumns, dates: &[String]) -> [(ResizableColumn, f32); 5] {
    let sha_w = text_w(ui, "0000000", &type_scale::mono(type_scale::SMALL));
    let last = ORDER.into_iter().rev().find(|c| c.shown(cfg));
    let mut out = ResizableColumn::ALL.map(|c| (c, 0.0));
    for title in ORDER {
        let Some(col) = title.resizable() else { continue };
        let mut content = text_w(ui, title.text(t), font);
        if col == ResizableColumn::Commit {
            content = content.max(sha_w);
        }
        if col == ResizableColumn::Date {
            let font = type_scale::font(type_scale::SMALL);
            content = dates.iter().fold(content, |w, d| w.max(text_w(ui, d, &font)));
        }
        let edge = col == ResizableColumn::Refs || last == Some(title);
        let floor = content + CELL_PAD + if edge { EDGE_INSET } else { CELL_PAD };
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

/// What the header lays the columns out from.
pub(super) struct HeaderIn<'a> {
    /// The GRAPH column's content width for its lanes.
    pub graph_w: f32,
    pub cfg: &'a GraphColumns,
    pub saved: ColumnWidths,
    /// The widest row's chips in BRANCH / TAG (`refs_fit::chips_w`).
    pub chips_w: f32,
    /// The widest dates the DATE column shows (`cells::wide_dates`).
    pub dates: &'a [String],
}

/// `saved`, with BRANCH / TAG fitted to `chips_w` (its padding added, `REFS_FIT_MAX` at most)
/// while it has its default width.
fn refs_fitted(saved: ColumnWidths, chips_w: f32) -> ColumnWidths {
    let col = ResizableColumn::Refs;
    if saved.get(col) != col.default_width() {
        return saved;
    }
    let want = (chips_w + EDGE_INSET + CELL_PAD).ceil().min(REFS_FIT_MAX);
    if want > saved.get(col) { saved.with(col, want) } else { saved }
}

/// Draws the header row (the titles and dividers only) and returns the columns to lay the rows
/// out with this frame: the saved widths (BRANCH / TAG fitted to its chips), or the preview of a
/// divider being dragged, fitted to the header's width, so header and rows line up.
pub(super) fn column_header(ui: &mut egui::Ui, t: Lang, input: HeaderIn, drag: &mut Option<ColumnDrag>, msgs: &mut Vec<Msg>) -> Fitted {
    let (cols_cfg, saved) = (input.cfg, refs_fitted(input.saved, input.chips_w));
    let (hdr, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::hover());
    let font = FontId::proportional(TITLE_FONT);
    let floors = floors(ui, t, &font, cols_cfg, input.dates);
    let graph = (input.graph_w, text_w(ui, Title::Graph.text(t), &font));
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
            crate::widgets::Cut { rect, cut }
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

    const NARROW: ColumnWidths = ColumnWidths { refs: 100.0, changes: 60.0, author: 80.0, commit: 70.0, date: 80.0, layout: 2 };

    #[test]
    fn floors_widen_narrow_columns_only() {
        let floors = [(Refs, 90.0), (Changes, 66.0), (Author, 70.0), (Commit, 84.0), (Date, 40.0)];
        let w = floored(NARROW, &floors);
        assert_eq!((w.get(Refs), w.get(Author), w.get(Date)), (100.0, 80.0, 80.0), "wider columns are unchanged");
        assert_eq!((w.get(Changes), w.get(Commit)), (66.0, 84.0));
    }

    #[test]
    fn branch_tag_fits_its_chips_while_it_has_its_default_width() {
        let d = ColumnWidths::default();
        assert_eq!(refs_fitted(d, 60.0).get(Refs), 130.0, "narrow chips keep the default");
        assert_eq!(refs_fitted(d, 140.0).get(Refs), 168.0, "wider chips widen it");
        assert_eq!(refs_fitted(d, 400.0).get(Refs), REFS_FIT_MAX, "up to its cap");
        let dragged = d.with(Refs, 110.0);
        assert_eq!(refs_fitted(dragged, 400.0), dragged, "a width the user set stays");
    }

    #[test]
    fn floors_fit_titles_and_a_short_sha() {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let font = FontId::proportional(TITLE_FONT);
                let cfg = GraphColumns::default();
                let floors = floors(ui, Lang::En, &font, &cfg, &[]);
                let floor = |c: ResizableColumn| floors.iter().find(|(x, _)| *x == c).map_or(0.0, |f| f.1);
                let pads = 2.0 * CELL_PAD;
                for title in ORDER.into_iter().filter(|t| t.resizable().is_some()) {
                    assert!(floor(title.resizable().unwrap_or(Refs)) >= text_w(ui, title.text(Lang::En), &font) + pads);
                }
                let sha_w = text_w(ui, "0000000", &type_scale::mono(type_scale::SMALL));
                assert!(floor(Commit) >= sha_w + pads);
                assert!(floor(Commit) <= ColumnWidths::default().get(Commit), "the SHA fits COMMIT's default width");
                assert!(floor(Date) >= text_w(ui, "Date", &font) + CELL_PAD + EDGE_INSET, "DATE is last: the edge inset");
                let dates = ["12/28/2026 10:58 PM".to_owned(), "Jan 1".to_owned()];
                let wide = super::floors(ui, Lang::En, &font, &cfg, &dates);
                let date_w = text_w(ui, &dates[0], &type_scale::font(type_scale::SMALL));
                assert!(wide.iter().any(|&(c, f)| c == Date && f >= date_w + CELL_PAD + EDGE_INSET), "the widest date fits");
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
    fn an_earlier_years_short_date_fits_date_uncut() {
        use super::super::{cells, fit, layout::Columns};
        use glowfly_core::settings::Settings;
        // 2026-02-01 12:00 UTC; the commit is from the last day of 2025.
        let (now, then) = (1_769_947_200, 1_767_225_600 - 3600);
        let ctx = egui::Context::default();
        crate::theme::fonts::install(&ctx);
        let _ = ctx.run(egui::RawInput::default(), |_| {});
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                for (lang, locale) in [("en", "en_US"), ("de", "de_DE"), ("es", "es_ES")] {
                    let s = Settings { language: lang.into(), date_locale: locale.into(), ..Settings::default() };
                    assert!(s.relative_dates);
                    let shown = s.format_graph_time(then, now, 0);
                    assert!(shown.contains("2025"), "{shown}");
                    let cfg = GraphColumns::default();
                    let font = FontId::proportional(TITLE_FONT);
                    let wide = floors(ui, s.lang(), &font, &cfg, &cells::wide_dates(&s, now, 0, Some(then)));
                    let w = floored(ColumnWidths::default(), &wide);
                    let f = fit::fit(740.0, (48.0, 30.0), &cfg, w, &wide);
                    let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(740.0, 36.0));
                    let c = Columns::new(rect, f.graph_w, &f.cfg, &f.widths);
                    let need = text_w(ui, &shown, &type_scale::font(type_scale::SMALL));
                    assert!(f.cfg.date && c.date.1 - c.date.0 >= need, "{lang}: {shown:?} needs {need}, DATE has {:?}", c.date);
                    // A history all from this year keeps DATE at its 88 px.
                    let this_year = floors(ui, s.lang(), &font, &cfg, &cells::wide_dates(&s, now, 0, Some(now - 86_400)));
                    assert!(this_year.iter().any(|&(col, w)| col == Date && w <= 88.0), "{lang}: {this_year:?}");
                }
            });
        });
    }

    #[test]
    fn german_titles_widen_floors() {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let font = FontId::proportional(TITLE_FONT);
                let cfg = GraphColumns::default();
                let floor =
                    |t: Lang, c: ResizableColumn| floors(ui, t, &font, &cfg, &[]).iter().find(|(x, _)| *x == c).map_or(0.0, |f| f.1);
                let changes = Lang::De.tr("graph.header.changes");
                assert_eq!(changes, "Änderungen");
                assert!(floor(Lang::De, Changes) >= text_w(ui, changes, &font) + 2.0 * CELL_PAD);
                assert!(floor(Lang::De, Changes) > floor(Lang::En, Changes), "the longer German title widens the floor");
                let graph = Lang::De.tr("graph.header.graph");
                assert!(graph_width(ui, Lang::De, 1) >= text_w(ui, graph, &font));
            });
        });
    }
}
