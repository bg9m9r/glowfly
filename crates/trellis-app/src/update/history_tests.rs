//! Headless tests for the file history view: requests, results, selection, searching further,
//! jumping to the graph, and closing.

use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::{Effect, LoadKind};
use crate::msg::ConflictMsg;
use crate::msg::HistoryMsg;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::msg::TabsMsg;
use crate::state::conflict::ConflictView;
use crate::state::history::FileHistoryView;
use crate::state::{AppState, Selection};
use git2::Oid;
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::repo::{self, DiffTarget, FileDiff, FileHistory, HistoryEntry, HistoryStart, LoadOptions};

pub(super) fn oid(n: u8) -> Oid {
    Oid::from_bytes(&[n; 20]).unwrap()
}

pub(super) fn entry(n: u8, path: &str) -> HistoryEntry {
    HistoryEntry {
        oid: oid(n),
        summary: format!("commit {n}"),
        author: "Test".into(),
        email: "test@example.com".into(),
        time: 1_700_000_000 - n as i64,
        path: path.into(),
        old_path: None,
        status: 'M',
        files: Vec::new(),
    }
}

pub(super) fn empty_diff() -> FileDiff {
    FileDiff { binary: false, hunks: Vec::new(), highlight: None, intraline: Default::default(), whole: None }
}

/// A scratch repository opened in the app.
pub(super) fn opened(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    (st, dir)
}

/// An open repository with a loaded history of a.txt: entries 1 (at a.txt) and 2 (at old.txt).
pub(super) fn with_view(name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = opened(name);
    let mut view = FileHistoryView::loading("a.txt".into(), None);
    view.append(FileHistory { entries: vec![entry(1, "a.txt"), entry(2, "old.txt")], from_head: true, ..Default::default() });
    view.batches = 1;
    st.history = Some(view);
    (st, dir)
}

pub(super) fn loaded(st: &AppState, h: FileHistory) -> Msg {
    Msg::History(HistoryMsg::FileHistoryLoaded { path: "a.txt".into(), generation: st.history_gen, result: Ok(h) })
}

