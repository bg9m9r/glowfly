//! The history's File view: the whole file as of the selected commit (for a commit that deletes
//! it, the version just before), in the editor font with optional line numbers and syntax
//! colors. A toolbar row gives its size and notes (Git LFS, a deleted file's last version), find
//! and go to line, and the Wrap lines toggle; a note that asks for something (an LFS pointer
//! shown in place of the file) wraps on a line of its own under it. Unwrapped, long lines scroll
//! sideways. The text
//! comes from `FileHistoryView::text` (read by the runner); this only draws it.

use super::UiCache;
use super::diff::wrap::{self, MIN_COLS, RowMetrics, Wrap};
use super::diff::{DiffStyle, paint_line};
use super::find_bar::{self, FindIntent, plural};
use super::theme::color;
use super::widgets::truncate;
use crate::msg::{DiffMsg, HistoryMsg, Msg};
use crate::state::AppState;
use crate::state::blame_find::ranges;
use crate::state::history::{FileTextData, TextState};
use eframe::egui::{self, Align, Align2, Color32, Layout, Rect, RichText, Sense, Ui, UiBuilder, pos2, vec2};
use std::sync::Arc;
use trellis_core::repo::LfsState;

pub(in crate::ui) const WRAP_LINES: &str = "Wrap lines";
pub(in crate::ui) const WRAP_TIP: &str = "Continue long lines on the next line";
pub(in crate::ui) const DELETED_NOTE: &str = "Deleted by this commit; this is its last version";
pub(in crate::ui) const LFS_UNTRUSTED: &str = "Git LFS pointer. Trust this repository to see the file's content.";
/// The least width the toolbar needs to keep find and go to line on its one row.
const ONE_ROW: f32 = 560.0;

pub(super) fn show(ui: &mut Ui, st: &mut AppState, cache: &mut UiCache, msgs: &mut Vec<Msg>) {
    let Some(view) = &st.history else { return };
    let Some((oid, path, status)) = view.shown() else { return };
    let before = status == 'D';
    let text = view.text.as_ref().filter(|t| t.oid == oid && t.path == path && t.before == before);
    let data = match text.map(|t| &t.state) {
        None | Some(TextState::Loading) => return note(ui, "Loading…", false, color::weak()),
        Some(TextState::Failed(e)) => return note(ui, &e.clone(), true, color::weak()),
        Some(TextState::Ready(data)) => data.clone(),
    };
    let notes = notes(&data.lfs, before);
    if data.lines.is_empty() && notes.is_empty() {
        return note(ui, "This file is empty", false, color::weak());
    }
    toolbar(ui, st, data.lines.len(), &notes, msgs);
    for (text, col, _) in notes.iter().filter(|n| n.2) {
        note(ui, text, true, *col);
    }
    if data.lines.is_empty() {
        return note(ui, "This file is empty", false, color::weak());
    }
    body(ui, st, cache, data);
}

/// A line of text in `col` below the commit block; `wrap` for messages that can run long.
fn note(ui: &mut Ui, text: &str, wrap: bool, col: Color32) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        let label = egui::Label::new(RichText::new(text).color(col));
        ui.add(if wrap { label.wrap() } else { label });
    });
}

/// A note about the version shown: its text, its color, and whether it asks for something
/// (wrapped under the toolbar rather than cut in it).
type Note = (String, Color32, bool);

/// What the File view says about the version shown.
fn notes(lfs: &LfsState, before: bool) -> Vec<Note> {
    let mut out = Vec::new();
    if before {
        out.push((DELETED_NOTE.to_owned(), color::weak(), false));
    }
    match lfs {
        LfsState::None => {}
        LfsState::Content => out.push(("Stored in Git LFS".to_owned(), color::weak(), false)),
        LfsState::Pointer => out.push((LFS_UNTRUSTED.to_owned(), color::amber(), true)),
        LfsState::Unavailable(why) => out.push((format!("Git LFS pointer: {why}"), color::amber(), true)),
    }
    out
}

/// "N lines" and the notes that ask for nothing (cut with "…") on the left; on the right, right
/// to left, find and go to line, then Wrap lines. Narrower than `ONE_ROW`, find and go to line
/// move to a row of their own.
fn toolbar(ui: &mut Ui, st: &mut AppState, n: usize, notes: &[Note], msgs: &mut Vec<Msg>) {
    let wrap = st.settings.diff_word_wrap;
    let Some(view) = st.history.as_mut() else { return };
    let nav = &mut view.nav;
    let one_row = ui.available_width() >= ONE_ROW;
    let mut find = |ui: &mut Ui, msgs: &mut Vec<Msg>| {
        for intent in find_bar::find_and_line(ui, &mut nav.find, &mut nav.goto, &mut nav.focus_find) {
            msgs.push(Msg::History(match intent {
                FindIntent::Changed => HistoryMsg::FileFindChanged,
                FindIntent::Step(d) => HistoryMsg::FileFindStep(d),
                FindIntent::GoToLine => HistoryMsg::FileGoToLine,
            }));
        }
    };
    ui.add_space(2.0);
    bar_row(ui, "history-file-bar", |ui| {
        if one_row {
            find(ui, msgs);
        }
        let r = ui.selectable_label(wrap, WRAP_LINES).on_hover_text(WRAP_TIP);
        if r.clicked() {
            msgs.push(Msg::Diff(DiffMsg::SetWordWrap(!wrap)));
        }
        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
            ui.add_space(10.0);
            ui.label(RichText::new(plural(n, "line", "lines")).color(color::weak()));
            for (text, col, _) in notes.iter().filter(|n| !n.2) {
                ui.label(RichText::new("·").color(color::faint()));
                truncate::truncate_label(ui, RichText::new(text).color(*col), ui.available_width());
            }
        });
    });
    if !one_row {
        ui.add_space(2.0);
        bar_row(ui, "history-file-find", |ui| find(ui, msgs));
    }
    ui.add_space(2.0);
}

