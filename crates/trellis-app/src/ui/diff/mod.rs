//! The diff viewer: one column (inline) or old and new side by side (split), showing the hunks
//! or the whole file, with hunk- and line-level stage, unstage and discard, Revert hunk and
//! Revert lines on commit and comparison diffs, and next/previous change navigation. In the
//! whole-file view long unchanged runs are folded into "N unchanged lines" rows that open in
//! place when clicked; an opened run folds back from a control on its first line, and the header's
//! View menu can open or close them all. In the hunk view each hunk header after the first has a control
//! that fills in the unchanged lines between it and the previous hunk (the same runs as the
//! whole-file view's folds, open in both views), folded back from their first line. The fold
//! keys (E and Shift+E by default) open the first closed run at or below the top and fold back
//! the opened one at the top. With wrapping on, long lines continue on the next line and their
//! rows grow to fit. Changed lines are selected by click, Shift-click and Ctrl/Cmd-click, or by
//! pressing on one and dragging across the lines of its hunk. A changed PNG or JPEG shows its
//! before and after images (`preview`).

pub(in crate::ui) mod fold;
mod header;
mod hunk_header;
mod inline;
mod lfs_note;
pub(super) mod preview;
pub(super) mod rows_cache;
mod split;
mod style;
mod syntax;
mod thread_marks;
pub(in crate::ui) mod wrap;

use super::UiCache;
use super::empty_state;
use super::theme::color;
use crate::msg::DiffMsg;
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::lines::{LineClick, LineSelection};
use crate::state::pull_request_threads::{self, Marks};
use eframe::egui::{CursorIcon, FontId, Rect, Response, Sense, Ui, vec2};
use hunk_header::hunk_header;
use rows_cache::{DiffRowsCache, Source};
pub(super) use style::DiffStyle;
pub(super) use syntax::paint_line;
use trellis_core::diff_rows::{GapLines, GapSpan};
use trellis_core::highlight::DiffHighlight;
use trellis_core::i18n::Lang;
use trellis_core::repo::{DiffTarget, FileDiff};
use trellis_core::settings::DiffLayout;
use trellis_core::whole_file::{self, Fold, WholeFile};

/// The default row height and font, which the conflict view keeps; the diff viewer draws
/// with `DiffStyle` from the settings.
pub(super) const ROW_H: f32 = 19.0;

pub(super) fn mono() -> FontId {
    FontId::monospace(trellis_core::settings::DEFAULT_DIFF_FONT_SIZE)
}

/// What both layouts draw from.
pub(super) struct DiffView<'a> {
    pub target: &'a DiffTarget,
    pub diff: &'a FileDiff,
    /// Some in the whole-file view: rows and lines come from `whole.diff`; `diff` still
    /// supplies hunk headers and hunk indexes for the action rows.
    pub whole: Option<&'a WholeFile>,
    /// The opened folds of the whole-file view, in order; their first lines get the fold-back
    /// control.
    pub opened: Vec<Fold>,
    /// The hunk view's closed gaps between hunks, in order; their hunk headers get the unfold
    /// control.
    pub gaps: Vec<GapSpan>,
    /// The hunk view's opened gaps with their text, in order (`DiffRow::GapLine` rows).
    pub gap_lines: Vec<(GapSpan, &'a GapLines)>,
    /// Syntax spans of the diff the lines come from (`lines()`), `None` to paint plain.
    pub hl: Option<&'a DiffHighlight>,
    /// Syntax spans for the hunk headers' function context: the normal hunks' highlight, also
    /// in the whole-file view, whose action rows show those headers; `None` to paint headers
    /// plain.
    pub header_hl: Option<&'a DiffHighlight>,
    /// The diff's generation, or `None` while a reload is on its way (see `hunk_header`).
    pub hunk_gen: Option<u64>,
    /// The line selection; `None` when lines can't be selected (the whole-file view).
    pub sel: Option<&'a LineSelection>,
    /// Font, row height and gutters from Preferences > Editor.
    pub style: DiffStyle,
    /// How long lines wrap, `None` when they don't.
    pub wrap: Option<wrap::Wrap>,
    /// Code comment markers by file line, for a pull request's file whose threads can be
    /// placed on its lines.
    pub marks: Option<&'a Marks>,
    /// The language of the hunk buttons and their confirmations.
    pub t: Lang,
}

impl DiffView<'_> {
    /// The diff whose hunks hold the drawn lines: the whole file, or the normal hunks.
    pub fn lines(&self) -> &FileDiff {
        self.whole.map_or(self.diff, |w| &w.diff)
    }

    pub fn is_selected(&self, hunk: usize, line: usize) -> bool {
        self.sel.is_some_and(|s| s.contains(self.target, hunk, line))
    }
}

