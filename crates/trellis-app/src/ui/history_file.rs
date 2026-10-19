//! The history's File view: the whole file as of the selected commit (for a commit that deletes
//! it, the version just before), in the editor font with optional line numbers and syntax
//! colors. A toolbar row gives its size and notes (Git LFS, a deleted file's last version), find
//! and go to line, and the Wrap lines toggle; a note that asks for something (an LFS pointer
//! shown in place of the file) wraps on a line of its own under it. Unwrapped, long lines scroll
//! sideways. A Markdown file adds a Text / Preview switch (find, go to line and Wrap lines hide
//! over the preview), and an image is always previewed (`history_preview.rs`). The text comes
//! from `FileHistoryView::text` (read by the runner); this only draws it.

use super::UiCache;
use super::diff::wrap::{self, MIN_COLS, RowMetrics, Wrap};
use super::diff::{DiffStyle, paint_line};
use super::empty_state::{EmptyIcon, empty_state};
use super::find_bar::{self, FindIntent, plural};
use super::pane_status::{self, Failure};
use super::theme::color;
use super::widgets::{segmented, truncate};
use crate::msg::{DiffMsg, HistoryMsg, Msg};
use crate::state::AppState;
use crate::state::blame_find::ranges;
use crate::state::history::{FileTextData, Preview, TextState};
use eframe::egui::{self, Align, Align2, Color32, Layout, Rect, RichText, Sense, Ui, UiBuilder, pos2, vec2};
use std::sync::Arc;
use trellis_core::preview::{self, PreviewKind};
use trellis_core::repo::{BINARY_NOT_SHOWN, LfsState};

pub(in crate::ui) const WRAP_LINES: &str = "Wrap lines";
pub(in crate::ui) const WRAP_TIP: &str = "Continue long lines on the next line";
pub(in crate::ui) const DELETED_NOTE: &str = "Deleted by this commit; this is its last version";
pub(in crate::ui) const LFS_UNTRUSTED: &str = "Git LFS pointer. Trust this repository to see the file's content.";
pub(in crate::ui) const LOADING_FILE: &str = "Loading the file…";
pub(in crate::ui) const FAILED_TITLE: &str = "Couldn't read this version of the file";
pub(in crate::ui) const UNAVAILABLE_TITLE: &str = "Can't show this version";
pub(in crate::ui) const EMPTY_FILE: &str = "This file is empty";
pub(in crate::ui) const PREVIEW: &str = "Preview";
pub(in crate::ui) const TEXT: &str = "Text";
pub(in crate::ui) const PREVIEW_TIP: &str = "The file as Markdown renders it";
pub(in crate::ui) const TEXT_TIP: &str = "The file's text, with find and go to line";
pub(in crate::ui) const NO_PREVIEW_LFS: &str = "A Git LFS pointer has no preview";
pub(in crate::ui) const NO_PREVIEW_LONG: &str = "Too long to preview (over 512 KB)";
/// The least width the toolbar needs to keep find and go to line on its one row.
const ONE_ROW: f32 = 560.0;

pub(super) fn show(ui: &mut Ui, st: &mut AppState, cache: &mut UiCache, msgs: &mut Vec<Msg>) {
    let Some(view) = &st.history else { return };
    let Some((oid, path, status)) = view.shown() else { return };
    let before = status == 'D';
    let text = view.text.as_ref().filter(|t| t.oid == oid && t.path == path && t.before == before);
    let data = match text.map(|t| &t.state) {
        None | Some(TextState::Loading) => return pane_status::loading(ui, LOADING_FILE),
        Some(TextState::Failed(e)) if e.message.starts_with(BINARY_NOT_SHOWN) => return super::empty_state::binary_file(ui),
        Some(TextState::Failed(e)) if !e.retry => {
            empty_state(ui, EmptyIcon::File, UNAVAILABLE_TITLE, &e.message, &[]);
            return;
        }
        Some(TextState::Failed(e)) => {
            let (t, error) = (st.settings.lang(), e.message.clone());
            let retry = Some(Msg::History(HistoryMsg::RetryFileText));
            return pane_status::failed(ui, t, Failure { title: FAILED_TITLE, error: &error, retry }, msgs);
        }
        Some(TextState::Ready(data)) => data.clone(),
    };
    let notes = notes(&data.lfs, before);
    if let Some(Preview::Image { .. }) = &data.preview {
        return super::history_preview::image(ui, cache, &data, &notes);
    }
    if data.lines.is_empty() && notes.is_empty() {
        return empty_file(ui);
    }
    let md = preview::kind(path) == Some(PreviewKind::Markdown);
    let toggle = md.then(|| match (&data.preview, &data.lfs) {
        (Some(Preview::Markdown { .. }), _) => Ok(()),
        (_, LfsState::Pointer | LfsState::Unavailable(_)) => Err(NO_PREVIEW_LFS),
        _ => Err(NO_PREVIEW_LONG),
    });
    let previewing = view.previewing(st.settings.history.preview);
    let id = (oid, path.to_owned());
    toolbar(ui, st, Bar { lines: data.lines.len(), notes: &notes, toggle, previewing }, msgs);
    for (text, col, _) in notes.iter().filter(|n| n.2) {
        note(ui, text, true, *col);
    }
    match &data.preview {
        _ if data.lines.is_empty() => empty_file(ui),
        Some(Preview::Markdown { source }) if previewing => super::history_preview::markdown(ui, st, cache, id, source, msgs),
        _ => body(ui, st, cache, data.clone()),
    }
}

