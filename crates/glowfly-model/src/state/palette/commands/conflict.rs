//! The palette's conflict entries, while files are conflicted or resolved during an operation:
//! Mark all resolved, Mark <path> conflicted for each resolved file git can restore, and Mark
//! all conflicted when there are several.

use super::{Entry, PaletteCommand as C, entry, with_detail};
use crate::state::AppState;
use crate::state::conflict_files::{conflicted_paths, restorable_paths};
use glowfly_core::repo::Snapshot;

fn files(n: usize) -> String {
    format!("{n} file{}", if n == 1 { "" } else { "s" })
}

pub(super) fn conflict_entries(st: &AppState, snap: &Snapshot, out: &mut Vec<Entry>) {
    let conflicted = conflicted_paths(snap);
    if !conflicted.is_empty() {
        let e = entry(C::MarkAllResolved, "Mark all resolved", "conflict conflicted resolve stage add done merge rebase");
        out.push(with_detail(e, Some(files(conflicted.len()))));
    }
    let restorable = restorable_paths(st);
    if restorable.len() >= 2 {
        let e = entry(C::MarkAllConflicted, "Mark all conflicted", "conflict restore undo resolution markers again");
        out.push(with_detail(e, Some(files(restorable.len()))));
    }
    for p in restorable {
        out.push(entry(C::MarkConflicted(p.clone()), format!("Mark {p} conflicted"), "conflict restore undo resolution markers again"));
    }
}
