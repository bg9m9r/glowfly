//! The palette's worktree entries: open another worktree, create one from the selected commit
//! or HEAD, and remove, lock, unlock, move or prune them, offered by the same rules as the
//! WORKTREES row menu.

use super::{Entry, PaletteCommand, entry, with_detail};
use crate::state::worktree_manage::{lock_refused, prune_blocked, remove_refused, unlock_refused};
use crate::state::worktree_move::move_refused;
use crate::state::worktrees::open_blocked;
use crate::state::{AppState, Selection};
use crate::worktree::WorktreeMsg;
use trellis_core::native_path::shown;
use trellis_core::repo::Snapshot;
use trellis_core::worktree_ops::WorktreeBase;

fn command(m: WorktreeMsg) -> PaletteCommand {
    PaletteCommand::Worktree(Box::new(m))
}

/// What Create worktree starts from: the selected commit, else HEAD's branch, else HEAD's commit.
fn create_base(st: &AppState, snap: &Snapshot) -> Option<WorktreeBase> {
    let head = snap.head_oid?;
    Some(match (&st.selection, &snap.head_branch) {
        (Selection::Commit(oid), _) => WorktreeBase::Commit(*oid),
        (_, Some(b)) => WorktreeBase::Branch(b.clone()),
        (_, None) => WorktreeBase::Commit(head),
    })
}

pub(super) fn worktree_entries(st: &AppState, snap: &Snapshot, out: &mut Vec<Entry>) {
    for wt in snap.worktrees.iter().filter(|w| open_blocked(w).is_none()) {
        let what = match (&wt.branch, wt.head) {
            (Some(b), _) => format!(" ({b})"),
            (None, Some(oid)) => format!(" (detached {})", &oid.to_string()[..7]),
            (None, None) => String::new(),
        };
        let e = entry(command(WorktreeMsg::Open(wt.path.clone())), format!("Open worktree {}{what}", wt.folder_name()), OPEN_WORDS);
        out.push(with_detail(e, Some(shown(&wt.path))));
    }
    if let Some(base) = create_base(st, snap) {
        let label = format!("Create worktree from {}…", base.label());
        out.push(entry(command(WorktreeMsg::CreateDialog(base)), label, "worktree add new folder checkout"));
    }
    for wt in snap.worktrees.iter().filter(|w| !w.is_main() && !w.current) {
        let (name, path) = (wt.folder_name(), wt.path.clone());
        let mut add = |ok: bool, m: WorktreeMsg, label: String, words: &'static str| {
            if ok {
                out.push(with_detail(entry(command(m), label, words), Some(shown(&wt.path))));
            }
        };
        add(remove_refused(wt).is_none(), WorktreeMsg::RemoveDialog(path.clone()), format!("Remove worktree {name}…"), REMOVE_WORDS);
        add(lock_refused(wt).is_none(), WorktreeMsg::LockDialog(path.clone()), format!("Lock worktree {name}…"), LOCK_WORDS);
        add(unlock_refused(wt).is_none(), WorktreeMsg::Unlock(path.clone()), format!("Unlock worktree {name}"), LOCK_WORDS);
        add(move_refused(wt).is_none(), WorktreeMsg::MoveDialog(path), format!("Move worktree {name}…"), MOVE_WORDS);
    }
    if prune_blocked(snap).is_none() {
        let n = snap.worktrees.iter().filter(|w| w.prunable).count();
        let detail = if n == 1 { "1 record".to_owned() } else { format!("{n} records") };
        out.push(with_detail(entry(command(WorktreeMsg::Prune), "Prune worktrees", PRUNE_WORDS), Some(detail)));
    }
}

const OPEN_WORDS: &str = "worktree open tab switch folder";
const REMOVE_WORDS: &str = "worktree delete remove folder";
const LOCK_WORDS: &str = "worktree lock unlock protect removable drive";
const MOVE_WORDS: &str = "worktree move rename relocate folder";
const PRUNE_WORDS: &str = "worktree prune missing records clean up";
