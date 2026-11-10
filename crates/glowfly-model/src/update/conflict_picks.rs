//! Picking files in Conflicted Files and Resolved Files with the pointer: Shift+click picks
//! every file from the anchor to the clicked one (earlier gaps dropped), Ctrl/Cmd+click adds or
//! removes one file, gaps allowed. With nothing picked yet, the file the list has open counts as
//! picked. A pick never opens anything: the conflict view may hold choices not yet saved, and
//! the user is often mid-edit in one. The picks themselves are in `state/conflict_picks.rs`.

use crate::state::AppState;
use crate::state::conflict_picks::{self, ConflictPicks};
use crate::state::wip_lists::PickHow;
use std::collections::BTreeSet;

/// A Shift (`Range`) or Ctrl/Cmd (`Toggle`) click on the row of `path` in that list.
pub(super) fn pick(st: &mut AppState, resolved: bool, path: String, how: PickHow) {
    if !conflict_picks::order(st, resolved).contains(&path) {
        return;
    }
    // Picks belong to one list at a time.
    st.cues.wip_picks = None;
    let held = conflict_picks::held(st, resolved).cloned();
    let current = conflict_picks::current(st, resolved);
    match how {
        PickHow::Range => {
            let anchor = held.map(|p| p.anchor).or(current).unwrap_or_else(|| path.clone());
            let paths = conflict_picks::range(st, resolved, &anchor, &path).into_iter().collect();
            st.cues.conflict_picks = Some(ConflictPicks { resolved, anchor, paths });
        }
        PickHow::Toggle => {
            let mut paths: BTreeSet<String> = match held {
                Some(p) => p.paths,
                None => current.into_iter().collect(),
            };
            if !paths.remove(&path) {
                paths.insert(path.clone());
            }
            st.cues.conflict_picks = (!paths.is_empty()).then_some(ConflictPicks { resolved, anchor: path, paths });
        }
    }
}
