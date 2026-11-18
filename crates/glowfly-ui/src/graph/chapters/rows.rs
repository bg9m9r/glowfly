//! The Chapters list's rows, at one height: the work tree's row, a release heading, a chapter
//! (chevron, title, commit count, authors, date span) and a commit shown inside an expanded
//! chapter, plus the graph's end row. The right-hand columns are fixed; in a narrow view the
//! authors go first, then the dates, and the title is cut (whole on hover).

use crate::graph::layout::EDGE_INSET;
use crate::graph::{cells, more, ref_menu, select};
use crate::theme::color;
use crate::theme::sizes;
use crate::theme::type_scale;
use crate::widgets::row_fill;
use crate::widgets::truncate::{Cut, cut_galley, cut_job, paint_cut_text};
use eframe::egui::{self, Align2, Color32, FontId, Painter, Pos2, Rect, Sense, Stroke, pos2, vec2};
use glowfly_core::chapters::{Chapter, Chapters};
use glowfly_core::i18n::Lang;
use glowfly_core::repo::Snapshot;
use glowfly_model::msg::{ChaptersMsg, Msg, RepoMsg};
use glowfly_model::state::AppState;
use glowfly_model::state::chapters::{Row, is_selected, words};
use glowfly_model::state::custom_commands::MenuCommands;
use glowfly_model::state::load_more::EndRow;

/// From the row's left inset to its title: the chevron's room.
const TITLE_INDENT: f32 = 20.0;
/// How much further a commit inside a chapter is indented.
const INSIDE_INDENT: f32 = 24.0;
/// The columns right of the title, from the right inset: dates, authors, count.
const DATES_W: f32 = 170.0;
const AUTHORS_W: f32 = 190.0;
const COUNT_W: f32 = 100.0;
/// The space kept after a column's text.
const GAP: f32 = 12.0;
/// Below these row widths the authors, then the dates, are left out.
const AUTHORS_MIN_W: f32 = 720.0;
const DATES_MIN_W: f32 = 520.0;
/// The chevron's click area.
const TOGGLE_W: f32 = 24.0;

/// What every row of one frame shares.
pub(super) struct Frame<'a> {
    pub st: &'a AppState,
    pub snap: &'a Snapshot,
    pub list: &'a Chapters,
    pub rows: &'a [Row],
    pub t: Lang,
    pub row_h: f32,
    /// The work tree's row leads the list.
    pub wip: bool,
    /// The rows' count, WIP row included, without the end row.
    pub total: usize,
    /// The list has the keyboard focus, or nothing has.
    pub active: bool,
    pub end: Option<EndRow<'a>>,
    pub commands: MenuCommands<'a>,
    pub pointer: Option<Pos2>,
}

/// Where the right-hand columns start in a row.
struct Cols {
    right: f32,
    dates: bool,
    authors: Option<f32>,
    count: f32,
}

impl Cols {
    fn of(rect: Rect) -> Cols {
        let right = rect.right() - EDGE_INSET;
        let dates = rect.width() >= DATES_MIN_W;
        let after_dates = if dates { right - DATES_W } else { right };
        let authors = (rect.width() >= AUTHORS_MIN_W).then_some(after_dates - AUTHORS_W);
        Cols { right, dates, authors, count: authors.unwrap_or(after_dates) - COUNT_W }
    }

    /// The most the title may take, from `title_x`.
    fn title_room(&self, title_x: f32) -> f32 {
        self.count - GAP - title_x
    }
}

/// Draws display row `d` (the work tree's row counts first), or the end row.
pub(super) fn draw(ui: &mut egui::Ui, f: &Frame, d: usize, msgs: &mut Vec<Msg>) {
    if d >= f.total {
        if let Some(r) = &f.end {
            more::row(ui, f.t, f.row_h, ui.cursor().min.x + EDGE_INSET, r, f.snap, msgs);
        }
        return;
    }
    let Some(i) = d.checked_sub(usize::from(f.wip)) else { return wip(ui, f, msgs) };
    match f.rows.get(i).copied() {
        Some(Row::Release(c)) => release(ui, f, c),
        Some(Row::Chapter(c)) => chapter(ui, f, c, msgs),
        Some(Row::Commit { row, .. }) => inside(ui, f, row, msgs),
        None => {}
    }
}

fn row_rect(ui: &mut egui::Ui, f: &Frame, sense: Sense) -> (Rect, egui::Response) {
    ui.allocate_exact_size(vec2(ui.available_width(), f.row_h), sense)
}

fn title_x(rect: Rect) -> f32 {
    rect.left() + EDGE_INSET + TITLE_INDENT
}