/// The open diff: its header (`header`, one 48 px row) and, below it, `body`.
pub fn show(ui: &mut Ui, st: &mut AppState, cache: &mut UiCache, msgs: &mut Vec<Msg>) {
    if st.diff.is_none() {
        // A scroll target for a diff that is gone is dropped, not kept for the next one.
        st.view.diff_scroll_to_row = None;
        return;
    }
    if let Some(m) = crate::state::diff_header::model(st) {
        header::show(ui, &m, &st.keymap, st.settings.lang(), msgs);
    }
    body(ui, st, cache, msgs);
}

/// Draws the open diff's rows and records the first visible row and how many rows fit.
fn record_rows(ui: &mut Ui, st: &mut AppState, cache: &mut DiffRowsCache, scroll_to: Option<usize>, msgs: &mut Vec<Msg>) {
    if let Some((top, visible)) = rows(ui, st, cache, scroll_to, msgs) {
        st.view.diff_top_row = top;
        st.view.diff_visible_rows = visible;
    }
}

/// Draws the open diff's rows in the current layout and view mode (hunks, or the whole file
/// when it loaded) and returns the first visible row and how many rows fit. Lines are
/// selectable in every diff unless line operations are blocked.
fn rows(ui: &mut Ui, st: &AppState, cache: &mut DiffRowsCache, scroll_to: Option<usize>, msgs: &mut Vec<Msg>) -> Option<(usize, usize)> {
    let (target, diff) = st.diff.as_ref()?;
    let whole = whole_file::shown(diff, st.settings.diff_mode);
    // Highlighting always comes with the diff; the setting only decides whether to paint it.
    let hl = whole.map_or(diff.highlight.as_ref(), |w| w.diff.highlight.as_ref()).filter(|_| st.settings.syntax_highlighting);
    let header_hl = diff.highlight.as_ref().filter(|_| st.settings.syntax_highlighting);
    let selectable = st.line_ops_blocked().is_none();
    let sel = selectable.then_some(&st.view.diff_lines);
    // The release can happen over a row that is no longer drawn, so the drag ends here.
    if sel.and_then(LineSelection::dragging).is_some() {
        if ui.input(|i| i.pointer.primary_down()) {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        } else {
            msgs.push(Msg::Diff(DiffMsg::EndLineDrag));
        }
    }
    let style = DiffStyle::new(&st.settings);
    let expanded = &st.view.diff_expanded;
    let opened = whole.map(|w| whole_file::opened(w, expanded)).unwrap_or_default();
    let layout = st.settings.diff_layout;
    let generation = (st.diff_gen, st.diff_shown_gen);
    // The gaps between hunks fill in only in the hunk view, and never with whitespace hidden.
    let fill = whole.is_none() && !st.settings.diff_ignore_whitespace;
    cache.refresh(Source { diff, whole, generation, layout, expanded, gaps: fill, tab: style.tab });
    let gap_lines: Vec<(GapSpan, &GapLines)> = match expanded.gap_text() {
        Some(text) => cache.gaps.iter().filter(|g| expanded.contains(g.key)).filter_map(|g| Some((*g, text.lines_for(g)?))).collect(),
        None => Vec::new(),
    };
    let gaps = cache.gaps.iter().filter(|g| !gap_lines.iter().any(|(o, _)| o.key == g.key)).copied().collect();
    let wrap = wrap::wrap_for(ui, &style, layout);
    let lines: &FileDiff = whole.map_or(diff, |w| &w.diff);
    // The same rows at new heights (a resize, or a font, number or tab change while wrapped)
    // keep the row at the top in place.
    let anchor = cache.refresh_heights(lines, &gap_lines, style.row_h, style.tab, wrap);
    let scroll_to = scroll_to.or(anchor.then_some(st.view.diff_top_row));
    let m = cache.row_metrics(style.row_h);
    let marks = st.pull_request.as_ref().filter(|v| v.shows(target)).and_then(|v| pull_request_threads::marks(v, target.path()));
    let marks = marks.as_ref();
    let hunk_gen = st.hunk_ops_gen();
    let t = st.settings.lang();
    let view = DiffView { target, diff, whole, opened, gaps, gap_lines, hl, header_hl, hunk_gen, sel, style, wrap, marks, t };
    Some(match layout {
        DiffLayout::Inline => inline::show(ui, &view, &cache.inline, cache.widest, m, scroll_to, msgs),
        DiffLayout::Split => split::show(ui, &view, &cache.split, cache.widest, m, scroll_to, msgs),
    })
}

