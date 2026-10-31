//! Next/previous change across files: where a step goes, the counter between the arrows, and the
//! neighbouring file in the list a diff came from (`diff_nav/file_order.rs`).

pub mod file_order;

use super::AppState;
use file_order::source_entries;
use glowfly_core::diff_rows::step_change;
use glowfly_core::repo::{DiffTarget, FileDiff};
use glowfly_core::settings::Settings;
use glowfly_core::whole_file::{self, Expanded};

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
pub fn position(st: &AppState) -> usize {
    let top = st.session.tab_view.diff_top_row;
    match st.view.diff_cursor {
        Some(c) if c >= top && c < top + st.view.diff_visible_rows.max(1) => c,
        _ => top,
    }
}

/// The rows next/previous change stops at in `d`, counted with the whole-file folds in `ex`
/// open; none for a binary diff.
pub fn change_rows_of(d: &FileDiff, s: &Settings, ex: &Expanded) -> Vec<usize> {
    if d.binary { Vec::new() } else { whole_file::change_rows(d, s.diff_mode, s.diff_layout, ex) }
}

/// The open diff's change rows as stepping and the header see them: none while the LFS summary
/// shows instead of the rows.
pub fn shown_change_rows(st: &AppState) -> Vec<usize> {
    let Some((_, d)) = st.session.diff.view.as_ref() else { return Vec::new() };
    if super::lfs_notes::summary(st).is_some() { Vec::new() } else { change_rows_of(d, &st.settings, &st.session.tab_view.diff_expanded) }
}

/// Where a step of `dir` (1 next, -1 previous) from the open diff goes.
pub fn step(st: &AppState, dir: isize) -> Step {
    let Some((target, _)) = st.session.diff.view.as_ref().filter(|_| dir != 0) else { return Step::End };
    step_from(&shown_change_rows(st), position(st), dir, || next_file(st, target, dir))
}

/// Where a step of `dir` from row `pos` goes: the next of `rows` that way, else the file `file`
/// gives.
pub fn step_from(rows: &[usize], pos: usize, dir: isize, file: impl FnOnce() -> Option<DiffTarget>) -> Step {
    if dir == 0 {
        return Step::End;
    }
    if let Some(row) = step_change(rows, pos, dir) {
        return Step::Row(row);
    }
    match file() {
        Some(t) => Step::File(t, Landing::for_step(dir)),
        None => Step::End,
    }
}

/// The hunk the Revert key acts on in the open diff, as a normal hunk index (also in the
/// whole-file view): the change at or above where stepping continues from (the change last
/// stepped to while on screen, else the top row), else the first one. `None` without changes.
pub fn current_hunk(st: &AppState) -> Option<usize> {
    let (_, d) = st.session.diff.view.as_ref()?;
    let rows = change_rows_of(d, &st.settings, &st.session.tab_view.diff_expanded);
    if rows.is_empty() {
        return None;
    }
    let i = rows.partition_point(|&r| r <= position(st)).saturating_sub(1);
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

/// Whether `t` is in the file list it was opened from (`source_files`).
pub fn in_list(st: &AppState, t: &DiffTarget) -> bool {
    source_files(st, t).is_some_and(|files| files.contains(t))
}

/// The file list `from` was opened from, in on-screen order, as diff targets
/// (`file_order::source_entries`).
fn source_files(st: &AppState, from: &DiffTarget) -> Option<Vec<DiffTarget>> {
    Some(source_entries(st, from)?.into_iter().map(|(t, _)| t).collect())
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
    let (target, _) = st.session.diff.view.as_ref()?;
    counter_from(&shown_change_rows(st), position(st), || {
        let files = source_files(st, target)?;
        Some((files.iter().position(|t| t == target)?, files.len()))
    })
}

/// The counter over `rows` from row `pos`, else from the file's index and list length `place`
/// gives.
pub fn counter_from(rows: &[usize], pos: usize, place: impl FnOnce() -> Option<(usize, usize)>) -> Option<Counter> {
    if !rows.is_empty() {
        let index = rows.partition_point(|&r| r <= pos).max(1);
        return Some(Counter { index, total: rows.len(), files: false });
    }
    let (i, total) = place()?;
    Some(Counter { index: i + 1, total, files: true })
}
