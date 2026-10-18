//! Headless tests for re-running the open history when a refresh finds HEAD moved: the
//! request, keeping the selection and the search depth, and taking a renamed file's new name.

use super::history_tests::{empty_diff, entry, loaded, oid, opened, selected_target, with_view};
use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::{Effect, LoadKind};
use crate::msg::{HistoryMsg, Msg, RepoMsg};
use crate::state::AppState;
use crate::state::history::FileHistoryView;
use git2::Oid;
use std::time::Instant;
use trellis_core::repo::{self, DiffTarget, FileHistory, HistoryStart, LoadOptions};

/// A refresh whose snapshot has HEAD at `head` (the repository's own state otherwise).
fn refresh_with_head(st: &mut AppState, head: Option<Oid>) -> Vec<Effect> {
    let dir = st.workdir().unwrap();
    let mut snap = repo::load(&dir, &LoadOptions::all(100)).unwrap();
    snap.head_oid = head;
    let m = Msg::Repo(RepoMsg::RepoLoaded { generation: st.load_gen, kind: LoadKind::Refresh, result: Ok(Box::new(snap)) });
    update(st, m, Instant::now())
}

/// The history walks among `fx`.
fn walks(fx: &[Effect]) -> Vec<&Effect> {
    fx.iter().filter(|e| matches!(e, Effect::LoadFileHistory { .. })).collect()
}

fn history(entries: Vec<trellis_core::repo::HistoryEntry>) -> FileHistory {
    FileHistory { entries, from_head: true, ..Default::default() }
}

