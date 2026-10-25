//! The Git LFS notices: when the main window's "git-lfs missing" banner shows and how it is
//! dismissed, and the diff's LFS note (kept per target, Show pointer text across reloads, parked
//! with its tab, no in-file stops for next/previous change while the summary shows).

use super::diff_tests::three_hunks;
use super::tests::{drive, scratch_repo, state};
use super::update;
use crate::lfs::LfsMsg;
use crate::msg::{Msg, PrefsMsg, RepoMsg, TabsMsg};
use crate::state::diff_nav::{self, Step};
use crate::state::lfs::LfsInstall;
use crate::state::lfs_notes::{banner_workdir, pointer_shown, summary};
use crate::state::lines::LineClick;
use crate::state::{AppState, PrefPage};
use git2::Oid;
use glowfly_core::lfs::pointer::{LfsChange, LfsSide, Pointer};
use glowfly_core::lfs::{LfsPattern, LfsStatus};
use glowfly_core::repo::DiffTarget;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn run(st: &mut AppState, m: Msg) {
    update(st, m, Instant::now());
}

fn opened(st: &mut AppState, name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    drive(st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let snap = st.snap.as_mut().expect("opened");
    snap.lfs_patterns = vec![LfsPattern { raw: "*.psd".into(), display: "*.psd".into(), lockable: false }];
    dir
}

fn workdir(st: &AppState) -> PathBuf {
    st.workdir().unwrap()
}

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

fn change() -> LfsChange {
    let p = |size| LfsSide::Pointer(Pointer { oid: "a".repeat(64), size });
    LfsChange { old: p(10), new: p(20), new_in_workdir: false, filtered: false }
}

fn target(path: &str) -> DiffTarget {
    DiffTarget::Commit(Oid::zero(), path.into())
}

fn loaded(st: &mut AppState, target: DiffTarget, reload: bool, lfs: Option<LfsChange>) {
    let generation = st.diff_gen;
    run(st, Msg::Repo(RepoMsg::DiffLoaded { target, reload, generation, result: Ok(three_hunks()), lfs }));
}

#[test]
fn the_banner_shows_only_when_git_lfs_is_missing_and_patterns_exist() {
    let mut st = state();
    let dir = opened(&mut st, "lfs-notes-banner");
    for install in [LfsInstall::Unknown, LfsInstall::Checking, LfsInstall::Installed("3.7.1".into())] {
        st.lfs.install = install.clone();
        assert_eq!(banner_workdir(&st), None, "{install:?}");
    }
    st.lfs.install = LfsInstall::Missing;
    assert_eq!(banner_workdir(&st), Some(workdir(&st).as_path()));
    st.snap.as_mut().unwrap().lfs_patterns.clear();
    assert_eq!(banner_workdir(&st), None, "no patterns");
    cleanup(&[&dir]);
}

#[test]
fn dismissing_hides_the_banner_for_that_repository_only() {
    let mut st = state();
    st.lfs.install = LfsInstall::Missing;
    let a = opened(&mut st, "lfs-notes-dismiss-a");
    let wa = workdir(&st);
    run(&mut st, Msg::Lfs(LfsMsg::DismissBanner(wa.clone())));
    run(&mut st, Msg::Lfs(LfsMsg::DismissBanner(wa.clone())));
    assert_eq!(st.lfs.banner_dismissed.len(), 1, "dismissing twice keeps one entry");
    assert_eq!(banner_workdir(&st), None);
    let b = opened(&mut st, "lfs-notes-dismiss-b");
    assert_eq!(st.active_tab, 1);
    assert_eq!(banner_workdir(&st), Some(workdir(&st).as_path()), "another repository still qualifies");
    run(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)));
    assert_eq!(workdir(&st), wa);
    assert_eq!(banner_workdir(&st), None, "the dismissal survives a tab switch");
    cleanup(&[&a, &b]);
}

