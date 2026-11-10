//! The files picked with Shift or Ctrl/Cmd in Conflicted Files or Resolved Files (shown during
//! an operation in progress), gaps allowed (`ConflictPicks`, read as `wip_lists::Picked`): the
//! two lists' files in on-screen order, the file each list has open (counted as picked while
//! nothing is), a Shift run (`range`), which rows draw selected (`drawn_selected`) and `prune`
//! after a status read. Unlike the Unstaged and Staged Files picks they don't open anything, so
//! the open file may be left out of them. Read by `update/conflict_picks.rs` and the lists' views.

use super::wip_lists::Picked;
use crate::state::AppState;
use glowfly_core::repo::DiffTarget;
use std::collections::BTreeSet;

/// The files picked in Conflicted Files (`resolved: false`) or Resolved Files; `anchor` is where
/// the next Shift range starts. View state, changed only by `update`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConflictPicks {
    pub resolved: bool,
    pub anchor: String,
    pub paths: BTreeSet<String>,
}

/// That list's files in on-screen order: the conflicted entries in the snapshot's order, or
/// every resolved row (with a staged change or not).
pub fn order(st: &AppState, resolved: bool) -> Vec<String> {
    let Some(snap) = st.snap.as_ref() else { return Vec::new() };
    if resolved {
        st.session.conflict_files.resolved_rows(snap).into_iter().map(|r| r.entry.path.clone()).collect()
    } else {
        snap.unstaged.iter().filter(|f| f.status == '!').map(|f| f.path.clone()).collect()
    }
}

/// The file that list has open: the conflict view's, or the resolved file whose staged diff is
/// on screen.
pub fn current(st: &AppState, resolved: bool) -> Option<String> {
    if !resolved {
        return st.session.conflict.view.as_ref().map(|c| c.path.clone());
    }
    let path = match super::wip_lists::cursor(st)? {
        DiffTarget::Staged(p) => p,
        _ => return None,
    };
    order(st, true).into_iter().find(|p| p == path)
}

/// The picks of that list, if it holds them.
pub fn held(st: &AppState, resolved: bool) -> Option<&ConflictPicks> {
    st.cues.conflict_picks.as_ref().filter(|p| p.resolved == resolved)
}

/// Whether the row of `path` draws selected, given that list's picks (`held`) and whether the
/// row's file is the one open: while the list holds picks only they do (a single one left too),
/// so the open file taken out of them doesn't look picked and every row drawn selected is one
/// the actions take; with none, the open file.
pub fn drawn_selected(held: Option<&ConflictPicks>, path: &str, open: bool) -> bool {
    held.map_or(open, |p| p.paths.contains(path))
}

/// The files picked in that list in on-screen order, for the rows to draw selected and the
/// actions to take; None unless there are several.
pub fn picked(st: &AppState, resolved: bool) -> Option<Picked> {
    let p = held(st, resolved)?;
    let paths: Vec<String> = order(st, resolved).into_iter().filter(|f| p.paths.contains(f)).collect();
    (paths.len() > 1).then(|| Picked { set: paths.iter().cloned().collect(), paths })
}

/// That list's files from `from` to `to`, both included (either may come first); just `to`
/// when `from` isn't listed.
pub fn range(st: &AppState, resolved: bool, from: &str, to: &str) -> Vec<String> {
    let all = order(st, resolved);
    let (Some(a), Some(b)) = (all.iter().position(|p| p == from), all.iter().position(|p| p == to)) else {
        return vec![to.to_owned()];
    };
    all[a.min(b)..=a.max(b)].to_vec()
}

/// Forgets the picks no longer in their list (after a status read: a file marked resolved
/// leaves Conflicted Files); no picks are left when none is.
pub fn prune(st: &mut AppState) {
    let Some(resolved) = st.cues.conflict_picks.as_ref().map(|p| p.resolved) else { return };
    let listed: BTreeSet<String> = order(st, resolved).into_iter().collect();
    let Some(p) = st.cues.conflict_picks.as_mut() else { return };
    p.paths.retain(|f| listed.contains(f));
    if p.paths.is_empty() {
        st.cues.conflict_picks = None;
    }
}
