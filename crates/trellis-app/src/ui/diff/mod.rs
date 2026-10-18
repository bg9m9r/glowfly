//! The diff viewer: one column (inline) or old and new side by side (split), showing the hunks
//! or the whole file, with hunk- and line-level stage, unstage and discard, Revert Hunk and
//! Revert lines on commit and comparison diffs, and next/previous change navigation. In the whole-file view long unchanged
//! runs are folded into "N unchanged lines" rows that open in place when clicked; an opened
//! run folds back from a control on its first line, and the header can open or close them all.
//! With wrapping on, long lines continue on the next line and their rows grow to fit.

mod fold;
mod header;
mod inline;
pub(super) mod rows_cache;
mod split;
mod style;
mod syntax;
mod thread_marks;
mod wrap;

use super::file_buttons::CommitFile;
use super::theme::color;
use crate::msg::DiffMsg;
use crate::msg::Msg;
use crate::msg::StageMsg;
use crate::state::AppState;
use crate::state::diff_nav;
use crate::state::lines::{LineClick, LineSelection};
use crate::state::pull_request_threads::{self, Marks};
use eframe::egui::{self, CursorIcon, FontId, Layout, Rect, Response, RichText, TextStyle, Ui, pos2, vec2};
use rows_cache::{DiffRowsCache, Source};
pub(super) use style::DiffStyle;
pub(super) use syntax::paint_line;
use trellis_core::highlight::{self, DiffHighlight};
use trellis_core::repo::{DiffTarget, FileDiff, HunkOp};
use trellis_core::settings::DiffLayout;
use trellis_core::whole_file::{self, Fold, WholeFile, folds};

/// The default row height and font, which the conflict view keeps; the diff viewer draws
/// with `DiffStyle` from the settings.
pub(super) const ROW_H: f32 = 19.0;