#[test]
fn a_status_with_a_version_turns_the_banner_off() {
    let mut st = state();
    st.lfs.install = LfsInstall::Missing;
    let dir = opened(&mut st, "lfs-notes-installed");
    assert!(banner_workdir(&st).is_some());
    run(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::RepoLfs)));
    let generation = st.prefs.as_ref().unwrap().lfs.generation;
    let status = LfsStatus { version: Some("3.7.1".into()), ..Default::default() };
    run(&mut st, Msg::Lfs(LfsMsg::Loaded { generation, result: Ok(status) }));
    assert_eq!(banner_workdir(&st), None);
    cleanup(&[&dir]);
}

#[test]
fn a_diff_with_an_lfs_side_keeps_its_note_and_pointer_choice() {
    let mut st = state();
    loaded(&mut st, target("art.psd"), false, Some(change()));
    assert_eq!(st.lfs.diff.as_ref().map(|n| &n.target), Some(&target("art.psd")));
    assert!(summary(&st).is_some() && !pointer_shown(&st));

    run(&mut st, Msg::Lfs(LfsMsg::ShowPointer(true)));
    assert!(pointer_shown(&st) && summary(&st).is_none());
    loaded(&mut st, target("art.psd"), true, Some(change()));
    assert!(pointer_shown(&st), "a reload of the same diff keeps the pointer text");

    // A superseded result changes nothing.
    let generation = st.diff_gen.wrapping_sub(1);
    let msg = RepoMsg::DiffLoaded { target: target("other.psd"), reload: false, generation, result: Ok(three_hunks()), lfs: None };
    run(&mut st, Msg::Repo(msg));
    assert!(pointer_shown(&st));

    loaded(&mut st, target("other.psd"), false, Some(change()));
    let note = st.lfs.diff.as_ref().unwrap();
    assert_eq!((&note.target, note.show_pointer), (&target("other.psd"), false), "another file starts on the summary");
    loaded(&mut st, target("plain.txt"), false, None);
    assert_eq!(st.lfs.diff, None, "a diff without LFS sides");
    assert!(summary(&st).is_none() && !pointer_shown(&st));
}

#[test]
fn showing_the_summary_clears_the_line_selection() {
    let mut st = state();
    loaded(&mut st, target("art.psd"), false, Some(change()));
    run(&mut st, Msg::Lfs(LfsMsg::ShowPointer(true)));
    let (t, d) = st.diff.clone().unwrap();
    st.view.diff_lines.click(&t, &d, 0, 1, LineClick::Only);
    assert!(!st.view.diff_lines.is_empty());
    run(&mut st, Msg::Lfs(LfsMsg::ShowPointer(false)));
    assert!(st.view.diff_lines.is_empty());
    assert!(summary(&st).is_some());
}

#[test]
fn the_note_is_parked_with_its_tab() {
    let mut st = state();
    let a = opened(&mut st, "lfs-notes-park-a");
    loaded(&mut st, DiffTarget::Unstaged("a.txt".into()), false, Some(change()));
    run(&mut st, Msg::Lfs(LfsMsg::ShowPointer(true)));
    let note = st.lfs.diff.clone();
    let b = opened(&mut st, "lfs-notes-park-b");
    assert_eq!(st.lfs.diff, None, "the new tab has no note");
    run(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)));
    assert_eq!(st.lfs.diff, note);
    assert!(pointer_shown(&st));
    cleanup(&[&a, &b]);
}

#[test]
fn next_change_has_no_stop_inside_a_summarised_file() {
    let mut st = state();
    loaded(&mut st, target("art.psd"), false, Some(change()));
    assert!(!matches!(diff_nav::step(&st, 1), Step::Row(_)), "the summary hides the rows");
    run(&mut st, Msg::Lfs(LfsMsg::ShowPointer(true)));
    assert!(matches!(diff_nav::step(&st, 1), Step::Row(_)), "the pointer text's changes are stops");
}
