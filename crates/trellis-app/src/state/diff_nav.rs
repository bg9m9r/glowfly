//! Next/previous change across files: where a step goes, and the file list a diff came from.

use super::history::{FileHistoryView, HistoryKind};
use super::{AppState, Selection, ViewState};
use trellis_core::diff_rows::step_change;
use trellis_core::repo::{DiffTarget, FileDiff, FileEntry};
use trellis_core::settings::Settings;
use trellis_core::whole_file::{self, Expanded};

/// The change a file opened by stepping lands on.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Landing {
    /// Its first change (stepping forwards).
    First,
    /// Its last change (stepping backwards).
    Last,
}

impl Landing {
    fn for_step(dir: isize) -> Landing {
        if dir > 0 { Landing::First } else { Landing::Last }
    }

    /// The step direction that arrives this way: 1 for `First`, -1 for `Last`.
    pub fn dir(self) -> isize {
        match self {
            Landing::First => 1,
            Landing::Last => -1,
        }
    }
}

/// Where a next/previous change press goes.
#[derive(Clone, PartialEq, Debug)]
pub enum Step {
    /// A further change row in the open diff.
    Row(usize),
    /// The neighbouring changed file, and the change to land on there.
    File(DiffTarget, Landing),
    /// Nowhere: no further change in this direction, in this file or the list.
    End,
}

/// What a step does, for the header buttons.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum StepKind {
    Change,
    File,
    End,
}

impl Step {
    pub fn kind(&self) -> StepKind {
        match self {
            Step::Row(_) => StepKind::Change,
            Step::File(..) => StepKind::File,
            Step::End => StepKind::End,
        }
    }
}

/// The row stepping continues from: the change last stepped to while it is still on screen
/// (it may sit below the top when the scroll clamps at the end), otherwise the top row.
pub fn position(v: &ViewState) -> usize {
    match v.diff_cursor {
        Some(c) if c >= v.diff_top_row && c < v.diff_top_row + v.diff_visible_rows.max(1) => c,
        _ => v.diff_top_row,
    }
}

/// The rows next/previous change stops at in `d`, counted with the whole-file folds in `ex`
/// open; none for a binary diff.
pub fn change_rows_of(d: &FileDiff, s: &Settings, ex: &Expanded) -> Vec<usize> {
    if d.binary { Vec::new() } else { whole_file::change_rows(d, s.diff_mode, s.diff_layout, ex) }
}

/// Where a step of `dir` (1 next, -1 previous) from the open diff goes.
pub fn step(st: &AppState, dir: isize) -> Step {
    let Some((target, d)) = st.diff.as_ref().filter(|_| dir != 0) else { return Step::End };
    // The LFS summary shows instead of the rows: nothing to stop at inside the file.
    let rows = if super::lfs_notes::summary(st).is_some() { Vec::new() } else { change_rows_of(d, &st.settings, &st.view.diff_expanded) };
    if let Some(row) = step_change(&rows, position(&st.view), dir) {
        return Step::Row(row);
    }
    match next_file(st, target, dir) {
        Some(t) => Step::File(t, Landing::for_step(dir)),
        None => Step::End,
    }
}

/// The hunk the Revert key acts on in the open diff, as a normal hunk index (also in the
/// whole-file view): the change at or above where stepping continues from (the change last
/// stepped to while on screen, else the top row), else the first one. `None` without changes.
pub fn current_hunk(st: &AppState) -> Option<usize> {
    let (_, d) = st.diff.as_ref()?;
    let rows = change_rows_of(d, &st.settings, &st.view.diff_expanded);
    if rows.is_empty() {
        return None;
    }
    let i = rows.partition_point(|&r| r <= position(&st.view)).saturating_sub(1);
    whole_file::hunk_of_change(d, st.settings.diff_mode, i)
}

/// The file after (`dir` 1) or before (-1) `from` in the list it came from; nothing wraps.
pub fn next_file(st: &AppState, from: &DiffTarget, dir: isize) -> Option<DiffTarget> {
    if dir == 0 {
        return None;
    }
    let files = source_files(st, from)?;
    let i = files.iter().position(|t| t == from)?;
    let j = if dir > 0 { i.checked_add(1)? } else { i.checked_sub(1)? };
    files.get(j).cloned()
}

/// The selected folder entry's files, when `from` is one of that commit's diffs.
fn history_files(h: &FileHistoryView, from: &DiffTarget) -> Option<Vec<(DiffTarget, char)>> {
    let DiffTarget::Commit(oid, _) = from else { return None };
    let e = h.selected_entry().filter(|e| h.kind == HistoryKind::Folder && e.oid == *oid)?;
    Some(e.files.iter().map(|f| (DiffTarget::Commit(*oid, f.path.clone()), f.status)).collect())
}

/// Whether `t` is in the file list it was opened from (`source_files`).
pub fn in_list(st: &AppState, t: &DiffTarget) -> bool {
    source_files(st, t).is_some_and(|files| files.contains(t))
}