/// What the modifiers held make of a click or a drag on a line: Shift for a range, Ctrl/Cmd to
/// toggle (or, for a drag, to add).
fn how(ui: &Ui) -> LineClick {
    let m = ui.input(|i| i.modifiers);
    if m.shift {
        LineClick::Range
    } else if m.command {
        LineClick::Toggle
    } else {
        LineClick::Only
    }
}

/// Click handling for a diff line (a row in inline, a cell in split): `+`/`-` lines are
/// selected (Shift for a range, Ctrl/Cmd to toggle); a plain click elsewhere clears the
/// selection. A press on a `+`/`-` line that moves becomes a drag (`StartLineDrag`). Returns
/// whether the line is selectable and hovered.
pub(super) fn line_click(ui: &Ui, resp: &Response, hunk: usize, line: usize, origin: char, msgs: &mut Vec<Msg>) -> bool {
    let changed = matches!(origin, '+' | '-');
    let hovered = changed && resp.hovered();
    if hovered {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    if changed && resp.drag_started() {
        msgs.push(Msg::Diff(DiffMsg::StartLineDrag { hunk, line, how: how(ui) }));
    }
    if resp.clicked() {
        let how = how(ui);
        if changed {
            msgs.push(Msg::Diff(DiffMsg::SelectDiffLine { hunk, line, how }));
        } else if how == LineClick::Only {
            msgs.push(Msg::Diff(DiffMsg::ClearLineSelection));
        }
    }
    hovered
}

/// How a selectable line senses the pointer: changed lines can also be dragged across; other
/// lines leave a drag to the scroll area.
pub(super) fn line_sense(origin: char) -> Sense {
    if matches!(origin, '+' | '-') { Sense::click_and_drag() } else { Sense::click() }
}

/// During a line drag, reports the row at `rect` (line `line` of hunk `hunk`; 0 for a hunk
/// header or gap row) when the pointer is over it and it isn't the row last reported.
/// `rect_contains_pointer` respects the clip rect and works while the drag's origin holds the
/// pointer, unlike `hovered`.
pub(super) fn drag_over(ui: &Ui, v: &DiffView, rect: Rect, hunk: usize, line: usize, msgs: &mut Vec<Msg>) {
    let Some(d) = v.sel.and_then(LineSelection::dragging) else { return };
    if d.over != (hunk, line) && ui.rect_contains_pointer(rect) {
        msgs.push(Msg::Diff(DiffMsg::DragDiffLine { hunk, line }));
    }
}

/// The bar on a line's left edge: the accent when selected, faint when hovered.
pub(super) fn paint_mark(ui: &Ui, rect: Rect, selected: bool, hovered: bool) {
    let c = match (selected, hovered) {
        (true, _) => color::accent(),
        (false, true) => color::faint(),
        _ => return,
    };
    let bar = Rect::from_min_size(rect.min, vec2(3.0, rect.height()));
    ui.painter().with_clip_rect(rect.intersect(ui.clip_rect())).rect_filled(bar, 0.0, c);
}

/// The diff's rows without the header bar: what `show` draws below it, and what views that
/// embed a commit's diff of a file (the file history) draw. A Git LFS pointer on either side
/// shows a summary instead, until "Show pointer text"; a binary file shows its image preview or
/// sizes (`preview`), and a diff without hunks an empty state (whitespace-only, with a way to show it, when Ignore whitespace
/// hides the changes). Commit
/// diffs have Revert hunk buttons and line selection for Revert lines (both off while
/// whitespace is hidden).
pub(super) fn body(ui: &mut Ui, st: &mut AppState, cache: &mut UiCache, msgs: &mut Vec<Msg>) {
    let scroll_to = st.view.diff_scroll_to_row.take();
    if let Some(note) = crate::state::lfs_notes::summary(st) {
        let text = crate::state::lfs_notes::summary_text(&note.change, &st.lfs.install);
        lfs_note::summary(ui, &text, msgs);
        return;
    }
    if crate::state::lfs_notes::pointer_shown(st) {
        lfs_note::pointer_bar(ui, msgs);
    }
    {
        let Some((_, diff)) = &st.diff else { return };
        if diff.binary {
            preview::show(ui, st, &mut cache.diff_images);
            return;
        }
        if diff.hunks.is_empty() {
            if st.settings.diff_ignore_whitespace {
                if empty_state::whitespace_only(ui) {
                    msgs.push(Msg::Diff(DiffMsg::SetIgnoreWhitespace(false)));
                }
            } else {
                empty_state::no_changes(ui);
            }
            return;
        }
    }
    record_rows(ui, st, &mut cache.diff_rows, scroll_to, msgs);
}
