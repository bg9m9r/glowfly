//! Headless tests for other worktrees counted after the load: a worktree the load left Pending
//! shows as being counted, one background count fills it in and lays the graph out again, only
//! one count runs at a time (also through unchanged refreshes), a count for other refs is
//! dropped, a too-large worktree isn't retried, the option off counts nothing, a slow count
//! offers the untracked cache, and git reads a worktree only by that worktree's own trust.

use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::{Effect, LoadKind};
use crate::msg::{Msg, RepoMsg};
use crate::runner;
use crate::state::AppState;
use crate::state::worktrees::{right_text, tooltip};
use crate::worktree::WorktreeMsg;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use trellis_core::repo::{ChangeCounts, ChangesRead, SlowStatus, StatusSpeedups, WorktreeCount};

/// scratch_repo with a linked worktree on `side` beside it (a.txt modified, b.txt added), opened
/// on the main worktree.
fn opened(name: &str) -> (AppState, PathBuf, PathBuf) {
    let dir = scratch_repo(name);
    let linked = dir.with_file_name(format!("{}-side", dir.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&linked);
    git(&dir, &["worktree", "add", "-q", "-b", "side", &linked.display().to_string()]);
    std::fs::write(linked.join("a.txt"), "changed there\n").unwrap();
    std::fs::write(linked.join("b.txt"), "new there\n").unwrap();
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    (st, dir, linked)
}

const SIDE: ChangeCounts = ChangeCounts { modified: 1, added: 1, deleted: 0 };

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

/// Delivers a fresh load of the active tab in which the linked worktree was left Pending, as a
/// load would leave a large one. Returns the effects.
fn load_pending(st: &mut AppState) -> Vec<Effect> {
    let workdir = st.workdir().unwrap();
    let mut snap = trellis_core::repo::load(&workdir, &st.load_options(&workdir)).unwrap();
    snap.worktrees[1].read = ChangesRead::Pending;
    snap.worktrees[1].changes = ChangeCounts::default();
    super::refresh::lay_out_graph(&mut snap);
    let msg = Msg::Repo(RepoMsg::RepoLoaded { generation: st.load_gen, kind: LoadKind::Refresh, result: Ok(Box::new(snap)) });
    update(st, msg, Instant::now())
}

fn counts(fx: &[Effect]) -> Vec<&crate::worktree::CountRequest> {
    fx.iter().filter_map(|e| if let Effect::CountWorktreeChanges(r) = e { Some(r) } else { None }).collect()
}

/// Runs `msg` and the loads it leads to, collecting every other effect on the way.
fn step(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    let mut all = Vec::new();
    let mut queue = vec![msg];
    while let Some(m) = queue.pop() {
        for e in update(st, m, Instant::now()) {
            if matches!(e, Effect::LoadRepo { .. } | Effect::LoadStatus { .. } | Effect::LoadDiff { .. }) {
                queue.extend(runner::execute(e));
            } else {
                all.push(e);
            }
        }
    }
    all
}

fn linked_info(st: &AppState) -> &trellis_core::repo::WorktreeInfo {
    &st.snap.as_ref().unwrap().worktrees[1]
}

fn refs_key(st: &AppState) -> u64 {
    st.snap.as_ref().unwrap().refs_key
}

fn result(path: &Path, read: ChangesRead, changes: ChangeCounts, slow: Option<SlowStatus>) -> WorktreeCount {
    WorktreeCount { path: path.to_path_buf(), read, changes, slow }
}

#[test]
fn a_large_worktree_is_counted_after_the_graph() {
    let (mut st, dir, linked) = opened("wtcount-after");
    assert_eq!(linked_info(&st).read, ChangesRead::Counted, "a small worktree is counted by the load");
    let fx = load_pending(&mut st);
    let snap = st.snap.as_ref().unwrap();
    assert!(!snap.commits.is_empty());
    assert!(snap.graph.worktree_wips.is_empty(), "no row yet");
    assert!(right_text(linked_info(&st)).ends_with(" · counting…"), "{}", right_text(linked_info(&st)));
    assert!(tooltip(linked_info(&st)).contains("Counting its uncommitted changes…"));
    let asked = counts(&fx);
    assert_eq!(asked.len(), 1, "{fx:?}");
    assert_eq!(asked[0].targets.len(), 1);
    assert!(trellis_core::paths::same_folder(&asked[0].targets[0].0, &linked));
    assert_eq!(st.refresh.counting, Some(refs_key(&st)));

    let count = fx.into_iter().find(|e| matches!(e, Effect::CountWorktreeChanges(_))).unwrap();
    let done = runner::execute(count).expect("a result");
    let fx = update(&mut st, done, Instant::now());
    assert!(counts(&fx).is_empty(), "nothing left to count");
    let wt = linked_info(&st);
    assert_eq!((wt.read, wt.changes), (ChangesRead::Counted, SIDE));
    assert!(!right_text(wt).contains("counting"));
    let rows: Vec<usize> = st.snap.as_ref().unwrap().graph.worktree_wips.iter().map(|w| w.worktree).collect();
    assert_eq!(rows, [1], "its WIP row");
    assert_eq!(st.refresh.counting, None);
    cleanup(&[&linked, &dir]);
}

#[test]
fn one_count_at_a_time_through_unchanged_refreshes() {
    let (mut st, dir, linked) = opened("wtcount-one");
    assert_eq!(counts(&load_pending(&mut st)).len(), 1);
    let key = refs_key(&st);
    let fx = step(&mut st, Msg::Repo(RepoMsg::WindowFocused));
    assert!(counts(&fx).is_empty(), "the running count isn't asked for again: {fx:?}");
    assert_eq!(st.refresh.counting, Some(key), "it still runs");
    assert_eq!(linked_info(&st).read, ChangesRead::Pending, "the refresh found the repository unchanged");
    cleanup(&[&linked, &dir]);
}

#[test]
fn a_count_for_other_refs_is_dropped() {
    let (mut st, dir, linked) = opened("wtcount-other-refs");
    load_pending(&mut st);
    let old = refs_key(&st);
    // A newer snapshot (other refs) arrived while the count ran; its worktree is Pending too.
    st.snap.as_mut().unwrap().refs_key = old.wrapping_add(1);
    let stray = Msg::Worktree(WorktreeMsg::ChangesCounted { refs_key: old.wrapping_add(7), counts: vec![] });
    assert!(counts(&update(&mut st, stray, Instant::now())).is_empty());
    assert_eq!(st.refresh.counting, Some(old), "a result it doesn't own clears nothing");

    let late = vec![result(&linked_info(&st).path.clone(), ChangesRead::Counted, SIDE, None)];
    let fx = update(&mut st, Msg::Worktree(WorktreeMsg::ChangesCounted { refs_key: old, counts: late }), Instant::now());
    assert_eq!(linked_info(&st).read, ChangesRead::Pending, "applies nothing");
    let asked = counts(&fx);
    assert_eq!(asked.len(), 1, "asks again for the newer snapshot: {fx:?}");
    assert_eq!(asked[0].refs_key, old.wrapping_add(1));
    assert_eq!(st.refresh.counting, Some(old.wrapping_add(1)));
    cleanup(&[&linked, &dir]);
}

#[test]
fn too_large_is_shown_and_not_retried() {
    let (mut st, dir, linked) = opened("wtcount-too-large");
    load_pending(&mut st);
    let key = refs_key(&st);
    let path = linked_info(&st).path.clone();
    let msg = Msg::Worktree(WorktreeMsg::ChangesCounted {
        refs_key: key,
        counts: vec![result(&path, ChangesRead::TooLarge, ChangeCounts::default(), None)],
    });
    assert!(counts(&update(&mut st, msg, Instant::now())).is_empty());
    let wt = linked_info(&st);
    assert_eq!(wt.read, ChangesRead::TooLarge);
    assert!(right_text(wt).ends_with(" · not counted"), "{}", right_text(wt));
    assert!(tooltip(wt).contains("Too large to count its uncommitted changes in the background. Open it in a tab to see them."));
    assert!(st.snap.as_ref().unwrap().graph.worktree_wips.is_empty());

    let fx = step(&mut st, Msg::Repo(RepoMsg::WindowFocused));
    assert!(counts(&fx).is_empty(), "not retried: {fx:?}");
    assert_eq!(linked_info(&st).read, ChangesRead::TooLarge, "the refresh found the repository unchanged");
    assert_eq!(st.refresh.counting, None);
    cleanup(&[&linked, &dir]);
}

#[test]
fn nothing_is_counted_with_the_option_off() {
    let (mut st, dir, linked) = opened("wtcount-off");
    st.settings.worktree_wips = false;
    assert!(counts(&load_pending(&mut st)).is_empty());
    assert_eq!(st.refresh.counting, None);
    cleanup(&[&linked, &dir]);
}

#[test]
fn a_slow_background_count_offers_the_untracked_cache() {
    let (mut st, dir, linked) = opened("wtcount-slow");
    load_pending(&mut st);
    let (key, path) = (refs_key(&st), linked_info(&st).path.clone());
    let slow = SlowStatus { took: Duration::from_secs(2), speedups: StatusSpeedups::default() };
    let toasts = st.toasts.len();
    let msg =
        Msg::Worktree(WorktreeMsg::ChangesCounted { refs_key: key, counts: vec![result(&path, ChangesRead::Counted, SIDE, Some(slow))] });
    update(&mut st, msg, Instant::now());
    assert_eq!(st.toasts.len(), toasts + 1);
    let toast = st.toasts.last().unwrap();
    assert_eq!(toast.action.as_ref().map(|a| a.label.as_str()), Some("Turn on"), "{}", toast.text);
    let real = st.snap.as_ref().unwrap().real_workdir.clone();
    assert!(st.settings.status_tip_shown_for(&real), "once per repository");
    cleanup(&[&linked, &dir]);
}

/// A shell script that touches `marker` when run, as a fsmonitor hook git would start.
fn marker_hook(script: &Path, marker: &Path) -> String {
    let slashes = |p: &Path| p.display().to_string().replace('\\', "/");
    std::fs::write(script, format!("#!/bin/sh\ntouch \"{}\"\nexit 1\n", slashes(marker))).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(script, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    slashes(script)
}

#[test]
fn another_worktree_is_counted_by_its_own_trust_not_the_tab_s() {
    use crate::state::trust::record;
    use trellis_core::settings::RepoTrust;
    let (mut st, dir, linked) = opened("wtcount-own-trust");
    let real = st.snap.as_ref().unwrap().real_workdir.clone();
    record(&mut st.settings, &real, RepoTrust::Trusted);
    // Only the linked worktree's own config names a program for git to run.
    let marker = dir.join("ran.txt");
    let hook = marker_hook(&dir.join(".git").join("fsmon.sh"), &marker);
    git(&linked, &["config", "extensions.worktreeConfig", "true"]);
    git(&linked, &["config", "--worktree", "core.fsmonitor", &hook]);
    let path = linked_info(&st).path.clone();
    let count_with = |st: &mut AppState, decision: Option<RepoTrust>| {
        trellis_core::settings::repo_keys::set(&mut st.settings.repo_trust, &path, decision);
        st.refresh.counting = None;
        let fx = load_pending(st);
        let asked = counts(&fx);
        assert_eq!(asked.len(), 1, "{fx:?}");
        assert_eq!(asked[0].targets[0].1, decision, "the worktree's own decision, not the tab's");
        let count = fx.into_iter().find(|e| matches!(e, Effect::CountWorktreeChanges(_))).unwrap();
        let done = runner::execute(count).expect("a result");
        update(st, done, Instant::now());
        let wt = linked_info(st);
        assert_eq!((wt.read, wt.changes), (ChangesRead::Counted, SIDE));
    };

    count_with(&mut st, Some(RepoTrust::NotTrusted));
    assert!(!marker.exists(), "the tab is trusted, but the worktree isn't: its hook never runs");
    count_with(&mut st, None);
    assert!(!marker.exists(), "never reviewed, and its config runs a program: git doesn't read it");
    count_with(&mut st, Some(RepoTrust::Trusted));
    assert!(marker.exists(), "trusted itself: git reads it");
    cleanup(&[&linked, &dir]);
}
