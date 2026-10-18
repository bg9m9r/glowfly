//! Headless tests for re-running a blame at HEAD when a refresh finds HEAD moved: the request,
//! keeping the old blame and the top line, stale results, errors, following a rename, held
//! repositories, parked tabs and the toggles while a re-run is in flight.

use crate::effect::{BlameRequest, Effect, LoadKind};
use crate::msg::{BlameMsg, Msg, RepoMsg, TabsMsg};
use crate::state::AppState;
use crate::state::blame::{BlameData, BlameLanding, BlameView};
use crate::update::tests::{drive, git, scratch_repo, state};
use crate::update::update;
use git2::Oid;
use std::path::{Path, PathBuf};
use std::time::Instant;
use trellis_core::repo::{self, Blame, LoadOptions};

fn oid(n: u8) -> Oid {
    Oid::from_bytes(&[n; 20]).unwrap()
}

/// A blame of `path` with `n` lines and no commits.
fn data(path: &str, n: usize) -> BlameData {
    let blame = Blame {
        path: path.into(),
        at: oid(1),
        commits: Vec::new(),
        lines: (0..n).map(|i| format!("line {i}")).collect(),
        runs: Vec::new(),
        origins: Vec::new(),
        orig_lines: (0..n).collect(),
        language: None,
    };
    BlameData::new(blame, None)
}

/// Shows `data` in a blame of a.txt at `at`.
fn show(st: &mut AppState, at: Option<Oid>, data: BlameData) {
    let mut view = BlameView::loading("a.txt".into(), at);
    view.loading = false;
    view.data = Some(data);
    st.blame = Some(view);
}

/// A repository with a blame of a.txt at HEAD showing 20 lines, its top line at 5.
fn shown(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    show(&mut st, None, data("a.txt", 20));
    st.blame.as_mut().unwrap().top_line = 5;
    (st, dir)
}

/// A refresh whose snapshot has HEAD at `head` (the repository's own state otherwise).
fn refresh_with_head(st: &mut AppState, head: Option<Oid>) -> Vec<Effect> {
    let dir = st.workdir().unwrap();
    let mut snap = repo::load(&dir, &LoadOptions::all(100)).unwrap();
    snap.head_oid = head;
    let m = Msg::Repo(RepoMsg::RepoLoaded { generation: st.load_gen, kind: LoadKind::Refresh, result: Ok(Box::new(snap)) });
    update(st, m, Instant::now())
}

fn requests(fx: &[Effect]) -> Vec<&BlameRequest> {
    fx.iter().filter_map(|e| if let Effect::LoadBlame(r) = e { Some(r) } else { None }).collect()
}

fn view(st: &AppState) -> &BlameView {
    st.blame.as_ref().expect("blame open")
}

fn head_of(st: &AppState) -> Option<Oid> {
    st.snap.as_ref().and_then(|s| s.head_oid)
}

fn send(st: &mut AppState, m: BlameMsg) -> Vec<Effect> {
    update(st, Msg::Blame(m), Instant::now())
}

fn loaded(path: &str, generation: u64, result: Result<Box<BlameData>, String>) -> BlameMsg {
    BlameMsg::Loaded { path: path.into(), generation, result }
}

/// Writes `text` to `file` and commits everything.
fn commit(dir: &Path, file: &str, text: &str, message: &str) {
    std::fs::write(dir.join(file), text).unwrap();
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", message]);
}

