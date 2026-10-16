//! Status reads of only the paths the file watcher reported: what asks for them, how they queue
//! and merge, when they fall back to a whole read, and when the open diff is read again.

use crate::effect::{Effect, StatusRead};
use crate::msg::{LoadMsg, Msg, RepoMsg, StageMsg, WatchMsg};
use crate::runner;
use crate::state::AppState;
use crate::state::refresh::Wanted;
use crate::update::tests::{drive, finish, git, scratch_repo, state};
use crate::update::update;
use std::path::{Path, PathBuf};
use std::time::Instant;
use trellis_core::repo::DiffTarget;
use trellis_core::watch::MAX_WATCH_PATHS;

/// A clean repository (a.txt committed), opened.
fn opened(name: &str) -> (AppState, PathBuf, PathBuf) {
    let dir = scratch_repo(name);
    git(&dir, &["checkout", "-q", "--", "a.txt"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let workdir = st.workdir().expect("opened");
    (st, dir, workdir)
}

fn owned(p: &[&str]) -> Vec<String> {
    p.iter().map(|s| s.to_string()).collect()
}

fn changed(workdir: &Path, full: bool, paths: Option<&[&str]>) -> Msg {
    let msg = WatchMsg::RepoChanged { full, paths: paths.map(owned), last_event: Instant::now() };
    Msg::Scoped { workdir: workdir.to_path_buf(), msg: Box::new(Msg::Watch(msg)) }
}

/// The paths of the one status read in `fx`.
fn read_paths(fx: &[Effect]) -> Option<Vec<String>> {
    match fx {
        [Effect::LoadStatus { paths, .. }] => paths.clone(),
        other => panic!("one status read expected: {other:?}"),
    }
}

/// Runs the one status read in `fx` and hands its result to `update`.
fn run_read(st: &mut AppState, fx: Vec<Effect>) -> Vec<Effect> {
    let read = fx.into_iter().find(|e| matches!(e, Effect::LoadStatus { .. })).expect("a status read");
    let msg = runner::execute(read).expect("a result");
    update(st, msg, Instant::now())
}

fn diff_loads(fx: &[Effect]) -> usize {
    fx.iter().filter(|e| matches!(e, Effect::LoadDiff { .. })).count()
}

#[test]
fn a_path_report_reads_only_those_paths() {
    let (mut st, dir, workdir) = opened("paths-ask");
    let fx = update(&mut st, changed(&workdir, false, Some(&["a.txt"])), Instant::now());
    assert!(
        matches!(&fx[..], [Effect::LoadStatus { paths: Some(p), generation, .. }] if p == &owned(&["a.txt"]) && *generation == st.load_gen),
        "{fx:?}"
    );
    finish(&mut st, fx);
    let fx = update(&mut st, changed(&workdir, false, None), Instant::now());
    assert_eq!(read_paths(&fx), None, "no paths: the whole status");
    finish(&mut st, fx);
    let fx = update(&mut st, changed(&workdir, true, Some(&["a.txt"])), Instant::now());
    assert!(matches!(&fx[..], [Effect::LoadRepo { known: Some(_), .. }]), "full wins: {fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_edit_shows_in_the_lists_counts_and_graph() {
    let (mut st, dir, workdir) = opened("paths-edit");
    assert!(st.snap.as_ref().unwrap().graph.wip.is_none(), "clean");
    std::fs::write(dir.join("a.txt"), "edited\n").unwrap();
    let fx = update(&mut st, changed(&workdir, false, Some(&["a.txt"])), Instant::now());
    let fx = run_read(&mut st, fx);
    finish(&mut st, fx);
    let snap = st.snap.as_ref().unwrap();
    let rows: Vec<(&str, char)> = snap.unstaged.iter().map(|f| (f.path.as_str(), f.status)).collect();
    assert_eq!(rows, [("a.txt", 'M')]);
    assert_eq!(snap.changes.modified, 1);
    assert!(snap.graph.wip.is_some(), "the WIP row appears");
    assert!(!st.refresh.status_busy);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn reports_while_a_read_runs_queue_one_union() {
    let (mut st, dir, workdir) = opened("paths-queue");
    let first = update(&mut st, changed(&workdir, false, Some(&["a.txt"])), Instant::now());
    assert!(update(&mut st, changed(&workdir, false, Some(&["b.txt"])), Instant::now()).is_empty());
    assert!(update(&mut st, changed(&workdir, false, Some(&["c.txt", "b.txt"])), Instant::now()).is_empty());
    assert_eq!(st.refresh.status_queued, Some(Wanted::Paths(owned(&["b.txt", "c.txt"]).into_iter().collect())));
    let fx = run_read(&mut st, first);
    assert_eq!(read_paths(&fx), Some(owned(&["b.txt", "c.txt"])), "one more read for both");
    finish(&mut st, fx);
    assert!(st.refresh.status_queued.is_none() && !st.refresh.status_busy);

    // A whole read asked for meanwhile makes the next read whole.
    let first = update(&mut st, changed(&workdir, false, Some(&["a.txt"])), Instant::now());
    update(&mut st, changed(&workdir, false, Some(&["b.txt"])), Instant::now());
    update(&mut st, changed(&workdir, false, None), Instant::now());
    assert_eq!(read_paths(&run_read(&mut st, first)), None);
    st.refresh.status_busy = false;
    st.refresh.status_queued = None;

    // So does Trellis's own operation.
    let first = update(&mut st, changed(&workdir, false, Some(&["a.txt"])), Instant::now());
    update(&mut st, changed(&workdir, false, Some(&["b.txt"])), Instant::now());
    update(&mut st, Msg::Stage(StageMsg::LocalOpDone(Ok(None))), Instant::now());
    assert_eq!(st.refresh.status_queued, Some(Wanted::Whole));
    assert_eq!(read_paths(&run_read(&mut st, first)), None);
    st.refresh.status_busy = false;
    st.refresh.status_queued = None;
    // The operation's own events are ignored for a moment; these stand for later ones.
    st.watch.quiet_until = None;

    // Too many paths in the queue.
    let first = update(&mut st, changed(&workdir, false, Some(&["a.txt"])), Instant::now());
    for chunk in (0..=MAX_WATCH_PATHS).map(|i| format!("f{i}.txt")).collect::<Vec<_>>().chunks(100) {
        let paths: Vec<&str> = chunk.iter().map(String::as_str).collect();
        update(&mut st, changed(&workdir, false, Some(&paths)), Instant::now());
    }
    assert_eq!(st.refresh.status_queued, Some(Wanted::Whole), "past MAX_WATCH_PATHS");
    assert_eq!(read_paths(&run_read(&mut st, first)), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn paths_that_cannot_be_read_alone_read_everything_in_the_same_effect() {
    let (mut st, dir, workdir) = opened("paths-fallback");
    std::fs::write(dir.join("x.log"), "log\n").unwrap();
    std::fs::write(dir.join(".gitignore"), "*.log\n").unwrap();
    std::fs::write(dir.join("a.txt"), "edited\n").unwrap();
    let read = Effect::LoadStatus {
        workdir: workdir.clone(),
        generation: st.load_gen,
        submodules: false,
        open: None,
        git_status: true,
        paths: Some(owned(&[".gitignore"])),
    };
    let msg = runner::execute(read).expect("a result");
    let Msg::Scoped { msg, .. } = msg else { panic!("scoped") };
    assert!(matches!(&*msg, Msg::Load(LoadMsg::StatusLoaded { result: Ok(StatusRead::Whole(..)), .. })), "a whole read");
    st.refresh.status_busy = true;
    let fx = update(&mut st, *msg, Instant::now());
    finish(&mut st, fx);
    let snap = st.snap.as_ref().unwrap();
    let rows: Vec<(&str, char)> = snap.unstaged.iter().map(|f| (f.path.as_str(), f.status)).collect();
    assert_eq!(rows, [(".gitignore", '?'), ("a.txt", 'M')], "applied like a whole read");
    assert_eq!(snap.changes.total(), 2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_open_diff_is_read_again_only_when_its_file_is_covered_and_changed() {
    let (mut st, dir, workdir) = opened("paths-diff");
    std::fs::write(dir.join("a.txt"), "edited\n").unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(DiffTarget::Unstaged("a.txt".into()))));
    assert!(st.diff.is_some());
    let ask = update(&mut st, changed(&workdir, false, Some(&["a.txt"])), Instant::now());
    let fx = run_read(&mut st, ask);
    finish(&mut st, fx);
    let stamp = st.refresh.open_stamp;
    assert!(stamp.is_some());

    std::fs::write(dir.join("b.txt"), "new\n").unwrap();
    let ask = update(&mut st, changed(&workdir, false, Some(&["b.txt"])), Instant::now());
    assert!(matches!(&ask[..], [Effect::LoadStatus { open: None, .. }]), "the open file's stamp isn't needed: {ask:?}");
    let fx = run_read(&mut st, ask);
    assert_eq!(diff_loads(&fx), 0, "{fx:?}");
    assert_eq!(st.refresh.open_stamp, stamp, "left alone");
    assert!(st.snap.as_ref().unwrap().unstaged.iter().any(|f| f.path == "b.txt"));

    std::fs::write(dir.join("a.txt"), "edited again\nand more\n").unwrap();
    let ask = update(&mut st, changed(&workdir, false, Some(&["a.txt"])), Instant::now());
    let fx = run_read(&mut st, ask);
    assert_eq!(diff_loads(&fx), 1, "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn trellis_s_own_operations_and_polls_read_everything() {
    let (mut st, dir, _workdir) = opened("paths-own");
    std::fs::write(dir.join("a.txt"), "edited\n").unwrap();
    let fx = update(&mut st, Msg::Stage(StageMsg::StageFile("a.txt".into())), Instant::now());
    let op = fx.into_iter().find(|e| matches!(e, Effect::LocalOp { .. })).expect("a local op");
    let done = runner::execute(op).expect("a result");
    let fx = update(&mut st, done, Instant::now());
    assert_eq!(read_paths(&fx), None, "staging reads the whole status");
    finish(&mut st, fx);

    st.view.was_focused = true;
    let remote = Msg::Watch(WatchMsg::RemoteVolume { workdir: st.workdir().unwrap(), kind: "SMB" });
    update(&mut st, remote, Instant::now());
    let t0 = Instant::now();
    update(&mut st, Msg::Tick(t0), t0);
    let t1 = t0 + st.watch.poll_interval().expect("polled");
    assert_eq!(read_paths(&update(&mut st, Msg::Tick(t1), t1)), None, "the poll reads the whole status");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_path_read_of_an_older_load_is_dropped() {
    let (mut st, dir, workdir) = opened("paths-stale");
    std::fs::write(dir.join("a.txt"), "edited\n").unwrap();
    let ask = update(&mut st, changed(&workdir, false, Some(&["a.txt"])), Instant::now());
    let msg = runner::execute(ask.into_iter().next().unwrap()).unwrap();
    st.load_gen += 1;
    assert!(update(&mut st, msg, Instant::now()).is_empty());
    assert!(st.snap.as_ref().unwrap().unstaged.is_empty(), "not applied");
    let _ = std::fs::remove_dir_all(&dir);
}