/// The file list `from` was opened from, in on-screen order, as diff targets: the Unstaged then
/// the Staged Files list (conflicted entries open the conflict view, so they are left out), the
/// commit details list, the open pull request's list, the comparison list, a stacked list's
/// files across its commits, or in a history the selected folder entry's files. `None` in a file history, which shows one file, or when the
/// list isn't the one on screen.
fn source_files(st: &AppState, from: &DiffTarget) -> Option<Vec<DiffTarget>> {
    Some(source_entries(st, from)?.into_iter().map(|(t, _)| t).collect())
}

/// `source_files` with each file's status letter.
fn source_entries(st: &AppState, from: &DiffTarget) -> Option<Vec<(DiffTarget, char)>> {
    if let Some(h) = &st.history {
        return history_files(h, from);
    }
    let of = |files: &[FileEntry], make: &dyn Fn(String) -> DiffTarget| -> Vec<(DiffTarget, char)> {
        files.iter().map(|f| (make(f.path.clone()), f.status)).collect()
    };
    match from {
        DiffTarget::Unstaged(_) | DiffTarget::Staged(_) => {
            let snap = st.snap.as_ref()?;
            let unstaged = snap.unstaged.iter().filter(|f| f.status != '!').map(|f| (DiffTarget::Unstaged(f.path.clone()), f.status));
            let staged = snap.staged.iter().map(|f| (DiffTarget::Staged(f.path.clone()), f.status));
            Some(unstaged.chain(staged).collect())
        }
        DiffTarget::Commit(oid, _) => {
            if let Some(d) = st.detail.as_ref().filter(|d| d.oid == *oid) {
                return Some(of(&d.files, &|p| DiffTarget::Commit(*oid, p)));
            }
            // A stacked list: every section's files, commit by commit.
            let Selection::Commits(m) = &st.selection else { return None };
            let Some(Ok(stack)) = &m.stacked else { return None };
            stack
                .iter()
                .any(|c| c.oid == *oid)
                .then(|| stack.iter().flat_map(|c| c.files.iter().map(|f| (DiffTarget::Commit(c.oid, f.path.clone()), f.status))).collect())
        }
        DiffTarget::Between(a, b, _) => {
            if let Some(files) = st.pull_request.as_ref().and_then(|v| v.diff_files(*a, *b)) {
                return Some(of(files, &|p| DiffTarget::Between(*a, *b, p)));
            }
            match &st.selection {
                Selection::Commits(m) if m.compare_ends() == Some((*a, *b)) => match &m.changes {
                    Some(Ok(c)) => Some(of(&c.files, &|p| DiffTarget::Between(*a, *b, p))),
                    _ => None,
                },
                _ => None,
            }
        }
        // Only the list of the comparison's current direction: a ToWorkdir target is never
        // walked through a swapped list, or the reverse.
        DiffTarget::ToWorkdir(o, _) | DiffTarget::FromWorkdir(o, _) => match &st.selection {
            Selection::VsWorkdir(w) if w.oid == *o && w.swapped == matches!(from, DiffTarget::FromWorkdir(..)) => match &w.changes {
                Some(Ok(c)) => Some(of(&c.files, &|p| w.target(p))),
                _ => None,
            },
            _ => None,
        },
    }
}

/// Where the header's counter stands: change `index` of `total` in the open diff, or with no
/// change rows (`files`) the file's place in the list it came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Counter {
    pub index: usize,
    pub total: usize,
    pub files: bool,
}

/// The open diff's counter: over its change rows (the changes at or above where stepping
/// continues from, at least 1, as `current_hunk` counts), or for a file without rows (binary,
/// empty, an LFS summary) its place in the list it came from. `None` when neither applies.
pub fn counter(st: &AppState) -> Option<Counter> {
    let (target, d) = st.diff.as_ref()?;
    let rows = if super::lfs_notes::summary(st).is_some() { Vec::new() } else { change_rows_of(d, &st.settings, &st.view.diff_expanded) };
    if !rows.is_empty() {
        let index = rows.partition_point(|&r| r <= position(&st.view)).max(1);
        return Some(Counter { index, total: rows.len(), files: false });
    }
    let files = source_files(st, target)?;
    let i = files.iter().position(|t| t == target)?;
    Some(Counter { index: i + 1, total: files.len(), files: true })
}

/// `target`'s status letter in the list it came from (M, A, D, R, T, ?), when it is listed.
pub fn file_status(st: &AppState, target: &DiffTarget) -> Option<char> {
    if let Some(s) = source_entries(st, target).and_then(|e| e.into_iter().find(|(t, _)| t == target).map(|(_, s)| s)) {
        return Some(s);
    }
    // A file history's commit diff isn't in a list but its detail may still name it.
    let DiffTarget::Commit(oid, path) = target else { return None };
    st.detail.as_ref().filter(|d| d.oid == *oid)?.files.iter().find(|f| f.path == *path).map(|f| f.status)
}