#[test]
fn opening_file_history_requests_a_walk() {
    let (mut st, dir) = opened("history-open");
    st.diff = Some((DiffTarget::Unstaged("a.txt".into()), empty_diff().into()));
    st.conflict = Some(ConflictView::loading("a.txt".into()));
    let fx = update(&mut st, Msg::History(HistoryMsg::OpenFileHistory { path: "a.txt".into(), at: None }), Instant::now());
    let limit = st.settings.commit_limit();
    assert!(
        matches!(&fx[..], [Effect::LoadFileHistory { path, start: HistoryStart::Head, limit: l, generation: 1, .. }]
            if path == "a.txt" && *l == limit),
        "{fx:?}"
    );
    let view = st.history.as_ref().expect("view open");
    assert!(view.loading && view.entries.is_empty());
    assert!(st.diff.is_none() && st.conflict.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn file_history_end_to_end_with_rename() {
    let dir = scratch_repo("history-e2e");
    git(&dir, &["commit", "-q", "-am", "edit a"]);
    git(&dir, &["mv", "a.txt", "b.txt"]);
    git(&dir, &["commit", "-q", "-m", "rename a to b"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let head = st.snap.as_ref().and_then(|s| s.head_oid).unwrap();
    drive(&mut st, Msg::History(HistoryMsg::OpenFileHistory { path: "b.txt".into(), at: Some(head) }));
    let view = st.history.as_ref().expect("view open");
    assert!(!view.loading && view.complete() && view.as_of.is_none());
    let statuses: String = view.entries.iter().map(|e| e.status).collect();
    assert_eq!(statuses, "RMA");
    assert_eq!(view.entries[0].old_path.as_deref(), Some("a.txt"));
    assert_eq!(view.entries[2].path, "a.txt");
    assert_eq!(view.selected, Some(0));
    let (target, diff) = st.diff.as_ref().expect("diff open");
    assert_eq!(*target, DiffTarget::Commit(view.entries[0].oid, view.entries[0].path.clone()));
    assert!(!diff.binary && diff.hunks.is_empty(), "a pure rename has no textual changes");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stale_history_results_are_ignored() {
    let (mut st, dir) = opened("history-stale");
    update(&mut st, Msg::History(HistoryMsg::OpenFileHistory { path: "a.txt".into(), at: None }), Instant::now());
    let fresh = FileHistory { entries: vec![entry(1, "a.txt")], from_head: true, ..Default::default() };
    // An older generation.
    let old =
        Msg::History(HistoryMsg::FileHistoryLoaded { path: "a.txt".into(), generation: st.history_gen - 1, result: Ok(fresh.clone()) });
    assert!(update(&mut st, old, Instant::now()).is_empty());
    // Another path.
    let other = Msg::History(HistoryMsg::FileHistoryLoaded { path: "b.txt".into(), generation: st.history_gen, result: Ok(fresh.clone()) });
    assert!(update(&mut st, other, Instant::now()).is_empty());
    let view = st.history.as_ref().unwrap();
    assert!(view.loading && view.entries.is_empty());
    // After closing.
    let generation = st.history_gen;
    update(&mut st, Msg::History(HistoryMsg::CloseFileHistory), Instant::now());
    let late = Msg::History(HistoryMsg::FileHistoryLoaded { path: "a.txt".into(), generation, result: Ok(fresh) });
    assert!(update(&mut st, late, Instant::now()).is_empty());
    assert!(st.history.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn selecting_an_entry_opens_the_diff_at_its_path() {
    let (mut st, dir) = with_view("history-select");
    let fx = update(&mut st, Msg::History(HistoryMsg::SelectHistoryEntry(1)), Instant::now());
    assert_eq!(st.history.as_ref().unwrap().selected, Some(1));
    assert_eq!(st.view.history_scroll_to, Some(1));
    assert!(
        matches!(&fx[..], [Effect::LoadDiff { target: DiffTarget::Commit(o, p), reload: false, .. }] if *o == oid(2) && p == "old.txt"),
        "{fx:?}"
    );
    st.view.history_scroll_to = None;
    assert!(update(&mut st, Msg::History(HistoryMsg::SelectHistoryEntry(5)), Instant::now()).is_empty());
    assert_eq!(st.history.as_ref().unwrap().selected, Some(1));
    assert!(st.view.history_scroll_to.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn searching_further_appends_without_duplicates() {
    let (mut st, dir) = with_view("history-more");
    let resume = vec![(oid(3), "old.txt".to_owned())];
    {
        let v = st.history.as_mut().unwrap();
        v.resume = resume.clone();
        v.searched_until = Some((oid(2), 5));
    }
    let fx = update(&mut st, Msg::History(HistoryMsg::MoreFileHistory), Instant::now());
    assert!(st.history.as_ref().unwrap().loading);
    assert!(matches!(&fx[..], [Effect::LoadFileHistory { start: HistoryStart::Resume(r), .. }] if *r == resume), "{fx:?}");
    assert!(update(&mut st, Msg::History(HistoryMsg::MoreFileHistory), Instant::now()).is_empty(), "one request at a time");

    let batch = FileHistory {
        entries: vec![entry(2, "old.txt"), entry(3, "old.txt")],
        from_head: true,
        searched_until: Some((oid(4), 9)),
        resume: vec![(oid(5), "older.txt".into())],
        on_disk: true,
        head_path: None,
        absent: Default::default(),
    };
    let m = loaded(&st, batch);
    let fx = update(&mut st, m, Instant::now());
    assert!(fx.is_empty(), "the selection stays where it was");
    let v = st.history.as_ref().unwrap();
    let got: Vec<Oid> = v.entries.iter().map(|e| e.oid).collect();
    assert_eq!(got, [oid(1), oid(2), oid(3)]);
    assert_eq!(v.resume, [(oid(5), "older.txt".to_owned())]);
    assert_eq!(v.searched_until, Some((oid(4), 9)));
    assert!(!v.loading && !v.complete());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn view_on_graph_selects_visible_commits_only() {
    let (mut st, dir) = with_view("history-graph");
    let head = st.snap.as_ref().and_then(|s| s.head_oid).unwrap();
    st.view.scroll_to_row = None;
    let fx = update(&mut st, Msg::History(HistoryMsg::ShowInGraph(head)), Instant::now());
    assert!(st.history.is_none());
    assert_eq!(st.selection, Selection::Commit(head));
    assert!(matches!(&fx[..], [Effect::LoadDetail { oid, .. }] if *oid == head), "{fx:?}");
    assert!(st.view.scroll_to_row.is_some());

    let (mut st, dir2) = with_view("history-graph-hidden");
    let fx = update(&mut st, Msg::History(HistoryMsg::ShowInGraph(oid(9))), Instant::now());
    assert!(fx.is_empty());
    assert!(st.history.is_some());
    let toast = st.toasts.last().expect("a toast");
    assert!(!toast.ok && toast.text.contains("not visible on the graph"), "{}", toast.text);
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&dir2);
}

#[test]
fn other_views_close_file_history() {
    let (mut st, dir) = with_view("history-close");
    let head = st.snap.as_ref().and_then(|s| s.head_oid).unwrap();
    let reopen = |st: &mut AppState| {
        let mut v = FileHistoryView::loading("a.txt".into(), None);
        v.loading = false;
        st.history = Some(v);
        st.history_gen
    };
    let snap = Box::new(repo::load(&dir, &LoadOptions::all(100)).unwrap());
    let msgs = [
        Msg::Repo(RepoMsg::OpenDiff(DiffTarget::Commit(head, "a.txt".into()))),
        Msg::Repo(RepoMsg::SelectCommit(head)),
        Msg::Repo(RepoMsg::JumpTo(head)),
        Msg::Conflict(ConflictMsg::OpenConflict("a.txt".into())),
    ];
    for m in msgs {
        let before = reopen(&mut st);
        update(&mut st, m, Instant::now());
        assert!(st.history.is_none() && st.history_gen > before);
    }
    let before = reopen(&mut st);
    st.loading = true;
    let m = Msg::Repo(RepoMsg::RepoLoaded { generation: st.load_gen, kind: LoadKind::Open, result: Ok(snap) });
    update(&mut st, m, Instant::now());
    assert!(st.history.is_none() && st.history_gen > before);

    reopen(&mut st);
    st.diff = Some((DiffTarget::Commit(head, "a.txt".into()), empty_diff().into()));
    update(&mut st, Msg::History(HistoryMsg::CloseFileHistory), Instant::now());
    assert!(st.history.is_none() && st.diff.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn history_error_is_shown_and_logged() {
    let (mut st, dir) = opened("history-error");
    update(&mut st, Msg::History(HistoryMsg::OpenFileHistory { path: "a.txt".into(), at: None }), Instant::now());
    let m = Msg::History(HistoryMsg::FileHistoryLoaded { path: "a.txt".into(), generation: st.history_gen, result: Err("boom".into()) });
    update(&mut st, m, Instant::now());
    let v = st.history.as_ref().unwrap();
    assert_eq!(v.error.as_deref(), Some("boom"));
    assert!(!v.loading && !v.complete());
    assert!(st.log.last().is_some_and(|l| !l.ok && l.text == "Could not load the history of a.txt: boom"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn tab_switch_parks_file_history() {
    let (a, b) = (scratch_repo("history-tab-a"), scratch_repo("history-tab-b"));
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(a.clone())));
    update(&mut st, Msg::History(HistoryMsg::OpenFileHistory { path: "a.txt".into(), at: None }), Instant::now());
    let first = st.history_gen;
    drive(&mut st, Msg::Repo(RepoMsg::Open(b.clone())));
    assert!(st.history.is_none(), "B has no history open");
    let fx = update(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)), Instant::now());
    let v = st.history.as_ref().expect("restored");
    assert!(v.loading && v.path == "a.txt");
    assert!(st.history_gen > first);
    let gen_now = st.history_gen;
    assert!(
        fx.iter().any(|e| matches!(e, Effect::LoadFileHistory { start: HistoryStart::Head, generation, .. } if *generation == gen_now)),
        "{fx:?}"
    );
    let _ = std::fs::remove_dir_all(&a);
    let _ = std::fs::remove_dir_all(&b);
}

/// The target of the selected entry's diff.
pub(super) fn selected_target(st: &AppState) -> DiffTarget {
    let v = st.history.as_ref().expect("view open");
    let e = &v.entries[v.selected.expect("an entry selected")];
    DiffTarget::Commit(e.oid, e.path.clone())
}

/// Runs `effect` and feeds its results back, as `drive` does.
pub(super) fn run(st: &mut AppState, effect: Effect) {
    if let Some(m) = crate::runner::execute(effect) {
        drive(st, m);
    }
}

#[test]
fn tab_switch_reloads_the_selected_entrys_diff() {
    let (a, b) = (scratch_repo("history-tab-diff-a"), scratch_repo("history-tab-diff-b"));
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(a.clone())));
    let fx = update(&mut st, Msg::History(HistoryMsg::OpenFileHistory { path: "a.txt".into(), at: None }), Instant::now());
    // The walk lands, selecting entry 0 and asking for its diff, which is still in flight
    // when the user switches tabs: its result is dropped.
    let walk = fx.into_iter().find(|e| matches!(e, Effect::LoadFileHistory { .. })).unwrap();
    if let Some(m) = crate::runner::execute(walk) {
        update(&mut st, m, Instant::now());
    }
    assert_eq!(st.history.as_ref().unwrap().selected, Some(0));
    assert!(st.diff.is_none());
    drive(&mut st, Msg::Repo(RepoMsg::Open(b.clone())));
    let fx = update(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)), Instant::now());
    let want = selected_target(&st);
    let load = fx.into_iter().find(|e| matches!(e, Effect::LoadDiff { target, .. } if *target == want));
    run(&mut st, load.expect("the selected entry's diff is asked for again"));
    assert_eq!(st.diff.as_ref().map(|(t, _)| t), Some(&want));
    let _ = std::fs::remove_dir_all(&a);
    let _ = std::fs::remove_dir_all(&b);
}

#[test]
fn a_refresh_does_not_supersede_the_selected_entrys_diff() {
    let dir = scratch_repo("history-refresh-diff");
    git(&dir, &["commit", "-q", "-am", "edit a"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::History(HistoryMsg::OpenFileHistory { path: "a.txt".into(), at: None }));
    assert_eq!(st.diff.as_ref().map(|(t, _)| t.clone()), Some(selected_target(&st)), "entry 0 on screen");
    // Entry 1's diff is in flight when a refresh (window focus, file watcher) reloads the diff.
    let fx = update(&mut st, Msg::History(HistoryMsg::SelectHistoryEntry(1)), Instant::now());
    let [pending] = <[Effect; 1]>::try_from(fx).unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    run(&mut st, pending);
    assert_eq!(st.diff.as_ref().map(|(t, _)| t.clone()), Some(selected_target(&st)), "entry 1 on screen");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn loaded_history_records_on_disk() {
    let (mut st, dir) = opened("history-on-disk");
    update(&mut st, Msg::History(HistoryMsg::OpenFileHistory { path: "a.txt".into(), at: None }), Instant::now());
    let first = FileHistory { entries: vec![entry(1, "a.txt")], on_disk: true, ..Default::default() };
    let m = loaded(&st, first);
    update(&mut st, m, Instant::now());
    assert!(st.history.as_ref().unwrap().on_disk);
    let later = FileHistory { entries: vec![entry(2, "a.txt")], on_disk: false, ..Default::default() };
    let m = loaded(&st, later);
    update(&mut st, m, Instant::now());
    assert!(!st.history.as_ref().unwrap().on_disk, "the latest batch wins");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn history_in_workdir_prefers_status() {
    let dir = scratch_repo("history-in-workdir");
    std::fs::remove_file(dir.join("a.txt")).unwrap();
    std::fs::write(dir.join("new.txt"), "new\n").unwrap();
    let snap = repo::load(&dir, &LoadOptions::all(100)).unwrap();
    assert!(snap.unstaged.iter().any(|f| f.path == "a.txt" && f.status == 'D'), "a.txt deleted");
    let view = |path: &str, on_disk: bool| FileHistoryView { on_disk, ..FileHistoryView::loading(path.into(), None) };

    assert!(!view("a.txt", true).in_workdir(Some(&snap)), "an unstaged deletion wins over on_disk");
    assert!(view("new.txt", false).in_workdir(Some(&snap)), "any other status entry means present");
    assert!(view("other.txt", true).in_workdir(Some(&snap)) && !view("other.txt", false).in_workdir(Some(&snap)));
    assert!(view("a.txt", true).in_workdir(None) && !view("a.txt", false).in_workdir(None));

    git(&dir, &["add", "new.txt"]);
    let snap = repo::load(&dir, &LoadOptions::all(100)).unwrap();
    assert!(snap.staged.iter().any(|f| f.path == "new.txt" && f.status == 'A'));
    assert!(view("new.txt", false).in_workdir(Some(&snap)), "a staged addition means present");
    let _ = std::fs::remove_dir_all(&dir);
}
