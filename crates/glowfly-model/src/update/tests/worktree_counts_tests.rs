//! Headless tests for the background count of other worktrees right after a full load
//! (update/worktree_counts.rs): Pending and Carried worktrees are asked for in one effect, its
//! result fills in the carried ones and keeps those left out (as Counted), a result for the old
//! snapshot is dropped and followed by one for the new, and a Carried worktree reads as Counted.
//! The flows in glowfly-flows run the count end to end.

use super::tests::{scratch_repo, state};
use super::update;
use crate::effect::{Effect, LoadKind, WorktreeEffect};
use crate::msg::{Msg, RepoMsg};
use crate::state::worktrees::{right_text, tooltip};
use crate::state::{AppState, Tab};
use crate::worktree::{CountRequest, WorktreeMsg};
use glowfly_core::repo::{self, ChangeCounts, ChangesRead, Snapshot, WorktreeCount, WorktreeInfo};
use std::path::{Path, PathBuf};
use std::time::Instant;

const CARRIED: ChangeCounts = ChangeCounts { modified: 1, added: 0, deleted: 0 };
const FRESH: ChangeCounts = ChangeCounts { modified: 2, added: 1, deleted: 0 };

/// `scratch_repo(name)` as the only tab's folder, with no snapshot yet.
fn opened(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    let mut st = state();
    st.tabs[0] = Tab::unloaded(dir.clone());
    (st, dir)
}

/// The path of the side worktree `name`.
fn side_path(dir: &Path, name: &str) -> PathBuf {
    dir.with_file_name(format!("wtcount-model-{name}"))
}

/// A fresh load of `dir` with a side worktree for each of `reads`, after the current one.
fn snapshot(st: &AppState, dir: &Path, reads: &[(&str, ChangesRead, ChangeCounts)]) -> Snapshot {
    let mut snap = repo::load(dir, &st.load_options(dir)).unwrap();
    for (name, read, changes) in reads {
        let mut side: WorktreeInfo = snap.worktrees[0].clone();
        side.current = false;
        (side.path, side.read, side.changes) = (side_path(dir, name), *read, *changes);
        snap.worktrees.push(side);
    }
    snap
}

/// A full load of `snap` lands in the active tab.
fn loaded(st: &mut AppState, snap: Snapshot) -> Vec<Effect> {
    let msg = Msg::Repo(RepoMsg::RepoLoaded { generation: st.load_gen, kind: LoadKind::Refresh, result: Ok(Box::new(snap)) });
    update(st, msg, Instant::now())
}

fn requests(fx: &[Effect]) -> Vec<&CountRequest> {
    fx.iter().filter_map(|e| if let Effect::Worktree(WorktreeEffect::CountChanges(r)) = e { Some(r) } else { None }).collect()
}

fn result(path: PathBuf, changes: ChangeCounts) -> WorktreeCount {
    WorktreeCount { path, read: ChangesRead::Counted, changes, slow: None }
}

fn counted(st: &mut AppState, refs_key: u64, counts: Vec<WorktreeCount>) -> Vec<Effect> {
    update(st, Msg::Worktree(WorktreeMsg::ChangesCounted { refs_key, counts }), Instant::now())
}

fn read_of(st: &AppState, dir: &Path, name: &str) -> (ChangesRead, ChangeCounts) {
    let path = side_path(dir, name);
    let wt = st.snap.as_ref().unwrap().worktrees.iter().find(|w| w.path == path).unwrap();
    (wt.read, wt.changes)
}

#[test]
fn a_full_load_asks_one_count_for_pending_and_carried_worktrees() {
    let (mut st, dir) = opened("wtcount-model-ask");
    let snap =
        snapshot(&st, &dir, &[("pending", ChangesRead::Pending, ChangeCounts::default()), ("carried", ChangesRead::Carried, CARRIED)]);
    let key = snap.refs_key;
    let fx = loaded(&mut st, snap);
    let asked = requests(&fx);
    assert_eq!(asked.len(), 1, "{fx:?}");
    let paths: Vec<_> = asked[0].targets.iter().map(|t| t.0.clone()).collect();
    assert_eq!(paths, [side_path(&dir, "pending"), side_path(&dir, "carried")]);
    assert_eq!(st.refresh.counting, Some(key));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_count_fills_carried_counts_and_keeps_ones_left_out() {
    let (mut st, dir) = opened("wtcount-model-fill");
    let reads = [
        ("filled", ChangesRead::Carried, CARRIED),
        ("left-out", ChangesRead::Carried, CARRIED),
        ("pending", ChangesRead::Pending, ChangeCounts::default()),
    ];
    let snap = snapshot(&st, &dir, &reads);
    let key = snap.refs_key;
    loaded(&mut st, snap);

    let fx = counted(&mut st, key, vec![result(side_path(&dir, "filled"), FRESH)]);
    assert!(requests(&fx).is_empty(), "nothing is asked for again: {fx:?}");
    assert_eq!(read_of(&st, &dir, "filled"), (ChangesRead::Counted, FRESH));
    assert_eq!(read_of(&st, &dir, "left-out"), (ChangesRead::Counted, CARRIED), "its carried counts stand");
    assert_eq!(read_of(&st, &dir, "pending").0, ChangesRead::TooLarge, "left out while Pending");
    assert_eq!(st.refresh.counting, None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_count_for_the_old_snapshot_is_followed_by_one_for_the_carried() {
    let (mut st, dir) = opened("wtcount-model-stale");
    let snap = snapshot(&st, &dir, &[("carried", ChangesRead::Carried, CARRIED)]);
    let new_key = snap.refs_key;
    let old_key = new_key.wrapping_add(1);
    st.refresh.counting = Some(old_key);

    let fx = loaded(&mut st, snap);
    assert!(requests(&fx).is_empty(), "the old count still runs: {fx:?}");
    assert_eq!(st.refresh.counting, Some(old_key));

    let late = vec![result(side_path(&dir, "carried"), FRESH)];
    let fx = counted(&mut st, old_key, late);
    assert_eq!(read_of(&st, &dir, "carried"), (ChangesRead::Carried, CARRIED), "the old result applies nothing");
    let asked = requests(&fx);
    assert_eq!(asked.len(), 1, "{fx:?}");
    assert_eq!(asked[0].refs_key, new_key);
    assert!(asked[0].targets.iter().any(|t| t.0 == side_path(&dir, "carried")));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_carried_worktree_reads_as_counted() {
    let (st, dir) = opened("wtcount-model-text");
    let snap = snapshot(&st, &dir, &[("carried", ChangesRead::Carried, CARRIED)]);
    let carried = &snap.worktrees[1];
    let counted = &WorktreeInfo { read: ChangesRead::Counted, ..carried.clone() };
    for text in [right_text(carried), tooltip(carried)] {
        assert!(!text.to_lowercase().contains("counting"), "{text}");
    }
    assert_eq!(right_text(carried), right_text(counted));
    assert_eq!(tooltip(carried), tooltip(counted));
    let _ = std::fs::remove_dir_all(&dir);
}