#[test]
fn head_move_reruns_the_open_history() {
    let (mut st, dir) = with_view("history-rerun");
    let old = st.snap.as_ref().and_then(|s| s.head_oid);
    let fx = refresh_with_head(&mut st, Some(oid(9)));
    let gen_now = st.history_gen;
    assert!(
        matches!(&walks(&fx)[..], [Effect::LoadFileHistory { start: HistoryStart::Moved(o), path, folder: false, generation, .. }]
            if Some(*o) == old && path == "a.txt" && *generation == gen_now),
        "{fx:?}"
    );
    let v = st.history.as_ref().unwrap();
    assert_eq!(v.entries.len(), 2, "the list stays until the result arrives");
    assert!(v.loading && v.rerun);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn same_head_refresh_leaves_history_alone() {
    let (mut st, dir) = with_view("history-same-head");
    let head = st.snap.as_ref().and_then(|s| s.head_oid);
    st.snap.as_mut().unwrap().refs_key ^= 1;
    let fx = refresh_with_head(&mut st, head);
    assert!(walks(&fx).is_empty(), "{fx:?}");
    assert!(!st.history.as_ref().unwrap().loading);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rerun_replaces_entries_and_keeps_the_selection() {
    let (mut st, dir) = with_view("history-rerun-keep");
    update(&mut st, Msg::History(HistoryMsg::SelectHistoryEntry(1)), Instant::now());
    st.diff = Some((DiffTarget::Commit(oid(2), "old.txt".into()), empty_diff().into()));
    refresh_with_head(&mut st, Some(oid(9)));
    st.view.history_scroll_to = None;
    let m = loaded(&st, history(vec![entry(9, "a.txt"), entry(1, "a.txt"), entry(2, "old.txt")]));
    let fx = update(&mut st, m, Instant::now());
    let v = st.history.as_ref().unwrap();
    let got: Vec<Oid> = v.entries.iter().map(|e| e.oid).collect();
    assert_eq!(got, [oid(9), oid(1), oid(2)]);
    assert_eq!(v.selected, Some(2));
    assert!(!v.loading && !v.rerun);
    assert_eq!(st.view.history_scroll_to, Some(2));
    assert!(!fx.iter().any(|e| matches!(e, Effect::LoadDiff { .. })), "the diff on screen stays: {fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rerun_without_the_selected_commit_selects_the_first() {
    let (mut st, dir) = with_view("history-rerun-first");
    update(&mut st, Msg::History(HistoryMsg::SelectHistoryEntry(1)), Instant::now());
    refresh_with_head(&mut st, Some(oid(9)));
    let m = loaded(&st, history(vec![entry(9, "a.txt"), entry(1, "a.txt")]));
    let fx = update(&mut st, m, Instant::now());
    assert_eq!(st.history.as_ref().unwrap().selected, Some(0));
    assert!(
        fx.iter().any(|e| matches!(e, Effect::LoadDiff { target: DiffTarget::Commit(o, p), .. } if *o == oid(9) && p == "a.txt")),
        "{fx:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rerun_to_an_empty_history_closes_the_diff() {
    let (mut st, dir) = with_view("history-rerun-empty");
    update(&mut st, Msg::History(HistoryMsg::SelectHistoryEntry(0)), Instant::now());
    st.diff = Some((DiffTarget::Commit(oid(1), "a.txt".into()), empty_diff().into()));
    refresh_with_head(&mut st, Some(oid(9)));
    let m = loaded(&st, history(Vec::new()));
    update(&mut st, m, Instant::now());
    let v = st.history.as_ref().unwrap();
    assert!(v.entries.is_empty() && v.selected.is_none() && !v.loading);
    assert!(st.diff.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rerun_keeps_the_search_depth() {
    let (mut st, dir) = with_view("history-rerun-depth");
    st.history.as_mut().unwrap().resume = vec![(oid(3), "old.txt".to_owned())];
    update(&mut st, Msg::History(HistoryMsg::MoreFileHistory), Instant::now());
    let m = loaded(&st, history(vec![entry(3, "old.txt")]));
    update(&mut st, m, Instant::now());
    assert_eq!(st.history.as_ref().unwrap().batches, 2);
    let fx = refresh_with_head(&mut st, Some(oid(9)));
    let want = 2 * st.settings.commit_limit();
    assert!(matches!(&walks(&fx)[..], [Effect::LoadFileHistory { limit, .. }] if *limit == want), "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn head_view_takes_the_new_name_from_a_rerun() {
    let (mut st, dir) = with_view("history-rerun-rename");
    refresh_with_head(&mut st, Some(oid(9)));
    let m = loaded(&st, FileHistory { head_path: Some("b.txt".into()), ..history(vec![entry(9, "b.txt"), entry(1, "a.txt")]) });
    update(&mut st, m, Instant::now());
    let v = st.history.as_ref().unwrap();
    assert_eq!(v.path, "b.txt");
    assert!(v.head_path.is_none() && v.as_of.is_none());
    let fx = refresh_with_head(&mut st, Some(oid(8)));
    assert!(matches!(&walks(&fx)[..], [Effect::LoadFileHistory { path, .. }] if path == "b.txt"), "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn at_view_records_the_head_name() {
    // The scratch repository has a.txt modified in the working tree.
    let (mut st, dir) = opened("history-at-head-name");
    let mut view = FileHistoryView::loading("old.txt".into(), Some(oid(5)));
    view.on_disk = false;
    st.history = Some(view);
    let m = Msg::History(HistoryMsg::FileHistoryLoaded {
        path: "old.txt".into(),
        generation: st.history_gen,
        result: Ok(FileHistory {
            head_path: Some("a.txt".into()),
            on_disk: false,
            ..history(vec![entry(1, "a.txt"), entry(5, "old.txt")])
        }),
    });
    update(&mut st, m, Instant::now());
    let v = st.history.as_ref().unwrap();
    assert_eq!(v.path, "old.txt");
    assert_eq!(v.head_path.as_deref(), Some("a.txt"));
    assert_eq!(v.today_path(), "a.txt");
    assert!(v.in_workdir(st.snap.as_ref()), "status lists a.txt");
    assert_eq!(v.selected, Some(0));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn search_further_keeps_on_disk_for_the_head_name() {
    let (mut st, dir) = opened("history-at-head-name-more");
    st.history = Some(FileHistoryView::loading("old.txt".into(), Some(oid(5))));
    let first = FileHistory {
        head_path: Some("a.txt".into()),
        on_disk: true,
        resume: vec![(oid(4), "old.txt".to_owned())],
        ..history(vec![entry(1, "a.txt"), entry(5, "old.txt")])
    };
    let m = Msg::History(HistoryMsg::FileHistoryLoaded { path: "old.txt".into(), generation: st.history_gen, result: Ok(first) });
    update(&mut st, m, Instant::now());
    update(&mut st, Msg::History(HistoryMsg::MoreFileHistory), Instant::now());
    // The resumed walk checked the disk for old.txt, which is gone since the rename.
    let more = FileHistory { on_disk: false, ..history(vec![entry(4, "old.txt")]) };
    let m = Msg::History(HistoryMsg::FileHistoryLoaded { path: "old.txt".into(), generation: st.history_gen, result: Ok(more) });
    update(&mut st, m, Instant::now());
    let v = st.history.as_ref().unwrap();
    assert_eq!(v.entries.len(), 3);
    assert_eq!(v.today_path(), "a.txt");
    assert!(v.on_disk && v.in_workdir(None), "a.txt is still on disk");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rerun_e2e_after_a_commit() {
    let dir = scratch_repo("history-rerun-e2e");
    git(&dir, &["commit", "-q", "-am", "edit a"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::History(HistoryMsg::OpenFileHistory { path: "a.txt".into(), at: None }));
    drive(&mut st, Msg::History(HistoryMsg::SelectHistoryEntry(1)));
    let was = st.history.as_ref().unwrap().selected_entry().unwrap().oid;
    std::fs::write(dir.join("a.txt"), "one\ntwo\nthree\n").unwrap();
    git(&dir, &["commit", "-q", "-am", "edit a again"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let v = st.history.as_ref().unwrap();
    let summaries: Vec<&str> = v.entries.iter().map(|e| e.summary.as_str()).collect();
    assert_eq!(summaries, ["edit a again", "edit a", "first"]);
    assert_eq!(v.selected_entry().map(|e| e.oid), Some(was), "the selected commit stays selected");
    assert!(!v.loading);
    assert_eq!(st.diff.as_ref().map(|(t, _)| t.clone()), Some(selected_target(&st)));
    let _ = std::fs::remove_dir_all(&dir);
}