/// `text` ending at `right`, cut to `max_w`.
fn paint_right(p: &Painter, right: f32, mid: f32, (text, font, col): (&str, FontId, Color32), max_w: f32) {
    let galley = cut_galley(p.ctx(), cut_job(text, font, col), max_w);
    let at = pos2(right - galley.size().x, sizes::centred_on(mid, galley.size().y));
    p.galley(at, galley, col);
}

/// The work tree's row: its label and change chips.
fn wip(ui: &mut egui::Ui, f: &Frame, msgs: &mut Vec<Msg>) {
    let (rect, resp) = row_rect(ui, f, Sense::click());
    let p = ui.painter_at(rect);
    let selected = f.st.session.wip_drawn_selected();
    row_fill::paint_in(&p, rect, selected, f.active, resp.hovered() || ui.rect_contains_pointer(rect));
    let label = f.t.tr("graph.wip.label");
    let mid = rect.center().y;
    let ink = if selected { color::text() } else { color::weak() };
    let cut = paint_cut_text(
        &p,
        pos2(title_x(rect), mid),
        label,
        type_scale::font(type_scale::BODY),
        ink,
        Cols::of(rect).title_room(title_x(rect)),
    );
    crate::line_counts::paint_chips(&p, cut.rect.right() + 10.0, mid, f.snap.changes);
    crate::access::name_button(&resp, true, label);
    if resp.clicked() {
        select::focus(ui.ctx());
        msgs.push(Msg::Repo(RepoMsg::SelectWip));
    }
}

/// A release heading: the tags, with a line after them. It can't be selected.
fn release(ui: &mut egui::Ui, f: &Frame, c: u32) {
    let Some(ch) = f.list.list.get(c as usize) else { return };
    let (rect, resp) = row_rect(ui, f, Sense::hover());
    let p = ui.painter_at(rect);
    let (x, mid) = (title_x(rect), rect.bottom() - f.row_h * 0.35);
    let tags = ch.releases.join(", ");
    let text =
        crate::widgets::paint_text(&p, pos2(x, mid), Align2::LEFT_CENTER, &tags, type_scale::strong(ui, type_scale::SMALL), color::weak());
    let end = rect.right() - EDGE_INSET;
    if text.right() + GAP < end {
        p.line_segment([pos2(text.right() + GAP, mid), pos2(end, mid)], Stroke::new(1.0_f32, color::border()));
    }
    let name = words::release_name(f.t, ch);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &name));
}

/// One chapter: chevron (when it has commits to show), title, count, authors and dates.
fn chapter(ui: &mut egui::Ui, f: &Frame, c: u32, msgs: &mut Vec<Msg>) {
    let Some(ch) = f.list.list.get(c as usize) else { return };
    let (rect, resp) = row_rect(ui, f, Sense::click());
    let p = ui.painter_at(rect);
    let st = f.st;
    let selected = is_selected(ch, f.snap, &st.session.selection);
    let hovered = resp.hovered() || ui.rect_contains_pointer(rect);
    row_fill::paint_in(&p, rect, selected, f.active, hovered);
    let (t, mid, cols) = (f.t, rect.center().y, Cols::of(rect));
    let expandable = !ch.inside().is_empty();
    let expanded = st.session.chapters.expanded.contains(&ch.id);
    let x = title_x(rect);
    let ink = if selected { color::strong() } else { color::text() };
    let title = paint_cut_text(&p, pos2(x, mid), &ch.title, type_scale::font(type_scale::BODY), ink, cols.title_room(x));
    let count = paint_cut_text(
        &p,
        pos2(cols.count, mid),
        &words::count_text(t, ch),
        type_scale::font(type_scale::SMALL),
        color::weak(),
        COUNT_W - GAP,
    );
    if let Some(ax) = cols.authors {
        let names = words::authors_text(t, ch);
        paint_cut_text(&p, pos2(ax, mid), &names, type_scale::font(type_scale::BODY), color::weak(), AUTHORS_W - GAP);
    }
    if cols.dates {
        let span = words::span_text(&st.settings, ch, st.cues.now, st.cues.utc_offset);
        paint_right(&p, cols.right, mid, (&span, type_scale::font(type_scale::SMALL), color::weak()), DATES_W - GAP);
    }
    crate::access::name_button(&resp, true, &words::row_name(t, ch));
    let resp = hover_tips(resp, f.pointer, (&title, &ch.title), (&count, ch.partial.then(|| t.tr("chapters.count_partial.tip"))));
    if resp.clicked() {
        select::focus(ui.ctx());
        msgs.push(Msg::Chapters(ChaptersMsg::Select(ch.id)));
    }
    if expandable {
        toggle(ui, f, rect, ch, expanded, hovered, &p, msgs);
        if resp.double_clicked() {
            msgs.push(Msg::Chapters(ChaptersMsg::Toggle(ch.id)));
        }
    }
}

