//! Headless tests for the blame view: requests, results, stale results, selecting a commit,
//! what closes it, tab parking and held repositories.

use super::blame::HELD;
use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::{BlameRequest, Effect, LoadKind};
use crate::msg::{BlameMsg, ConflictMsg, HistoryMsg, Msg, RebaseMsg, RepoMsg, TabsMsg};
use crate::state::blame::{BlameData, BlameView};
use crate::state::conflict::ConflictView;
use crate::state::history::FileHistoryView;
use crate::state::{AppState, Modal, Selection};
use git2::Oid;
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::rebase::RebaseBase;
use trellis_core::repo::{self, Blame, BlameOptions, DiffTarget, FileDiff, LoadOptions};
use trellis_core::settings::{BlameMoves, BlamePrefs};

fn oid(n: u8) -> Oid {
    Oid::from_bytes(&[n; 20]).unwrap()
}

fn empty_diff() -> FileDiff {
    FileDiff { binary: false, hunks: Vec::new(), highlight: None, intraline: Default::default(), whole: None }
}

fn opened(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    (st, dir)
}

fn open(path: &str, at: Option<Oid>) -> Msg {
    Msg::Blame(BlameMsg::Open { path: path.into(), at })
}

fn data(path: &str) -> Box<BlameData> {
    let blame = Blame {
        path: path.into(),
        at: oid(1),
        commits: Vec::new(),
        lines: vec!["x".into()],
        runs: Vec::new(),
        origins: Vec::new(),
        orig_lines: vec![0],
        language: None,
    };
    Box::new(BlameData::new(blame, None))
}

fn loaded(path: &str, generation: u64, result: Result<Box<BlameData>, String>) -> Msg {
    Msg::Blame(BlameMsg::Loaded { path: path.into(), generation, result })
}

fn head(st: &AppState) -> Oid {
    st.snap.as_ref().and_then(|s| s.head_oid).unwrap()
}

