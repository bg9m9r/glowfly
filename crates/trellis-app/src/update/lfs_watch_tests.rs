//! Hand edits to .gitattributes reaching the LFS patterns without a full reload: whole status
//! reads (the watcher's, a Checked refresh that finds the refs unchanged) bring the root file's
//! patterns again, reads of some paths and stale results leave them alone, and an open Git LFS
//! page reads its status again in place.

use super::tests::{drive, finish, scratch_repo, state};
use super::update;
use crate::effect::{Effect, StatusRead};
use crate::msg::{LoadMsg, Msg, PrefsMsg, RepoMsg, WatchMsg};
use crate::runner;
use crate::state::lfs::{LfsInstall, TrackContext, track_items};
use crate::state::{AppState, PrefPage};
use std::path::{Path, PathBuf};
use std::time::Instant;
use trellis_core::lfs::{LfsPattern, LfsStatus};

fn opened(name: &str) -> (AppState, PathBuf, PathBuf) {
    let dir = scratch_repo(name);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let workdir = st.workdir().expect("opened");
    (st, dir, workdir)
}

fn watched(workdir: &Path) -> Msg {
    watched_full(workdir, false)
}

fn watched_full(workdir: &Path, full: bool) -> Msg {
    let msg = WatchMsg::RepoChanged { full, paths: None, last_event: Instant::now() };
    Msg::Scoped { workdir: workdir.to_path_buf(), msg: Box::new(Msg::Watch(msg)) }
}

fn raws(st: &AppState) -> Vec<String> {
    st.snap.as_ref().unwrap().lfs_patterns.iter().map(|p| p.raw.clone()).collect()
}

fn pattern(raw: &str) -> LfsPattern {
    LfsPattern { raw: raw.into(), display: raw.into(), lockable: false }
}

/// Whether the file menu's "Every .bin file" entry is disabled because the pattern is tracked.
fn bin_tracked(st: &AppState) -> bool {
    let items = track_items("data/file.bin", TrackContext::of(st));
    items.iter().find(|i| i.label == "Every .bin file").expect("an extension entry").blocked.is_some()
}

#[test]
fn a_watched_gitattributes_edit_reads_the_whole_status_and_the_patterns() {
    let (mut st, dir, workdir) = opened("lfs-watch-edit");
    st.lfs.install = LfsInstall::Installed("3.7.1".into());
    assert!(raws(&st).is_empty());
    assert!(!bin_tracked(&st));
    std::fs::write(dir.join(".gitattributes"), "*.bin filter=lfs -text\n").unwrap();
    let fx = update(&mut st, watched(&workdir), Instant::now());
    assert!(matches!(&fx[..], [Effect::LoadStatus { paths: None, .. }]), "one whole status read, no reload: {fx:?}");
    finish(&mut st, fx);
    assert_eq!(raws(&st), ["*.bin"]);
    assert!(bin_tracked(&st), "the file menu sees the new pattern");

    std::fs::write(dir.join(".gitattributes"), "").unwrap();
    let fx = update(&mut st, watched(&workdir), Instant::now());
    assert!(!fx.iter().any(|e| matches!(e, Effect::LoadRepo { .. })), "{fx:?}");
    finish(&mut st, fx);
    assert!(raws(&st).is_empty(), "removing the line empties the patterns");
    assert!(!bin_tracked(&st));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_refresh_that_finds_the_refs_unchanged_picks_up_the_patterns() {
    let (mut st, dir, workdir) = opened("lfs-watch-unchanged");
    std::fs::write(dir.join(".gitattributes"), "*.psd filter=lfs diff=lfs merge=lfs -text\n").unwrap();
    // A rescan (events were lost) checks the refs first.
    let fx = update(&mut st, watched_full(&workdir, true), Instant::now());
    let load = fx.into_iter().find(|e| matches!(e, Effect::LoadRepo { known: Some(_), .. })).expect("a checked refresh");
    let msg = runner::execute(load).expect("a result");
    assert!(matches!(msg, Msg::Load(LoadMsg::RepoUnchanged { lfs_patterns: Some(_), .. })));
    let fx = update(&mut st, msg, Instant::now());
    finish(&mut st, fx);
    assert_eq!(raws(&st), ["*.psd"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stale_and_paths_reads_leave_the_patterns_alone() {
    let (mut st, dir, _) = opened("lfs-watch-stale");
    let whole = || {
        let status = StatusRead::Whole(Vec::new(), Vec::new(), git2::RepositoryState::Clean);
        (Ok(status), Some(vec![pattern("*.bin")]))
    };
    let (result, lfs_patterns) = whole();
    let old = st.load_gen.wrapping_sub(1);
    let msg = LoadMsg::StatusLoaded { generation: old, result, submodules: None, open_stamp: None, slow: None, lfs_patterns };
    update(&mut st, Msg::Load(msg), Instant::now());
    assert!(raws(&st).is_empty(), "an older generation changes nothing");
    let paths = trellis_core::repo::status_of_paths(&dir, &["a.txt".into()], false).unwrap().expect("a paths read");
    let generation = st.load_gen;
    let msg = LoadMsg::StatusLoaded {
        generation,
        result: Ok(StatusRead::Paths(paths)),
        submodules: None,
        open_stamp: None,
        slow: None,
        lfs_patterns: None,
    };
    update(&mut st, Msg::Load(msg), Instant::now());
    assert!(raws(&st).is_empty(), "a read of some paths brings no patterns");
    let (result, lfs_patterns) = whole();
    let msg = LoadMsg::StatusLoaded { generation, result, submodules: None, open_stamp: None, slow: None, lfs_patterns };
    update(&mut st, Msg::Load(msg), Instant::now());
    assert_eq!(raws(&st), ["*.bin"], "a current whole read does");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_open_lfs_page_reads_its_status_again_in_place() {
    let (mut st, dir, _) = opened("lfs-watch-page");
    drive(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::RepoLfs)));
    // Whatever the machine has, the page shows a status (or an error) once read; give it one.
    st.prefs.as_mut().unwrap().lfs.status = Some(LfsStatus::default());
    let read = |st: &mut AppState, patterns: Vec<LfsPattern>| {
        let generation = st.load_gen;
        let result = Ok(StatusRead::Whole(Vec::new(), Vec::new(), git2::RepositoryState::Clean));
        let msg =
            LoadMsg::StatusLoaded { generation, result, submodules: None, open_stamp: None, slow: None, lfs_patterns: Some(patterns) };
        update(st, Msg::Load(msg), Instant::now())
    };
    let fx = read(&mut st, vec![pattern("*.bin")]);
    assert_eq!(fx.iter().filter(|e| matches!(e, Effect::LoadLfs { .. })).count(), 1, "{fx:?}");
    assert!(st.prefs.as_ref().unwrap().lfs.status.is_some(), "the page keeps its status on screen");
    let fx = read(&mut st, vec![pattern("*.bin")]);
    assert!(!fx.iter().any(|e| matches!(e, Effect::LoadLfs { .. })), "the same patterns: {fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