/// The whole title when it was cut, and the note on a partial count, on hover over each.
fn hover_tips(
    resp: egui::Response,
    pointer: Option<Pos2>,
    (title, whole): (&Cut, &str),
    (count, note): (&Cut, Option<&str>),
) -> egui::Response {
    if !resp.hovered() {
        return resp;
    }
    if title.hovered(pointer) {
        return resp.on_hover_text_at_pointer(whole);
    }
    match note {
        Some(note) if pointer.is_some_and(|p| count.rect.contains(p)) => resp.on_hover_text_at_pointer(note),
        _ => resp,
    }
}

/// The chevron and its click area, registered after the row so a click on it only toggles.
#[allow(clippy::too_many_arguments)]
fn toggle(ui: &mut egui::Ui, f: &Frame, rect: Rect, ch: &Chapter, expanded: bool, row_hovered: bool, p: &Painter, msgs: &mut Vec<Msg>) {
    let area = Rect::from_min_size(pos2(rect.left() + EDGE_INSET - 4.0, rect.top()), vec2(TOGGLE_W, f.row_h));
    let resp = ui.interact(area, ui.id().with(("chapter-toggle", ch.id)), Sense::click());
    let title = [("title", &ch.title as &dyn std::fmt::Display)];
    let name = if expanded { f.t.trf("chapters.collapse.name", &title) } else { f.t.trf("chapters.expand.name", &title) };
    crate::access::name_button(&resp, true, &name);
    let lit = row_hovered || resp.hovered();
    crate::widgets::chevron(
        p,
        pos2(rect.left() + EDGE_INSET + 8.0, rect.center().y),
        expanded,
        if lit { color::text() } else { color::weak() },
    );
    if resp.clicked() {
        msgs.push(Msg::Chapters(ChaptersMsg::Toggle(ch.id)));
    }
}

/// A commit shown inside an expanded chapter: summary, short id, author and date.
fn inside(ui: &mut egui::Ui, f: &Frame, row: u32, msgs: &mut Vec<Msg>) {
    let Some(c) = f.snap.commits.get(row as usize) else { return };
    let (rect, resp) = row_rect(ui, f, Sense::click());
    let p = ui.painter_at(rect);
    let st = f.st;
    let selected = st.session.selection.includes(c.oid);
    row_fill::paint_in(&p, rect, selected, f.active, resp.hovered() || ui.rect_contains_pointer(rect));
    let (mid, cols) = (rect.center().y, Cols::of(rect));
    let x = title_x(rect) + INSIDE_INDENT;
    let ink = if selected { color::strong() } else { color::text() };
    let summary = paint_cut_text(&p, pos2(x, mid), &c.summary, type_scale::font(type_scale::BODY), ink, cols.title_room(x));
    paint_cut_text(
        &p,
        pos2(cols.count, mid),
        &crate::common::short(c.oid),
        type_scale::mono(type_scale::SMALL),
        color::faint(),
        COUNT_W - GAP,
    );
    if let Some(ax) = cols.authors {
        paint_cut_text(&p, pos2(ax, mid), c.author.name(), type_scale::font(type_scale::BODY), color::weak(), AUTHORS_W - GAP);
    }
    let date = st.settings.format_graph_time(c.time, st.cues.now, st.cues.utc_offset);
    if cols.dates {
        paint_right(&p, cols.right, mid, (&date, type_scale::font(type_scale::SMALL), color::weak()), DATES_W - GAP);
    }
    cells::name_row(&resp, c, f.snap.refs.get(&c.oid), date);
    let resp = if resp.hovered() && summary.hovered(f.pointer) { resp.on_hover_text_at_pointer(&*c.summary) } else { resp };
    if resp.clicked() {
        select::focus(ui.ctx());
        msgs.push(Msg::Repo(RepoMsg::SelectCommit(c.oid)));
    }
    let menu = ref_menu::RowMenu {
        commit: c,
        labels_hit: None,
        snap: f.snap,
        selection: &st.session.selection,
        settings: &st.settings,
        forge: st.session.forge.as_ref(),
        idle: st.job.is_none(),
        commands: &f.commands,
        keys: &st.keymap,
    };
    ref_menu::context_menu(ui, &resp, menu, msgs);
}