/// An empty file's File view (and blame's).
pub(super) fn empty_file(ui: &mut Ui) {
    empty_state(ui, EmptyIcon::File, EMPTY_FILE, "", &[]);
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
pub(super) type Note = (String, Color32, bool);

/// What the File view says about the version shown.
fn notes(lfs: &LfsState, before: bool) -> Vec<Note> {
    let mut out = Vec::new();
    if before {
        out.push((DELETED_NOTE.to_owned(), color::weak(), false));
    }
    match lfs {
        LfsState::None => {}
        LfsState::Content => out.push(("Stored in Git LFS".to_owned(), color::weak(), false)),
        LfsState::Pointer => out.push((LFS_UNTRUSTED.to_owned(), color::warn(), true)),
        LfsState::Unavailable(why) => out.push((format!("Git LFS pointer: {why}"), color::warn(), true)),
    }
    out
}

/// What the toolbar shows.
struct Bar<'a> {
    lines: usize,
    notes: &'a [Note],
    /// For Markdown, the Text / Preview switch: `Err` says why there is no preview.
    toggle: Option<Result<(), &'static str>>,
    previewing: bool,
}

/// "N lines" and the notes that ask for nothing (cut with "…") on the left; on the right, right
/// to left, the Text / Preview switch (Markdown), find and go to line, then Wrap lines (neither
/// over a preview). Narrower than `ONE_ROW`, find and go to line move to a row of their own.
fn toolbar(ui: &mut Ui, st: &mut AppState, bar: Bar<'_>, msgs: &mut Vec<Msg>) {
    let wrap = st.settings.diff_word_wrap;
    let Some(view) = st.history.as_mut() else { return };
    let nav = &mut view.nav;
    let one_row = ui.available_width() >= ONE_ROW;
    let text_tools = !bar.previewing;
    // Only as tall as the switch when it is there, so other files keep their toolbar.
    let row_h = ui.spacing().interact_size.y;
    let first_h = if bar.toggle.is_some() { row_h.max(segmented::HEIGHT) } else { row_h };
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
    bar_row(ui, "history-file-bar", first_h, |ui| {
        if let Some(toggle) = bar.toggle {
            super::history_preview::toggle(ui, toggle, bar.previewing, msgs);
            ui.add_space(8.0);
        }
        if text_tools {
            if one_row {
                find(ui, msgs);
            }
            let r = ui.selectable_label(wrap, WRAP_LINES).on_hover_text(WRAP_TIP);
            if r.clicked() {
                msgs.push(Msg::Diff(DiffMsg::SetWordWrap(!wrap)));
            }
        }
        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
            ui.add_space(10.0);
            ui.label(RichText::new(plural(bar.lines, "line", "lines")).color(color::weak()));
            note_labels(ui, bar.notes);
        });
    });
    if text_tools && !one_row {
        ui.add_space(2.0);
        bar_row(ui, "history-file-find", row_h, |ui| find(ui, msgs));
    }
    ui.add_space(2.0);
}

/// The notes that ask for nothing, each after a dot and cut with "…" when long.
pub(super) fn note_labels(ui: &mut Ui, notes: &[Note]) {
    for (text, col, _) in notes.iter().filter(|n| !n.2) {
        ui.label(RichText::new("·").color(color::faint()));
        truncate::truncate_label(ui, RichText::new(text).color(*col), ui.available_width());
    }
}

/// One row of the toolbar, `h` tall, laid out right to left in a rectangle of its own, so what
/// doesn't fit is clipped rather than widening the panel.
pub(super) fn bar_row(ui: &mut Ui, id: &str, h: f32, add: impl FnOnce(&mut Ui)) {
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
    let marks_col = color::warn().gamma_multiply(0.45);
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
            ui.painter().rect_filled(rect, 0.0, color::warn().gamma_multiply(0.08));
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