/// One row of the toolbar, laid out right to left in a rectangle of its own, so what doesn't
/// fit is clipped rather than widening the panel.
fn bar_row(ui: &mut Ui, id: &str, add: impl FnOnce(&mut Ui)) {
    let h = ui.spacing().interact_size.y;
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::hover());
    let rect = rect.with_max_x(rect.right() - 8.0);
    let builder = UiBuilder::new().id_salt(id).max_rect(rect).layout(Layout::right_to_left(Align::Center));
    add(&mut ui.new_child(builder));
}

/// The file's lines in a scroll area of their own, only those in view laid out.
fn body(ui: &mut Ui, st: &mut AppState, cache: &mut UiCache, data: Arc<FileTextData>) {
    let style = DiffStyle::new(&st.settings);
    let n = data.lines.len();
    let (char_w, line_h, digit_w) =
        ui.fonts(|f| (f.glyph_width(&style.font, 'M'), f.row_height(&style.font), f.glyph_width(&style.font, '0')));
    let num_w = if style.numbers { digit_w * n.to_string().len() as f32 } else { 0.0 };
    let code_x = 8.0 + if style.numbers { num_w + 12.0 } else { 0.0 };
    let scroll = &ui.spacing().scroll;
    let text_w = ui.available_width() - scroll.allocated_width().max(scroll.bar_width) - code_x - 8.0;
    let wrap = style.wrap.then(|| {
        let cols = if char_w > 0.0 { (text_w / char_w).floor().max(0.0) as usize } else { 0 };
        Wrap { cols: cols.max(MIN_COLS), line_h }
    });
    cache.history_text.refresh(&data, style.tab, style.row_h, wrap);
    let tc = &cache.history_text;
    let m = if wrap.is_some() { RowMetrics::Varied(&tc.offsets) } else { RowMetrics::Uniform(style.row_h) };
    let width = if wrap.is_some() { ui.available_width() } else { ui.available_width().max(code_x + tc.widest as f32 * char_w + 40.0) };

    let syntax = st.settings.syntax_highlighting;
    let Some(view) = st.history.as_mut() else { return };
    let nav = &mut view.nav;
    let scroll_row = if std::mem::take(&mut nav.to_top) {
        nav.scroll_to = None;
        Some(0)
    } else {
        nav.scroll_to.take().map(|l| l.saturating_sub(nav.visible / 2))
    };
    let (cursor, query, hits) = (nav.cursor, nav.find.query.clone(), nav.find.hits.clone());
    let marks_col = color::amber().gamma_multiply(0.45);
    let bg = ui.available_rect_before_wrap();
    ui.painter().rect_filled(bg, 0.0, color::bg());
    let mut clicked = None;
    let (top, visible) = wrap::scroll(ui, "history-file", n, m, scroll_row, |ui, i, h| {
        let (rect, resp) = ui.allocate_exact_size(vec2(width, h), Sense::click());
        if resp.clicked() {
            clicked = Some(i);
        }
        if !ui.is_rect_visible(rect) {
            return;
        }
        if cursor == Some(i) {
            ui.painter().rect_filled(rect, 0.0, color::selected());
            ui.painter().rect_filled(Rect::from_min_size(rect.min, vec2(3.0, h)), 0.0, color::accent());
        } else if hits.binary_search(&i).is_ok() {
            ui.painter().rect_filled(rect, 0.0, color::amber().gamma_multiply(0.08));
        }
        let y = rect.top() + style.row_h / 2.0;
        if style.numbers {
            let num_right = rect.left() + 8.0 + num_w;
            ui.painter().text(pos2(num_right, y), Align2::RIGHT_CENTER, (i + 1).to_string(), style.font.clone(), color::faint());
        }
        let code_left = rect.left() + code_x;
        let clip = Rect::from_min_max(pos2(code_left, rect.top()), rect.max);
        let painter = ui.painter().with_clip_rect(clip.intersect(ui.clip_rect()));
        let line = &data.lines[i];
        let spans = if syntax { data.spans(i) } else { &[] };
        let marks = ranges(line, &query);
        let marks = (!marks.is_empty()).then_some((marks.as_slice(), marks_col));
        paint_line(&painter, pos2(code_left, y), line, spans, marks, &style.font, color::text(), style.tab, wrap);
    });
    nav.top = top;
    nav.visible = visible;
    if clicked.is_some() {
        nav.cursor = clicked;
    }
}
