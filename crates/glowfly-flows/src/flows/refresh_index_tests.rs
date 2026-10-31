//! Status reads after an index change made outside Glowfly: what asks for them, how they are
//! applied to the lists, counts and open diff, and when they fall back to a whole read.

use crate::effect::{Effect, StatusRead};
use crate::flows::{drive, finish, git, scratch_repo, state};
use crate::msg::{LoadMsg, Msg, RepoMsg, WatchMsg};
use crate::runner;
use crate::state::AppState;
use crate::state::refresh::Wanted;
use crate::update::update;
use glowfly_core::repo::DiffTarget;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

/// The scratch repository (a.txt committed, then edited), opened.
fn opened(name: &str) -> (AppState, PathBuf, PathBuf) {
    let dir = scratch_repo(name);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let workdir = st.workdir().expect("opened");
    (st, dir, workdir)
}

fn index_changed(workdir: &Path, paths: &[&str]) -> Msg {
    let msg = WatchMsg::IndexChanged { paths: paths.iter().map(|s| s.to_string()).collect(), last_event: Instant::now() };
    Msg::Scoped { workdir: workdir.to_path_buf(), msg: Box::new(Msg::Watch(msg)) }
}

/// Runs the one status read in `fx` and returns its result message, unscoped.
fn result_of(fx: Vec<Effect>) -> Msg {
    let read = fx.into_iter().find(|e| matches!(e, Effect::LoadIndexStatus { .. } | Effect::LoadStatus { .. })).expect("a status read");
    match runner::testing::execute(read).expect("a result") {
        Msg::Scoped { msg, .. } => *msg,
        other => other,
    }
}

fn rows(list: &[glowfly_core::repo::FileEntry]) -> Vec<(&str, char)> {
    list.iter().map(|f| (f.path.as_str(), f.status)).collect()
}

fn diff_loads(fx: &[Effect]) -> usize {
    fx.iter().filter(|e| matches!(e, Effect::LoadDiff { .. })).count()
}

#[test]
fn a_terminal_add_moves_the_file_to_staged() {
    let (mut st, dir, workdir) = opened("index-add");
    let old = st.snap.as_ref().unwrap().index_memo.clone().expect("the load keeps a memo");
    assert_eq!(rows(&st.snap.as_ref().unwrap().unstaged), [("a.txt", 'M')]);
    git(&dir, &["add", "a.txt"]);
    let fx = update(&mut st, index_changed(&workdir, &[]), Instant::now());
    assert!(matches!(&fx[..], [Effect::LoadIndexStatus { generation, .. }] if *generation == st.load_gen), "{fx:?}");
    let msg = result_of(fx);
    assert!(matches!(&msg, Msg::Load(LoadMsg::StatusLoaded { result: Ok(StatusRead::Index { .. }), index: Some(_), .. })));
    let fx = update(&mut st, msg, Instant::now());
    finish(&mut st, fx);
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(rows(&snap.staged), [("a.txt", 'M')]);
    assert!(snap.unstaged.is_empty());
    assert_eq!((snap.changes.modified, snap.changes.total()), (1, 1));
    assert!(snap.graph.wip.is_some());
    assert!(!Arc::ptr_eq(snap.index_memo.as_ref().unwrap(), &old), "the new index is remembered");
    assert!(!st.refresh.status_busy);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_terminal_move_shows_a_rename() {
    let (mut st, dir, workdir) = opened("index-mv");
    git(&dir, &["checkout", "-q", "--", "a.txt"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    git(&dir, &["mv", "a.txt", "b.txt"]);
    let fx = update(&mut st, index_changed(&workdir, &["a.txt", "b.txt"]), Instant::now());
    finish(&mut st, fx);
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(rows(&snap.staged), [("b.txt", 'R')]);
    assert_eq!(snap.staged[0].renamed_from.as_deref(), Some("a.txt"));
    assert!(snap.unstaged.is_empty(), "{:?}", rows(&snap.unstaged));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_open_index_diff_of_a_covered_file_reloads() {
    let (mut st, dir, workdir) = opened("index-diff");
    git(&dir, &["add", "a.txt"]);
    let fx = update(&mut st, index_changed(&workdir, &[]), Instant::now());
    finish(&mut st, fx);
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(DiffTarget::Staged("a.txt".into()))));
    assert!(st.session.diff.view.is_some());

    std::fs::write(dir.join("a.txt"), "one\ntwo\nthree\n").unwrap();
    git(&dir, &["add", "a.txt"]);
    let fx = update(&mut st, index_changed(&workdir, &["a.txt"]), Instant::now());
    assert!(matches!(&fx[..], [Effect::LoadIndexStatus { open: Some(p), .. }] if p == "a.txt"), "{fx:?}");
    let fx = update(&mut st, result_of(fx), Instant::now());
    assert_eq!(diff_loads(&fx), 1, "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_conflict_falls_back_to_a_whole_read() {
    let (mut st, dir, workdir) = opened("index-conflict");
    git(&dir, &["commit", "-q", "-am", "ours"]);
    git(&dir, &["checkout", "-q", "-b", "other", "HEAD~1"]);
    std::fs::write(dir.join("a.txt"), "theirs\n").unwrap();
    git(&dir, &["commit", "-q", "-am", "theirs"]);
    git(&dir, &["checkout", "-q", "main"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let (ok, _) = glowfly_core::gitcli::run(&dir, &glowfly_core::gitcli::args(&["merge", "-q", "other"]));
    assert!(!ok, "the merge conflicts");
    let fx = update(&mut st, index_changed(&workdir, &[]), Instant::now());
    let msg = result_of(fx);
    assert!(matches!(&msg, Msg::Load(LoadMsg::StatusLoaded { result: Ok(StatusRead::Whole(..)), index: Some(_), .. })), "a whole read");
    let fx = update(&mut st, msg, Instant::now());
    finish(&mut st, fx);
    assert_eq!(rows(&st.snap.as_ref().unwrap().unstaged), [("a.txt", '!')]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_deferred_index_read_is_shown_and_queues_the_open_file() {
    let (mut st, dir, workdir) = opened("index-deferred");
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(DiffTarget::Unstaged("a.txt".into()))));
    git(&dir, &["add", "a.txt"]);
    let first = update(&mut st, index_changed(&workdir, &[]), Instant::now());
    // Another index change arrives while the first read runs.
    assert!(update(&mut st, index_changed(&workdir, &["c.txt"]), Instant::now()).is_empty());
    assert_eq!(st.refresh.status_queued, Some(Wanted::Index(["c.txt".to_string()].into())));
    let fx = update(&mut st, result_of(first), Instant::now());
    assert_eq!(rows(&st.snap.as_ref().unwrap().staged), [("a.txt", 'M')], "shown though out of date");
    assert_eq!(diff_loads(&fx), 0, "the diff waits for the next read");
    match &fx[..] {
        [Effect::LoadIndexStatus { read, .. }] => assert_eq!(read.paths, ["a.txt", "c.txt"], "the open file joins the next read"),
        other => panic!("one index read expected: {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}
