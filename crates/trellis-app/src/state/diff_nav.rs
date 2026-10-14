//! Next/previous change across files: where a step goes, and the file list a diff came from.

use super::{AppState, Selection, ViewState};
use trellis_core::diff_rows::step_change;
use trellis_core::repo::{DiffTarget, FileDiff};
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
    if let Some(row) = step_change(&change_rows_of(d, &st.settings, &st.view.diff_expanded), position(&st.view), dir) {
        return Step::Row(row);
    }
    match next_file(st, target, dir) {
        Some(t) => Step::File(t, Landing::for_step(dir)),
        None => Step::End,
    }
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

/// The file list `from` was opened from, in on-screen order, as diff targets: the Unstaged then
/// the Staged Files list (conflicted entries open the conflict view, so they are left out), the
/// commit details list, or the comparison list. `None` in the file history, which shows one
/// file, or when the list isn't the one on screen.
fn source_files(st: &AppState, from: &DiffTarget) -> Option<Vec<DiffTarget>> {
    if st.history.is_some() {
        return None;
    }
    match from {
        DiffTarget::Unstaged(_) | DiffTarget::Staged(_) => {
            let snap = st.snap.as_ref()?;
            let unstaged = snap.unstaged.iter().filter(|f| f.status != '!').map(|f| DiffTarget::Unstaged(f.path.clone()));
            let staged = snap.staged.iter().map(|f| DiffTarget::Staged(f.path.clone()));
            Some(unstaged.chain(staged).collect())
        }
        DiffTarget::Commit(oid, _) => {
            let d = st.detail.as_ref().filter(|d| d.oid == *oid)?;
            Some(d.files.iter().map(|f| DiffTarget::Commit(*oid, f.path.clone())).collect())
        }
        DiffTarget::Between(a, b, _) => match &st.selection {
            Selection::Commits(m) if m.pair() == Some((*a, *b)) => match &m.changes {
                Some(Ok(c)) => Some(c.files.iter().map(|f| DiffTarget::Between(*a, *b, f.path.clone())).collect()),
                _ => None,
            },
            _ => None,
        },
    }
}
