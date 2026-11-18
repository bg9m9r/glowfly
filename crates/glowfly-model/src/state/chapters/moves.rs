//! Moving the selection over the Chapters list with the keyboard. A position counts the work
//! tree's row first (when there is one), then `ChaptersView::rows`; release headings can't be
//! selected, so moves skip them.

use super::{ChaptersView, Row, is_selected};
use crate::state::list_keys::{ListMove, Place};
use crate::state::selection::Selection;
use glowfly_core::repo::Snapshot;

fn selectable(rows: &[Row], wip: bool, d: usize) -> bool {
    match d.checked_sub(usize::from(wip)) {
        None => true,
        Some(i) => rows.get(i).is_some_and(|r| !matches!(r, Row::Release(_))),
    }
}

/// The position `mv` lands on from `from` (None: nothing selected), over the work tree's row and
/// `rows`; None when it stays where it is. Headings are skipped; a start on one (never
/// selected) counts as between its neighbours.
pub fn step(rows: &[Row], wip: bool, from: Option<usize>, mv: ListMove) -> Option<usize> {
    let total = rows.len() + usize::from(wip);
    let ok: Vec<usize> = (0..total).filter(|&d| selectable(rows, wip, d)).collect();
    let place = match from {
        None => Place::Outside,
        Some(f) => match ok.iter().position(|&d| d == f) {
            Some(i) => Place::At(i),
            None => Place::Hidden { before: ok.iter().take_while(|&&d| d < f).count() },
        },
    };
    mv.target(ok.len(), place).map(|i| ok[i])
}

/// The position of what is selected: the work tree's row when it is lit, a commit row of the
/// selected commit, else the chapter row the selection is exactly that of.
pub fn current(view: &ChaptersView, snap: &Snapshot, sel: &Selection, wip_lit: bool, wip: bool) -> Option<usize> {
    if wip && wip_lit && matches!(sel, Selection::Wip) {
        return Some(0);
    }
    let list = view.list.as_ref()?;
    let base = usize::from(wip);
    if let Selection::Commit(oid) = sel {
        let at = view.rows.iter().position(|r| match r {
            Row::Commit { row, .. } => snap.commits.get(*row as usize).is_some_and(|c| c.oid == *oid),
            _ => false,
        });
        if let Some(i) = at {
            return Some(i + base);
        }
    }
    let chapter = view.rows.iter().position(|r| match r {
        Row::Chapter(c) => list.list.get(*c as usize).is_some_and(|ch| is_selected(ch, snap, sel)),
        _ => false,
    });
    chapter.map(|i| i + base)
}

/// The chapter row of the commit row at position `d`.
pub fn parent_row(rows: &[Row], wip: bool, d: usize) -> Option<usize> {
    let Row::Commit { chapter, .. } = *rows.get(d.checked_sub(usize::from(wip))?)? else { return None };
    rows.iter().position(|r| *r == Row::Chapter(chapter)).map(|i| i + usize::from(wip))
}
