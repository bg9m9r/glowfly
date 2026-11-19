//! Which line of the working file Open in external editor opens at: the first selected line of
//! the open diff (the gutter's lines or the text selected in the code, whichever is topmost),
//! else the line the row menu was opened on, else the first changed line of the change the
//! header's counter stands on; in a file history's File view the cursor line. A line the working
//! file's side doesn't have (a removed line) maps to the next line of its hunk that it has,
//! else the nearest one before. The rule itself (`choose`) is pure; `of_open_diff`, `on_screen`
//! and `working_file` read it from the state.

use crate::state::diff_header::{self, OpenAction};
use crate::state::diff_nav;
use crate::state::diff_text::{LineRef, TextSel, Unit};
use crate::state::files::not_in_workdir;
use crate::state::history::{FileHistoryView, HistoryPane};
use crate::state::layout::{self, ToolView};
use crate::state::lines::LineSelection;
use crate::state::tool_compare::ToolSetup;
use crate::state::{AppState, ViewState, diff_preview, lfs_notes};
use glowfly_core::diff_rows::{self, LineSide};
use glowfly_core::repo::{DiffLine, DiffTarget, FileDiff};
use std::borrow::Cow;
use std::collections::BTreeSet;

/// What decides the line: the diff, the side the working file is on and the selections and the
/// row menu's line that belong to it.
pub struct Picked<'a> {
    /// The diff's normal hunks (its `whole` serves `LineRef::Whole`).
    pub diff: &'a FileDiff,
    /// The side of the diff the working file is on.
    pub side: LineSide,
    /// The gutter selection: its hunk and the selected line indexes.
    pub lines: Option<(usize, &'a BTreeSet<usize>)>,
    /// The two ends of the text selection.
    pub text: Option<[LineRef; 2]>,
    /// The line the row menu was opened on.
    pub at: Option<LineRef>,
}

impl<'a> Picked<'a> {
    /// What `diff` (of `target`, as loaded in generation `shown_gen`) gets from the selections:
    /// a gutter selection of another diff or of a hunk whose header changed, an empty text
    /// selection and one of another diff or an earlier load are left out.
    pub fn of(
        target: &DiffTarget,
        diff: &'a FileDiff,
        lines: Option<&'a LineSelection>,
        text: Option<&TextSel>,
        shown_gen: u64,
        at: Option<LineRef>,
    ) -> Picked<'a> {
        let lines = lines
            .filter(|s| s.target.as_ref() == Some(target) && !s.lines.is_empty())
            .filter(|s| diff.hunks.get(s.hunk).is_some_and(|h| h.header == s.header))
            .map(|s| (s.hunk, &s.lines));
        let text = text
            .filter(|s| s.key.0 == *target && s.key.1 == shown_gen)
            .filter(|s| !(s.unit == Unit::Char && s.anchor == s.head))
            .map(|s| [s.anchor.line, s.head.line]);
        Picked { diff, side: working_side(target), lines, text, at }
    }
}

/// The side of a diff the working file is on: the old one for a diff from the working tree to
/// a commit, the new one otherwise (for a staged diff, a commit's or a comparison of two
/// commits, the nearest the working file has).
fn working_side(target: &DiffTarget) -> LineSide {
    match target {
        DiffTarget::FromWorkdir(..) => LineSide::Old,
        _ => LineSide::New,
    }
}

/// The working file's line shown as `r` (1-based): its own number, else the next line of its
/// hunk that the side has, else the nearest before; `None` when the hunk has none or `r` isn't
/// in the diff.
fn file_line(diff: &FileDiff, side: LineSide, r: LineRef) -> Option<u32> {
    match r {
        LineRef::Hunk { hunk, line } => near(&diff.hunks.get(hunk)?.lines, line, side),
        LineRef::Whole { line } => near(&diff.whole.as_ref()?.diff.hunks.first()?.lines, line, side),
        LineRef::Gap { hunk, line } => {
            let g = diff_rows::gaps(diff).into_iter().find(|g| g.hunk == hunk).filter(|g| line < g.len)?;
            let start = if side == LineSide::New { g.new_start } else { g.old_start };
            start.checked_add(u32::try_from(line).ok()?)
        }
    }
}

fn near(lines: &[DiffLine], i: usize, side: LineSide) -> Option<u32> {
    lines.get(i)?;
    let num = |l: &DiffLine| if side == LineSide::New { l.new } else { l.old };
    lines[i..].iter().find_map(num).or_else(|| lines[..i].iter().rev().find_map(num))
}