#[test]
fn opening_blame_requests_a_load() {
    let (mut st, dir) = opened("blame-open");
    st.diff = Some((DiffTarget::Unstaged("a.txt".into()), empty_diff().into()));
    st.conflict = Some(ConflictView::loading("a.txt".into()));
    st.settings.blame = BlamePrefs { ignore_whitespace: true, moves: BlameMoves::WithinFile };
    let fx = update(&mut st, open("a.txt", None), Instant::now());
    let hl = st.settings.syntax_highlighting;
    let want = BlameOptions { ignore_whitespace: true, moves: BlameMoves::WithinFile, ignore_revs: Vec::new() };
    assert!(
        matches!(&fx[..], [Effect::LoadBlame(BlameRequest { path, at: None, generation: 1, highlight, opts, .. })]
            if path == "a.txt" && *highlight == hl && *opts == want),
        "{fx:?}"
    );
    let v = st.blame.as_ref().expect("view open");
    assert!(v.loading && v.data.is_none() && v.error.is_none());
    assert!(st.diff.is_none() && st.conflict.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn opening_blame_from_history_keeps_it() {
    let (mut st, dir) = opened("blame-over-history");
    let mut h = FileHistoryView::loading("a.txt".into(), None);
    h.loading = false;
    st.history = Some(h);
    let target = DiffTarget::Commit(head(&st), "a.txt".into());
    st.diff = Some((target.clone(), empty_diff().into()));
    let h = head(&st);
    update(&mut st, open("a.txt", Some(h)), Instant::now());
    assert!(st.blame.is_some() && st.history.is_some());
    assert_eq!(st.diff.as_ref().map(|(t, _)| t), Some(&target));
    update(&mut st, Msg::Blame(BlameMsg::Close), Instant::now());
    assert!(st.blame.is_none() && st.history.is_some() && st.diff.is_some(), "Esc returns to the history");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn loaded_fills_the_view_and_stale_results_are_dropped() {
    let (mut st, dir) = opened("blame-loaded");
    update(&mut st, open("a.txt", None), Instant::now());
    let g = st.blame_gen;
    assert!(update(&mut st, loaded("a.txt", g - 1, Ok(data("a.txt"))), Instant::now()).is_empty());
    update(&mut st, loaded("b.txt", g, Ok(data("b.txt"))), Instant::now());
    assert!(st.blame.as_ref().unwrap().loading, "another path's result is ignored");
    update(&mut st, loaded("a.txt", g, Ok(data("a.txt"))), Instant::now());
    let v = st.blame.as_ref().unwrap();
    assert!(!v.loading && v.data.as_ref().is_some_and(|d| d.blame.lines == ["x"]));

    update(&mut st, open("a.txt", None), Instant::now());
    let g = st.blame_gen;
    update(&mut st, loaded("a.txt", g, Err("boom".into())), Instant::now());
    let v = st.blame.as_ref().unwrap();
    assert_eq!(v.error.as_deref(), Some("boom"));
    assert!(!v.loading && v.data.is_none());
    assert!(st.log.last().is_some_and(|l| !l.ok && l.text == "boom"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn close_ignores_a_late_result() {
    let (mut st, dir) = opened("blame-late");
    update(&mut st, open("a.txt", None), Instant::now());
    let g = st.blame_gen;
    update(&mut st, Msg::Blame(BlameMsg::Close), Instant::now());
    update(&mut st, loaded("a.txt", g, Ok(data("a.txt"))), Instant::now());
    assert!(st.blame.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn select_commit_selects_on_graph_and_closes() {
    let (mut st, dir) = opened("blame-select");
    let h = head(&st);
    let mut hist = FileHistoryView::loading("a.txt".into(), None);
    hist.loading = false;
    st.history = Some(hist);
    st.blame = Some(BlameView::loading("a.txt".into(), None));
    let fx = update(&mut st, Msg::Blame(BlameMsg::SelectCommit(h)), Instant::now());
    assert!(st.blame.is_none() && st.history.is_none());
    assert_eq!(st.selection, Selection::Commit(h));
    assert!(matches!(&fx[..], [Effect::LoadDetail { oid, .. }] if *oid == h), "{fx:?}");

    st.blame = Some(BlameView::loading("a.txt".into(), None));
    let fx = update(&mut st, Msg::Blame(BlameMsg::SelectCommit(oid(9))), Instant::now());
    assert!(fx.is_empty() && st.blame.is_some());
    let toast = st.toasts.last().expect("a toast");
    assert!(!toast.ok && toast.text == super::history::not_on_graph(oid(9)), "{}", toast.text);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn other_views_close_blame() {
    let (mut st, dir) = opened("blame-closed-by");
    let h = head(&st);
    let msgs = [
        Msg::Repo(RepoMsg::OpenDiff(DiffTarget::Commit(h, "a.txt".into()))),
        Msg::Conflict(ConflictMsg::OpenConflict("a.txt".into())),
        Msg::History(HistoryMsg::OpenFileHistory { path: "a.txt".into(), at: None }),
        Msg::Repo(RepoMsg::SelectCommit(h)),
        Msg::Rebase(RebaseMsg::OpenRebase(RebaseBase::FromCommit(h))),
    ];
    for m in msgs {
        st.blame = Some(BlameView::loading("a.txt".into(), None));
        let before = st.blame_gen;
        update(&mut st, m, Instant::now());
        assert!(st.blame.is_none() && st.blame_gen > before);
    }
    assert!(st.rebase.is_some(), "the rebase panel opened");
    st.blame = Some(BlameView::loading("a.txt".into(), None));
    let snap = Box::new(repo::load(&dir, &LoadOptions::all(100)).unwrap());
    st.loading = true;
    let m = Msg::Repo(RepoMsg::RepoLoaded { generation: st.load_gen, kind: LoadKind::Open, result: Ok(snap) });
    update(&mut st, m, Instant::now());
    assert!(st.blame.is_none(), "reopening the repository closes it");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn blame_is_parked_with_its_tab() {
    let (a, b) = (scratch_repo("blame-tab-a"), scratch_repo("blame-tab-b"));
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(a.clone())));
    update(&mut st, open("a.txt", None), Instant::now());
    let first = st.blame_gen;
    drive(&mut st, Msg::Repo(RepoMsg::Open(b.clone())));
    assert!(st.blame.is_none(), "B has no blame open");
    let fx = update(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)), Instant::now());
    let v = st.blame.as_ref().expect("restored");
    assert!(v.loading && v.path == "a.txt");
    assert!(st.blame_gen > first);
    let now = st.blame_gen;
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadBlame(r) if r.generation == now && r.path == "a.txt")), "{fx:?}");
    let _ = std::fs::remove_dir_all(&a);
    let _ = std::fs::remove_dir_all(&b);
}

#[test]
fn held_repository_asks_for_trust() {
    let (mut st, dir) = super::trust_held_tests::held_repo("blame-held");
    let fx = update(&mut st, open("a.txt", None), Instant::now());
    assert!(!fx.iter().any(|e| matches!(e, Effect::LoadBlame(_))), "{fx:?}");
    let v = st.blame.as_ref().expect("view open");
    assert_eq!(v.error.as_deref(), Some(HELD));
    assert!(!v.loading);
    assert!(matches!(st.modal, Some(Modal::Trust(_))), "the trust prompt opens");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn blame_end_to_end() {
    let dir = scratch_repo("blame-e2e");
    std::fs::write(dir.join("main.rs"), "fn main() {\n    println!(\"hi\");\n}\n").unwrap();
    git(&dir, &["add", "main.rs"]);
    git(&dir, &["commit", "-q", "-m", "add main"]);
    std::fs::write(dir.join("main.rs"), "fn main() {\n    println!(\"hello\");\n}\n").unwrap();
    git(&dir, &["commit", "-q", "-am", "greet"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, open("main.rs", None));
    let v = st.blame.as_ref().expect("view open");
    assert!(!v.loading && v.error.is_none(), "{:?}", v.error);
    let d = v.data.as_ref().expect("loaded");
    assert_eq!(d.blame.lines.len(), 3);
    assert_eq!(d.blame.runs.len(), 3);
    assert_eq!(d.blame.commits.len(), 2);
    assert!(!d.spans(0).is_empty(), "a .rs file is highlighted");
    let _ = std::fs::remove_dir_all(&dir);
}
