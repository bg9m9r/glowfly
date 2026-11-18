//! Headless tests for the look for git-lfs on window focus (`update::lfs::window_focused`): it
//! runs only while git-lfs is missing or too old, at most once every `RECHECK_EVERY`, never sets
//! the Checking state, and its answer clears or changes the banner; and for the banner's two forms
//! (missing, too old) and their dismissals.

use super::tests::{scratch_repo, state};
use super::update;
use crate::effect::{Effect, LfsEffect};
use crate::lfs::LfsMsg;
use crate::msg::{Msg, RepoMsg};
use crate::state::lfs::{LfsInstall, LfsProblem, RECHECK_EVERY};
use crate::state::lfs_notes::{banner, banner_text};
use crate::state::{AppState, Tab};
use glowfly_core::i18n::Lang;
use glowfly_core::lfs::LfsPattern;
use glowfly_core::repo::{self, LoadOptions};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// A state showing the repository `name` with one LFS pattern.
fn lfs_repo(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    let mut st = state();
    st.tabs[0] = Tab::unloaded(dir.clone());
    let mut snap = repo::load(&dir, &LoadOptions::all(100)).unwrap();
    snap.lfs_patterns = vec![LfsPattern::root("*.psd", false)];
    st.snap = Some(snap);
    (st, dir)
}

fn detects(fx: &[Effect]) -> usize {
    fx.iter().filter(|e| matches!(e, Effect::Lfs(LfsEffect::Detect))).count()
}

fn focus(st: &mut AppState, at: Instant) -> usize {
    detects(&update(st, Msg::Repo(RepoMsg::WindowFocused), at))
}

fn detected(st: &mut AppState, v: Option<&str>, at: Instant) {
    update(st, Msg::Lfs(LfsMsg::Detected(v.map(str::to_owned))), at);
}

fn problem(st: &AppState) -> Option<LfsProblem> {
    banner(st).map(|b| b.problem)
}

#[test]
fn a_focus_regain_looks_again_and_the_banner_clears() {
    let (mut st, dir) = lfs_repo("lfs-focus-clears");
    st.lfs.install = LfsInstall::Missing;
    assert_eq!(problem(&st), Some(LfsProblem::Missing));
    let t0 = Instant::now();
    assert_eq!(focus(&mut st, t0), 1);
    assert_eq!(st.lfs.install, LfsInstall::Missing, "no Checking flip");
    assert!(banner(&st).is_some());
    detected(&mut st, Some("3.7.1"), t0);
    assert!(banner(&st).is_none());
    assert_eq!(st.lfs.install, LfsInstall::Installed("3.7.1".into()));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_look_on_focus_is_at_most_once_a_minute() {
    let mut st = state();
    st.lfs.install = LfsInstall::Missing;
    let t0 = Instant::now();
    assert_eq!(focus(&mut st, t0), 1);
    detected(&mut st, None, t0);
    assert_eq!(focus(&mut st, t0 + Duration::from_secs(59)), 0);
    assert_eq!(focus(&mut st, t0 + RECHECK_EVERY), 1);
    assert_eq!(focus(&mut st, t0 + RECHECK_EVERY + Duration::from_secs(1)), 0);

    let mut st = state();
    st.lfs.install = LfsInstall::Missing;
    update(&mut st, Msg::Lfs(LfsMsg::Detect), t0);
    detected(&mut st, None, t0);
    assert_eq!(focus(&mut st, t0 + Duration::from_secs(10)), 0, "the startup check counts");
}

#[test]
fn no_look_on_focus_when_nothing_is_wrong_or_known() {
    for install in [
        LfsInstall::Unknown,
        LfsInstall::Checking,
        LfsInstall::Installed("3.7.1".into()),
        LfsInstall::Installed("2.9.0".into()),
        LfsInstall::Installed("main".into()),
    ] {
        let mut st = state();
        st.lfs.install = install.clone();
        assert_eq!(focus(&mut st, Instant::now()), 0, "{install:?}");
        assert_eq!(st.lfs.install, install);
        assert!(st.lfs.checked_at.is_none(), "{install:?}");
    }
}

#[test]
fn a_too_old_git_lfs_gets_its_own_banner() {
    let (mut st, dir) = lfs_repo("lfs-too-old-banner");
    st.lfs.install = LfsInstall::Installed("2.3.4".into());
    assert_eq!(problem(&st), Some(LfsProblem::TooOld));
    let text = banner_text(Lang::En, &st.lfs.install);
    assert!(text.contains("git-lfs 2.3.4"), "{text}");
    assert!(text.contains("2.9.0 or newer"), "{text}");
    for fine in ["2.9.0", "3.7.1", "main"] {
        st.lfs.install = LfsInstall::Installed(fine.into());
        assert!(banner(&st).is_none(), "{fine}");
    }
    st.lfs.install = LfsInstall::Installed("2.3.4".into());
    let patterns = std::mem::take(&mut st.snap.as_mut().unwrap().lfs_patterns);
    assert!(banner(&st).is_none(), "no patterns");
    st.snap.as_mut().unwrap().lfs_patterns = patterns;
    let t0 = Instant::now();
    assert_eq!(focus(&mut st, t0), 1);
    detected(&mut st, Some("3.7.1"), t0);
    assert!(banner(&st).is_none());

    st.lfs.install = LfsInstall::Missing;
    detected(&mut st, Some("2.3.4"), t0);
    assert_eq!(problem(&st), Some(LfsProblem::TooOld), "the missing form turns into the too-old form");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn dismissing_one_form_keeps_the_other() {
    let (mut st, dir) = lfs_repo("lfs-dismiss-forms");
    let workdir = st.snap.as_ref().unwrap().workdir.clone();
    let now = Instant::now();
    let dismiss = |st: &mut AppState| {
        update(st, Msg::Lfs(LfsMsg::DismissBanner(workdir.clone())), now);
    };
    st.lfs.install = LfsInstall::Missing;
    dismiss(&mut st);
    assert!(banner(&st).is_none());
    detected(&mut st, Some("2.3.4"), now);
    assert_eq!(problem(&st), Some(LfsProblem::TooOld));
    dismiss(&mut st);
    assert!(banner(&st).is_none());
    assert_eq!(st.lfs.banner_dismissed.len(), 2);
    detected(&mut st, None, now);
    assert!(banner(&st).is_none());
    detected(&mut st, Some("3.7.1"), now);
    dismiss(&mut st);
    assert_eq!(st.lfs.banner_dismissed.len(), 2, "nothing to dismiss while git-lfs is fine");
    let _ = std::fs::remove_dir_all(dir);
}
