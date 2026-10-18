//! Headless tests for the palette's worktree entries: Open worktree for the others (not this
//! tab's, not a missing one), Create worktree from the selected commit or HEAD's branch,
//! Remove / Lock / Unlock / Move by the row menu's rules, Prune only with a prunable record, and
//! what running each sends.

use super::tests::{drive, git, scratch_repo, state};
use crate::msg::{Msg, RepoMsg};
use crate::state::palette::{PaletteCommand, entries};
use crate::state::{AppState, Selection};
use crate::worktree::WorktreeMsg;
use std::path::{Path, PathBuf};
use trellis_core::worktree_ops::WorktreeBase;

fn labels(st: &AppState) -> Vec<String> {
    entries(st).into_iter().map(|e| e.label).collect()
}

fn command(st: &AppState, label: &str) -> Option<PaletteCommand> {
    entries(st).into_iter().find(|e| e.label == label).map(|e| e.command)
}

/// A repository opened in `st` with linked worktrees `side` and `old` (locked).
fn opened(name: &str) -> (AppState, PathBuf, PathBuf, PathBuf) {
    let dir = scratch_repo(name);
    let root = dir.with_file_name(format!("{}-worktrees", dir.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&root);
    let (side, old) = (root.join("side"), root.join("old"));
    git(&dir, &["worktree", "add", "-q", "-b", "side", &side.display().to_string()]);
    git(&dir, &["worktree", "add", "-q", "--detach", &old.display().to_string()]);
    git(&dir, &["worktree", "lock", &old.display().to_string()]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    (st, dir, side, old)
}

fn cleanup(dir: &Path, side: &Path, old: &Path) {
    git(dir, &["worktree", "unlock", &old.display().to_string()]);
    let _ = std::fs::remove_dir_all(side.parent().unwrap());
    let _ = std::fs::remove_dir_all(dir);
}

fn is(c: Option<PaletteCommand>, want: WorktreeMsg) -> bool {
    c == Some(PaletteCommand::Worktree(Box::new(want)))
}

#[test]
fn worktree_entries() {
    let (mut st, dir, side, old) = opened("palwt-entries");
    let head = st.snap.as_ref().unwrap().head_oid.unwrap();
    let sha = &head.to_string()[..7];
    let all = labels(&st);
    assert!(is(command(&st, "Open worktree side (side)"), WorktreeMsg::Open(side.clone())), "{all:?}");
    assert!(all.iter().any(|l| *l == format!("Open worktree old (detached {sha})")), "{all:?}");
    assert!(!all.iter().any(|l| l.starts_with(&format!("Open worktree {}", dir.file_name().unwrap().to_string_lossy()))), "not this tab's");
    let detail = entries(&st).into_iter().find(|e| e.label == "Open worktree side (side)").and_then(|e| e.detail);
    assert_eq!(detail, Some(trellis_core::native_path::shown(&side)));

    assert!(is(command(&st, "Create worktree from main…"), WorktreeMsg::CreateDialog(WorktreeBase::Branch("main".into()))));
    st.selection = Selection::Commit(head);
    assert!(is(command(&st, &format!("Create worktree from {sha}…")), WorktreeMsg::CreateDialog(WorktreeBase::Commit(head))));
    assert!(command(&st, "Create worktree from main…").is_none(), "the selected commit wins");

    assert!(is(command(&st, "Remove worktree side…"), WorktreeMsg::RemoveDialog(side.clone())));
    assert!(is(command(&st, "Lock worktree side…"), WorktreeMsg::LockDialog(side.clone())));
    assert!(is(command(&st, "Move worktree side…"), WorktreeMsg::MoveDialog(side.clone())));
    assert!(command(&st, "Unlock worktree side").is_none());
    assert!(is(command(&st, "Unlock worktree old"), WorktreeMsg::Unlock(old.clone())));
    assert!(command(&st, "Lock worktree old…").is_none() && command(&st, "Move worktree old…").is_none(), "locked");
    assert!(command(&st, "Remove worktree old…").is_some(), "a locked one can be force-removed");
    assert!(command(&st, "Prune worktrees").is_none(), "nothing to prune");

    std::fs::remove_dir_all(&side).unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(command(&st, "Open worktree side (side)").is_none(), "missing");
    let prune = entries(&st).into_iter().find(|e| e.label == "Prune worktrees").expect("a prunable record");
    assert_eq!((prune.command, prune.detail.as_deref()), (PaletteCommand::Worktree(Box::new(WorktreeMsg::Prune)), Some("1 record")));

    // Running one sends its message: Prune drops the record.
    drive(&mut st, Msg::Palette(crate::msg::PaletteMsg::RunPaletteCommand(PaletteCommand::Worktree(Box::new(WorktreeMsg::Prune)))));
    assert!(!st.snap.as_ref().unwrap().worktrees.iter().any(|w| w.missing), "{:?}", st.log.last().map(|l| &l.text));
    cleanup(&dir, &side, &old);
}