pub(super) fn mono() -> FontId {
    FontId::monospace(12.5)
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
    /// Syntax spans of the diff the lines come from (`lines()`), `None` to paint plain.
    pub hl: Option<&'a DiffHighlight>,
    /// Syntax spans for the hunk headers' function context: the normal hunks' highlight, also
    /// in the whole-file view, whose action rows show those headers; `None` to paint headers
    /// plain.
    pub header_hl: Option<&'a DiffHighlight>,
    /// The diff's generation, or `None` while a reload is on its way (see `hunk_header`).
    pub hunk_gen: Option<u64>,
    /// The line selection; `None` when lines can't be selected (the whole-file view, or while
    /// hunk and line operations are blocked).
    pub sel: Option<&'a LineSelection>,
    /// Why hunk and line operations (including Revert Hunk) are off for reasons other than a
    /// pending reload (e.g. whitespace hidden).
    pub blocked: Option<&'static str>,
    /// Font, row height and gutters from Preferences > Editor.
    pub style: DiffStyle,
    /// How long lines wrap, `None` when they don't.
    pub wrap: Option<wrap::Wrap>,
    /// Code comment markers by file line, for a pull request's file whose threads can be
    /// placed on its lines.
    pub marks: Option<&'a Marks>,
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

pub fn show(ui: &mut Ui, st: &mut AppState, rows_cache: &mut DiffRowsCache, msgs: &mut Vec<Msg>) {
    let scroll_to = st.view.diff_scroll_to_row.take();
    let ignore_ws = st.settings.diff_ignore_whitespace;
    // Where Previous and Next go: a change in this file, another file, or nowhere.
    let nav = [diff_nav::step(st, -1).kind(), diff_nav::step(st, 1).kind()];
    {
        let Some((target, diff)) = &st.diff else { return };
        let has_rows = !diff.binary && !diff.hunks.is_empty();
        ui.add_space(6.0);
        let rows = has_rows.then_some((st.settings.diff_layout, st.settings.diff_mode));
        // How many folds of the whole file on screen are open and closed, when it has any.
        let fold_counts = whole_file::shown(diff, st.settings.diff_mode)
            .map(|w| {
                let all = folds(w);
                let open = all.iter().filter(|f| st.view.diff_expanded.contains(f.key)).count();
                header::FoldCounts { open, closed: all.len() - open }
            })
            .filter(|c| c.open + c.closed > 0 && has_rows);
        // Restore, Open and Show for a commit's file, when the open detail lists it.
        let commit_file = match target {
            DiffTarget::Commit(oid, path) => st.detail.as_ref().filter(|d| d.oid == *oid).and_then(|d| {
                d.files.iter().find(|f| f.path == *path).map(|f| CommitFile {
                    oid: *oid,
                    path: &f.path,
                    status: f.status,
                    open_path: &f.path,
                    in_workdir: d.in_workdir(&f.path),
                })
            }),
            _ => None,
        };
        let toggles =
            header::Toggles { ignore_ws: (!diff.binary).then_some(ignore_ws), wrap: has_rows.then_some(st.settings.diff_word_wrap) };
        let diff_tool = crate::state::tool_compare::diff_blocked(&st.settings.external_tools);
        header::header_bar(ui, target, commit_file.as_ref(), rows, nav, toggles, fold_counts, &st.keymap, diff_tool, msgs);
        ui.separator();
        if diff.binary {
            ui.label(RichText::new("Binary file not shown.").color(color::weak()));
            return;
        }
        if diff.hunks.is_empty() {
            let text = if ignore_ws {
                "Only whitespace changed in this file. Turn off Ignore whitespace to see it."
            } else {
                "No textual changes (mode or empty file)."
            };
            ui.label(RichText::new(text).color(color::weak()));
            return;
        }
    }
    record_rows(ui, st, rows_cache, scroll_to, msgs);
}

/// Draws the open diff's rows and records the first visible row and how many rows fit.
fn record_rows(ui: &mut Ui, st: &mut AppState, cache: &mut DiffRowsCache, scroll_to: Option<usize>, msgs: &mut Vec<Msg>) {
    let blocked = st.hunk_ops_blocked();
    if let Some((top, visible)) = rows(ui, st, cache, blocked, scroll_to, msgs) {
        st.view.diff_top_row = top;
        st.view.diff_visible_rows = visible;
    }
}

/// Draws the open diff's rows in the current layout and view mode (hunks, or the whole file
/// when it loaded) and returns the first visible row and how many rows fit. Lines are
/// selectable in every diff unless line operations are blocked.
fn rows(
    ui: &mut Ui,
    st: &AppState,
    cache: &mut DiffRowsCache,
    blocked: Option<&'static str>,
    scroll_to: Option<usize>,
    msgs: &mut Vec<Msg>,
) -> Option<(usize, usize)> {
    let (target, diff) = st.diff.as_ref()?;
    let whole = whole_file::shown(diff, st.settings.diff_mode);
    // Highlighting always comes with the diff; the setting only decides whether to paint it.
    let hl = whole.map_or(diff.highlight.as_ref(), |w| w.diff.highlight.as_ref()).filter(|_| st.settings.syntax_highlighting);
    let header_hl = diff.highlight.as_ref().filter(|_| st.settings.syntax_highlighting);
    let selectable = st.line_ops_blocked().is_none();
    let sel = selectable.then_some(&st.view.diff_lines);
    let style = DiffStyle::new(&st.settings);
    let expanded = &st.view.diff_expanded;
    let opened = whole.map(|w| whole_file::opened(w, expanded)).unwrap_or_default();
    let layout = st.settings.diff_layout;
    let generation = (st.diff_gen, st.diff_shown_gen);
    cache.refresh(Source { diff, whole, generation, layout, expanded, tab: style.tab });
    let wrap = wrap::wrap_for(ui, &style, layout);
    let lines: &FileDiff = whole.map_or(diff, |w| &w.diff);
    // The same rows at new heights (a resize, or a font, number or tab change while wrapped)
    // keep the row at the top in place.
    let anchor = cache.refresh_heights(lines, style.row_h, style.tab, wrap);
    let scroll_to = scroll_to.or(anchor.then_some(st.view.diff_top_row));
    let m = cache.row_metrics(style.row_h);
    let marks = st.pull_request.as_ref().filter(|v| v.shows(target)).and_then(|v| pull_request_threads::marks(v, target.path()));
    let marks = marks.as_ref();
    let view = DiffView { target, diff, whole, opened, hl, header_hl, hunk_gen: st.hunk_ops_gen(), sel, blocked, style, wrap, marks };
    Some(match layout {
        DiffLayout::Inline => inline::show(ui, &view, &cache.inline, cache.widest, m, scroll_to, msgs),
        DiffLayout::Split => split::show(ui, &view, &cache.split, cache.widest, m, scroll_to, msgs),
    })
}

/// A hunk's action button.
#[derive(Clone, Copy)]
enum HunkButton {
    Op(HunkOp),
    /// Apply the inverse of the hunk, or of its selected lines, to the working tree (commit and
    /// comparison diffs).
    Revert,
}

/// A hunk header row: the `@@` line (its function context in syntax colors), plus its buttons
/// pinned to the visible right edge so they stay in view when scrolled sideways: Stage /
/// Unstage / Discard Hunk on working-tree diffs (or Stage / Unstage / Discard N lines when the
/// hunk has selected lines), Revert Hunk (or Revert N lines) on commit and comparison diffs. The buttons are
/// disabled while a reload is on its way (`hunk_gen` is `None`), since the hunk indexes may no
/// longer match the file, and while `blocked` (whitespace hidden: the shown hunks aren't the
/// ones the operations count).
pub(super) fn hunk_header(ui: &mut Ui, rect: Rect, v: &DiffView, h: usize, msgs: &mut Vec<Msg>) {
    let p = ui.painter();
    p.rect_filled(rect, 0.0, color::hunk_bg());
    let raw = &v.diff.hunks[h].header;
    let ctx = v.header_hl.map(|hl| hl.header(h)).filter(|s| !s.is_empty()).and_then(|s| highlight::context_start(raw).map(|at| (at, s)));
    syntax::paint_header(p, pos2(rect.left() + 8.0, rect.center().y), raw, ctx, &v.style.font, v.style.tab);
    let path = v.target.path();
    let selected = v.sel.map_or(0, |s| s.count_in(v.target, h));
    let buttons: &[HunkButton] = match v.target {
        DiffTarget::Unstaged(_) => &[HunkButton::Op(HunkOp::Discard), HunkButton::Op(HunkOp::Stage)],
        DiffTarget::Staged(_) => &[HunkButton::Op(HunkOp::Unstage)],
        DiffTarget::Commit(..) | DiffTarget::Between(..) => &[HunkButton::Revert],
    };
    let small = TextStyle::Small.resolve(ui.style());
    let mut x = ui.clip_rect().right() - 8.0;
    for &b in buttons {
        let verb = match b {
            HunkButton::Revert => "Revert",
            HunkButton::Op(HunkOp::Stage) => "Stage",
            HunkButton::Op(HunkOp::Unstage) => "Unstage",
            HunkButton::Op(HunkOp::Discard) => "Discard",
        };
        let label = match selected {
            0 => format!("{verb} Hunk"),
            1 => format!("{verb} 1 line"),
            n => format!("{verb} {n} lines"),
        };
        let text_w = ui.fonts(|f| f.layout_no_wrap(label.clone(), small.clone(), color::text()).size().x);
        let w = (text_w + 16.0).max(96.0);
        let r = Rect::from_min_size(pos2(x - w, rect.top() + 1.0), vec2(w, v.style.row_h - 2.0));
        x -= w + 6.0;
        let mut builder = egui::UiBuilder::new().max_rect(r).layout(Layout::centered_and_justified(egui::Direction::TopDown));
        let reason = v.blocked.or(v.hunk_gen.is_none().then_some("Updating the diff..."));
        if reason.is_some() {
            builder = builder.disabled();
        }
        let mut button = ui
            .allocate_new_ui(builder, |ui| ui.add(egui::Button::new(RichText::new(label).small())))
            .inner
            .on_disabled_hover_text(reason.unwrap_or_default());
        if let HunkButton::Revert = b {
            let what = if selected > 0 { "the selected changes" } else { "this change" };
            button = button.on_hover_text(format!("Undo {what} in your working copy of {path}. The commit itself is not changed."));
        }
        let (true, Some(generation)) = (button.clicked(), v.hunk_gen) else { continue };
        let op = match b {
            HunkButton::Revert => {
                msgs.push(Msg::Stage(if selected > 0 { StageMsg::RevertLines } else { StageMsg::RevertHunk { index: h, generation } }));
                continue;
            }
            HunkButton::Op(op) => op,
        };
        let m = if selected > 0 {
            Msg::Stage(StageMsg::ApplyLines(op))
        } else {
            Msg::Stage(StageMsg::Hunk { op, path: path.to_owned(), index: h, generation })
        };
        msgs.push(match (op, selected) {
            (HunkOp::Discard, 0) => {
                Msg::confirm("Discard hunk", format!("Discard this change in {path}? You can bring it back with Undo."), "Discard", m)
            }
            (HunkOp::Discard, n) => {
                let what = if n == 1 { "the selected line".to_owned() } else { format!("the {n} selected lines") };
                Msg::confirm("Discard lines", format!("Discard {what} in {path}? You can bring them back with Undo."), "Discard", m)
            }
            _ => m,
        });
    }
}

/// Click handling for a diff line (a row in inline, a cell in split): `+`/`-` lines are
/// selected (Shift for a range, Ctrl/Cmd to toggle); a plain click elsewhere clears the
/// selection. Returns whether the line is selectable and hovered.
pub(super) fn line_click(ui: &Ui, resp: &Response, hunk: usize, line: usize, origin: char, msgs: &mut Vec<Msg>) -> bool {
    let changed = matches!(origin, '+' | '-');
    let hovered = changed && resp.hovered();
    if hovered {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    if resp.clicked() {
        let m = ui.input(|i| i.modifiers);
        let how = if m.shift {
            LineClick::Range
        } else if m.command {
            LineClick::Toggle
        } else {
            LineClick::Only
        };
        if changed {
            msgs.push(Msg::Diff(DiffMsg::SelectDiffLine { hunk, line, how }));
        } else if how == LineClick::Only {
            msgs.push(Msg::Diff(DiffMsg::ClearLineSelection));
        }
    }
    hovered
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

/// The diff's rows without the header bar, for views that embed a commit's diff of a file (the
/// file history). Commit diffs have Revert Hunk buttons and line selection for Revert lines
/// (both off while whitespace is hidden).
pub(super) fn body(ui: &mut Ui, st: &mut AppState, rows_cache: &mut DiffRowsCache, msgs: &mut Vec<Msg>) {
    let scroll_to = st.view.diff_scroll_to_row.take();
    {
        let Some((_, diff)) = &st.diff else { return };
        if diff.binary {
            ui.label(RichText::new("Binary file not shown.").color(color::weak()));
            return;
        }
        if diff.hunks.is_empty() {
            ui.label(RichText::new("No textual changes (mode or empty file).").color(color::weak()));
            return;
        }
    }
    record_rows(ui, st, rows_cache, scroll_to, msgs);
}
