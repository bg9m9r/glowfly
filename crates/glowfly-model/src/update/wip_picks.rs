//! Picking files in the Unstaged and Staged Files lists with the pointer: Shift+click picks
//! every visible file from the selection's anchor to the clicked one (earlier gaps dropped),
//! Ctrl/Cmd+click adds or removes one file, gaps allowed. With nothing picked yet, the file open
//! in that list counts as picked. Adding a file opens it; removing the open one opens the
//! nearest pick left (after it first). The keys' Shift moves are in `wip_keys.rs`, the picks
//! themselves in `state/wip_lists.rs`.

use super::wip_list::{cursor, open, still_listed};
use crate::effect::Effect;
use crate::state::AppState;
use crate::state::wip_lists::{self, PickHow, WipPicks, files, order, path_in, target, visible};
use std::collections::BTreeSet;

/// A Shift (`Range`) or Ctrl/Cmd (`Toggle`) click on the row of `path` in that list.
pub(super) fn pick(st: &mut AppState, staged: bool, path: String, how: PickHow, fx: &mut Vec<Effect>) {
    let conflicted = st.snap.as_ref().is_some_and(|s| s.unstaged.iter().any(|f| f.path == path && f.status == '!'));
    if conflicted || !still_listed(st, &target(staged, &path)) {
        return;
    }
    // Picks belong to one list at a time.
    st.cues.conflict_picks = None;
    let open_here = cursor(st).and_then(|t| path_in(staged, t)).map(str::to_owned);
    let held = wip_lists::picks(st, staged).cloned();
    match how {
        PickHow::Range => {
            let anchor = held.map(|p| p.anchor).or(open_here).unwrap_or_else(|| path.clone());
            let paths = wip_lists::range(st, staged, &anchor, &path).into_iter().collect();
            open(st, target(staged, &path), fx);
            st.cues.wip_picks = Some(WipPicks { staged, anchor, paths });
        }
        PickHow::Toggle => {
            let mut paths: BTreeSet<String> = match held {
                Some(p) => p.paths,
                None => open_here.iter().cloned().collect(),
            };
            if paths.remove(&path) {
                if paths.is_empty() {
                    st.cues.wip_picks = None;
                    return;
                }
                if open_here.as_deref() == Some(path.as_str()) {
                    let Some(next) = nearest(st, staged, &path, &paths) else {
                        st.cues.wip_picks = None;
                        return;
                    };
                    open(st, target(staged, &next), fx);
                }
            } else {
                paths.insert(path.clone());
                open(st, target(staged, &path), fx);
            }
            st.cues.wip_picks = Some(WipPicks { staged, anchor: path, paths });
        }
    }
}

/// The visible file of that list nearest to `path` that is one of `paths`: after it first,
/// then before it.
fn nearest(st: &AppState, staged: bool, path: &str, paths: &BTreeSet<String>) -> Option<String> {
    let snap = st.snap.as_ref()?;
    let files = files(snap, staged);
    let shown: Vec<&str> = visible(&order(st, &files, staged)).into_iter().map(|i| files[i].path.as_str()).collect();
    let at = shown.iter().position(|p| *p == path)?;
    let picked = |p: &&&str| paths.contains(**p);
    shown[at + 1..].iter().find(picked).or_else(|| shown[..at].iter().rev().find(picked)).map(|p| (*p).to_owned())
}