/// The line to open at: the first selected line (the gutter's or the text's, the smaller),
/// else the row menu's, else the first changed line of the hunk `hunk` gives (called only when
/// the others give nothing).
pub fn choose(p: &Picked<'_>, hunk: impl FnOnce() -> Option<usize>) -> Option<u32> {
    let line = |r| file_line(p.diff, p.side, r);
    let gutter = p.lines.and_then(|(h, set)| set.first().map(|&line| LineRef::Hunk { hunk: h, line }));
    let selected = gutter.into_iter().chain(p.text.into_iter().flatten()).filter_map(line).min();
    if let Some(n) = selected.or_else(|| p.at.and_then(line)) {
        return Some(n);
    }
    let h = hunk()?;
    let first = p.diff.hunks.get(h)?.lines.iter().position(|l| matches!(l.origin, '+' | '-'))?;
    line(LineRef::Hunk { hunk: h, line: first })
}

/// The line the open diff gives: `at` is the row menu's line. `rows` are its change rows when
/// the caller keeps them (the header), else they are counted when needed. `view` is the view
/// state the caller was given (never `st.view`, which is lent while a view draws). `None` for
/// a diff without text rows (binary, LFS summary, an SVG's preview).
pub fn of_open_diff(st: &AppState, view: &ViewState, at: Option<LineRef>, rows: Option<&[usize]>) -> Option<u32> {
    let (target, diff) = st.session.diff.view.as_ref()?;
    if diff.binary || lfs_notes::summary(st).is_some() || diff_preview::drawn(st) {
        return None;
    }
    let lines = st.line_ops_blocked().is_none().then_some(&st.cues.diff_lines);
    let p = Picked::of(target, diff, lines, view.diff_text.as_ref(), st.session.diff.shown_gen, at);
    choose(&p, || {
        let rows = rows.map_or_else(|| Cow::Owned(diff_nav::shown_change_rows(st)), Cow::Borrowed);
        diff_nav::hunk_from(diff, st.settings.diff_mode, &rows, diff_nav::position(st, view))
    })
}

/// The File view's line: the cursor line, else the first on screen; nothing while a preview is
/// drawn or the text isn't loaded.
fn of_file_view(hv: &FileHistoryView, preview_on: bool) -> Option<u32> {
    if hv.pane != HistoryPane::File || hv.previewing(preview_on) {
        return None;
    }
    let n = hv.shown_text()?.lines.len();
    let line = hv.nav.cursor.unwrap_or(hv.nav.top).min(n.checked_sub(1)?);
    u32::try_from(line + 1).ok()
}

/// The file the Open in editor key opens: the history's file under its name today, or the open
/// diff's.
pub fn shown_file(st: &AppState) -> Option<String> {
    match layout::tool_view(st) {
        Some(ToolView::FileHistory | ToolView::FolderHistory) => st.session.history.view.as_ref()?.shown_today(),
        None | Some(ToolView::PullRequest) => st.session.diff.view.as_ref().map(|(t, _)| t.path().to_owned()),
        Some(_) => None,
    }
}

/// The line for opening `path` when the message names none: the line of the view showing that
/// file, if one is on screen; `view` as in `of_open_diff`.
pub fn on_screen(st: &AppState, view: &ViewState, path: &str) -> Option<u32> {
    match layout::tool_view(st) {
        Some(ToolView::FileHistory | ToolView::FolderHistory) => {
            let hv = st.session.history.view.as_ref()?;
            if hv.shown_today().as_deref() != Some(path) {
                return None;
            }
            match hv.pane {
                HistoryPane::File => of_file_view(hv, st.settings.history.preview),
                HistoryPane::Diff => {
                    let open = st.session.diff.view.as_ref().map(|(t, _)| t);
                    if open.is_some() && open == hv.target().as_ref() { of_open_diff(st, view, None, None) } else { None }
                }
            }
        }
        None | Some(ToolView::PullRequest) => {
            let open = st.session.diff.view.as_ref()?;
            if open.0.path() == path { of_open_diff(st, view, None, None) } else { None }
        }
        Some(_) => None,
    }
}

/// The working file the diff's row menu opens, with why it can't be opened and its Set up note.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditorFile {
    pub path: String,
    pub missing: Option<&'static str>,
    pub setup: Option<&'static str>,
}

/// The row menu's file: in a file or folder history, the file under its name today; otherwise
/// the open diff's working file (as the header's Open names it). Worked out while a menu is open.
pub fn working_file(st: &AppState) -> Option<EditorFile> {
    let t = st.settings.lang();
    let setup = ToolSetup::of(t, &st.settings.external_tools).editor;
    let (path, missing) = match layout::tool_view(st) {
        Some(ToolView::FileHistory | ToolView::FolderHistory) => {
            let hv = st.session.history.view.as_ref()?;
            (hv.shown_today()?, (!hv.shown_in_workdir(st.snap.as_ref())).then(|| not_in_workdir(t)))
        }
        _ => {
            let (target, _) = st.session.diff.view.as_ref()?;
            let status = diff_nav::file_order::file_order(st, target).status;
            let OpenAction { path, missing } = diff_header::open_of(st, target, status)?;
            (path, missing)
        }
    };
    Some(EditorFile { path, missing, setup })
}

#[cfg(test)]
#[path = "editor_line_tests.rs"]
mod tests;