#[test]
fn head_move_reblames_a_head_blame_keeping_the_top_line() {
    let (mut st, dir) = shown("blame-head-rerun");
    let old = head_of(&st);
    let fx = refresh_with_head(&mut st, Some(oid(9)));
    let now = st.blame_gen;
    let r = requests(&fx);
    assert!(
        matches!(&r[..], [r] if r.at.is_none() && r.path == "a.txt" && r.generation == now && r.follow == old && old.is_some()),
        "{fx:?}"
    );
    let v = view(&st);
    assert!(v.loading && v.data.is_some() && v.updating(), "the old blame stays on screen");
    assert_eq!(v.land, Some(BlameLanding { line: 5, center: false }));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn same_head_refresh_leaves_blame_alone() {
    let (mut st, dir) = shown("blame-head-same");
    let head = head_of(&st);
    st.snap.as_mut().unwrap().refs_key ^= 1;
    let fx = refresh_with_head(&mut st, head);
    assert!(requests(&fx).is_empty(), "{fx:?}");
    assert!(!view(&st).loading);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn blame_at_a_commit_is_not_rerun() {
    let (mut st, dir) = shown("blame-head-at");
    show(&mut st, Some(oid(3)), data("a.txt", 4));
    let fx = refresh_with_head(&mut st, Some(oid(9)));
    assert!(requests(&fx).is_empty(), "{fx:?}");
    assert!(!view(&st).loading && view(&st).follow.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn two_moves_keep_the_earliest_follow() {
    let (mut st, dir) = shown("blame-head-twice");
    let old = head_of(&st);
    refresh_with_head(&mut st, Some(oid(9)));
    let fx = refresh_with_head(&mut st, Some(oid(8)));
    let r = requests(&fx);
    assert!(matches!(&r[..], [r] if r.follow == old && r.generation == st.blame_gen), "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stale_result_after_a_rerun_is_dropped() {
    let (mut st, dir) = shown("blame-head-stale");
    let before = st.blame_gen;
    st.blame.as_mut().unwrap().cursor = Some(15);
    refresh_with_head(&mut st, Some(oid(9)));
    send(&mut st, loaded("a.txt", before, Ok(Box::new(data("a.txt", 3)))));
    let v = view(&st);
    assert!(v.loading, "the older result is ignored");
    assert_eq!(v.line_count(), 20, "the old blame stays");

    let now = st.blame_gen;
    send(&mut st, loaded("a.txt", now, Ok(Box::new(data("a.txt", 10)))));
    let v = view(&st);
    assert!(!v.loading && v.error.is_none());
    assert_eq!(v.line_count(), 10);
    assert_eq!(v.scroll_to, Some(BlameLanding { line: 5, center: false }));
    assert_eq!(v.cursor, None, "past the new end");
    assert_eq!((v.follow, v.data_gen), (None, now));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rerun_error_replaces_the_blame() {
    let (mut st, dir) = shown("blame-head-error");
    refresh_with_head(&mut st, Some(oid(9)));
    let now = st.blame_gen;
    send(&mut st, loaded("a.txt", now, Err("a.txt is not in commit 0909090".into())));
    let v = view(&st);
    assert!(v.data.is_none() && !v.loading && v.follow.is_none());
    assert_eq!(v.error.as_deref(), Some("a.txt is not in commit 0909090"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rerun_takes_the_followed_name() {
    let (mut st, dir) = shown("blame-head-name");
    refresh_with_head(&mut st, Some(oid(9)));
    let now = st.blame_gen;
    send(&mut st, loaded("a.txt", now, Ok(Box::new(data("b.txt", 4)))));
    assert_eq!(view(&st).path, "b.txt");

    show(&mut st, Some(oid(3)), data("a.txt", 4));
    st.blame.as_mut().unwrap().loading = true;
    let now = st.blame_gen;
    send(&mut st, loaded("a.txt", now, Ok(Box::new(data("old.txt", 4)))));
    assert_eq!(view(&st).path, "a.txt", "a blame at a commit keeps its path");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn held_repository_does_not_rerun_or_prompt() {
    let (mut st, dir) = crate::update::test_mods::trust_held_tests::held_repo("blame-head-held");
    show(&mut st, None, data("a.txt", 3));
    let fx = refresh_with_head(&mut st, Some(oid(9)));
    assert!(requests(&fx).is_empty(), "{fx:?}");
    assert!(st.modal.is_none(), "no trust prompt");
    assert!(!view(&st).loading && view(&st).follow.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn toggle_during_a_rerun_keeps_follow() {
    let (mut st, dir) = shown("blame-head-toggle");
    let old = head_of(&st);
    refresh_with_head(&mut st, Some(oid(9)));
    let fx = send(&mut st, BlameMsg::SetIgnoreWhitespace(true));
    let r = requests(&fx);
    assert!(matches!(&r[..], [r] if r.follow == old), "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn back_and_parent_clear_follow() {
    use crate::state::blame::BlameStep;
    use trellis_core::repo::{BlameCommit, BlameOrigin, BlameRun};
    let (mut st, dir) = shown("blame-head-back");
    let step = BlameStep { path: "a.txt".into(), at: Some(oid(4)), skipped: Vec::new(), top_line: 0, cursor: None };
    st.blame.as_mut().unwrap().back.push(step);
    refresh_with_head(&mut st, Some(oid(9)));
    let fx = send(&mut st, BlameMsg::Back);
    assert!(matches!(&requests(&fx)[..], [r] if r.follow.is_none() && r.at == Some(oid(4))), "{fx:?}");

    let mut d = data("a.txt", 2);
    d.blame.commits = vec![BlameCommit {
        oid: oid(5),
        author: "Ada".into(),
        email: "a@example.com".into(),
        time: 0,
        summary: "s".into(),
        body: String::new(),
    }];
    d.blame.runs = vec![BlameRun { commit: 0, origin: 0, start: 0, len: 2 }];
    d.blame.origins = vec![BlameOrigin { commit: 0, path: "a.txt".into(), previous: Some((oid(6), "a.txt".into())) }];
    show(&mut st, None, d);
    refresh_with_head(&mut st, Some(oid(8)));
    assert!(view(&st).follow.is_some());
    let fx = send(&mut st, BlameMsg::BlameParent { line: 0 });
    assert!(matches!(&requests(&fx)[..], [r] if r.follow.is_none() && r.at == Some(oid(6))), "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A repository with a.txt committed as "one\ntwo\n" and a blame of it at HEAD loaded.
fn blamed(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    git(&dir, &["commit", "-q", "-am", "second"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Blame(BlameMsg::Open { path: "a.txt".into(), at: None }));
    assert_eq!(view(&st).line_count(), 2);
    (st, dir)
}

#[test]
fn rerun_e2e_after_a_commit() {
    let (mut st, dir) = blamed("blame-head-e2e");
    let commits = view(&st).data.as_ref().unwrap().blame.commits.len();
    commit(&dir, "a.txt", "one\ntwo\nthree\n", "third");
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let v = view(&st);
    assert!(!v.loading && v.error.is_none(), "{:?}", v.error);
    let d = v.data.as_ref().unwrap();
    assert_eq!(d.blame.lines, ["one", "two", "three"]);
    assert_eq!(d.blame.commits.len(), commits + 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rerun_follows_a_rename_e2e() {
    let (mut st, dir) = blamed("blame-head-e2e-rename");
    git(&dir, &["mv", "a.txt", "b.txt"]);
    git(&dir, &["commit", "-q", "-m", "rename"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let v = view(&st);
    assert!(v.error.is_none(), "{:?}", v.error);
    assert_eq!(v.path, "b.txt");
    assert_eq!(v.line_count(), 2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rerun_after_a_delete_shows_the_error() {
    let (mut st, dir) = blamed("blame-head-e2e-delete");
    commit(&dir, "other.txt", "x\n", "other");
    git(&dir, &["rm", "-q", "a.txt"]);
    git(&dir, &["commit", "-q", "-m", "remove"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let v = view(&st);
    assert!(v.data.is_none() && !v.loading);
    let e = v.error.as_deref().unwrap_or_default();
    assert!(e.contains("a.txt is not in commit"), "{e}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn head_move_while_parked_reblames_on_return() {
    let (mut st, a) = blamed("blame-head-park-a");
    let b = scratch_repo("blame-head-park-b");
    drive(&mut st, Msg::Repo(RepoMsg::Open(b.clone())));
    assert!(st.blame.is_none());
    commit(&a, "a.txt", "one\ntwo\nthree\n", "third");
    drive(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)));
    let v = view(&st);
    assert!(!v.loading && v.error.is_none(), "{:?}", v.error);
    assert_eq!(v.data.as_ref().unwrap().blame.lines, ["one", "two", "three"]);
    let _ = std::fs::remove_dir_all(&a);
    let _ = std::fs::remove_dir_all(&b);
}
