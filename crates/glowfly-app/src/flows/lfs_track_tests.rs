//! Headless tests for Track in Git LFS on file rows: the startup check and the page keeping it
//! current, the entries and why they are disabled, the job TrackPath starts or the reason it
//! refuses, and tracking an extension end to end (skipped without git-lfs).

use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::effect::LfsEffect;
use crate::lfs::LfsMsg;
use crate::msg::Msg;
use crate::msg::PrefsMsg;
use crate::msg::RepoMsg;
use crate::state::lfs::{LfsInstall, TrackContext, track_items as track_items_in};
use crate::state::{AppState, PrefPage};
use glowfly_core::gitcli::args;
use glowfly_core::i18n::Lang;
use glowfly_core::lfs::{LfsPattern, LfsStatus, TrackTarget, track_filename_args};
use glowfly_core::repo::{SubmoduleInfo, SubmoduleState};
use std::path::{Path, PathBuf};
use std::time::Instant;

const CHECKING: &str = "Checking whether Git LFS is installed…";
const NOT_INSTALLED_MENU: &str = "Git LFS isn't installed. See Preferences > Git LFS.";
const ONLY_THIS_FILE: &str = "Only this file";

/// The entries in English.
fn track_items(path: &str, cx: TrackContext) -> Vec<crate::state::lfs::TrackItem> {
    track_items_in(Lang::En, path, cx)
}

fn lfs(st: &mut AppState, m: LfsMsg) -> Vec<Effect> {
    update(st, Msg::Lfs(m), Instant::now())
}

fn open(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    assert!(st.snap.is_some());
    (st, dir)
}

fn cleanup(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

fn pattern(raw: &str) -> LfsPattern {
    LfsPattern::root(raw, false)
}

fn installed() -> LfsInstall {
    LfsInstall::Installed("3.7.1".into())
}

fn git_cmds(fx: &[Effect]) -> Option<Vec<Vec<String>>> {
    fx.iter().find_map(|e| match e {
        Effect::RunGit { cmds, .. } => Some(cmds.clone()),
        _ => None,
    })
}

fn last_log(st: &AppState) -> String {
    st.log.last().map(|l| l.text.clone()).unwrap_or_default()
}

fn status(version: Option<&str>) -> LfsStatus {
    LfsStatus { version: version.map(Into::into), ..Default::default() }
}

#[test]
fn detect_and_page_refresh() {
    let mut st = state();
    let fx = lfs(&mut st, LfsMsg::Detect);
    assert_eq!(st.lfs.install, LfsInstall::Checking);
    assert!(matches!(&fx[..], [Effect::Lfs(LfsEffect::Detect)]), "{fx:?}");
    lfs(&mut st, LfsMsg::Detected(Some("3.7.1".into())));
    assert_eq!(st.lfs.install, installed());
    lfs(&mut st, LfsMsg::Detected(None));
    assert_eq!(st.lfs.install, LfsInstall::Missing);

    let (mut st, dir) = open("lfs-track-detect");
    update(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::RepoLfs)), Instant::now());
    let generation = st.prefs.as_ref().unwrap().lfs.generation;
    lfs(&mut st, LfsMsg::Loaded { generation, result: Ok(status(Some("3.7.1"))) });
    assert_eq!(st.lfs.install, installed());
    lfs(&mut st, LfsMsg::Reload);
    let generation = st.prefs.as_ref().unwrap().lfs.generation;
    lfs(&mut st, LfsMsg::Loaded { generation, result: Ok(status(None)) });
    assert_eq!(st.lfs.install, LfsInstall::Missing);
    lfs(&mut st, LfsMsg::Loaded { generation: generation - 1, result: Ok(status(Some("3.7.1"))) });
    assert_eq!(st.lfs.install, LfsInstall::Missing, "a stale read changes nothing");
    lfs(&mut st, LfsMsg::Loaded { generation, result: Err("boom".into()) });
    assert_eq!(st.lfs.install, LfsInstall::Missing, "a failed read changes nothing");
    cleanup(&dir);
}

fn submodule(path: &str) -> SubmoduleInfo {
    SubmoduleInfo {
        name: path.into(),
        path: path.into(),
        url: None,
        recorded: git2::Oid::zero(),
        checked_out: None,
        state: SubmoduleState::InSync,
        left_behind: vec![],
        outside_sparse: false,
        clone_url: None,
        branch: None,
        added: false,
    }
}

#[test]
fn track_items_reasons() {
    let psd = [pattern("*.psd")];
    fn cx<'a>(install: &'a LfsInstall, patterns: &'a [LfsPattern]) -> TrackContext<'a> {
        TrackContext { install, patterns, submodules: &[] }
    }
    for (install, why) in [(LfsInstall::Unknown, CHECKING), (LfsInstall::Checking, CHECKING), (LfsInstall::Missing, NOT_INSTALLED_MENU)] {
        let items = track_items("assets/a.psd", cx(&install, &psd));
        assert_eq!(items.len(), 3);
        assert!(items.iter().all(|i| i.blocked.as_deref() == Some(why)), "{install:?}");
    }

    let ok = installed();
    let items = track_items("assets/a.psd", cx(&ok, &psd));
    let labels: Vec<_> = items.iter().map(|i| i.label.as_str()).collect();
    assert_eq!(labels, [ONLY_THIS_FILE, "Every .psd file", "Everything in assets/"]);
    let blocked: Vec<_> = items.iter().map(|i| i.blocked.as_deref()).collect();
    assert_eq!(blocked, [None, Some("*.psd is already tracked."), None]);
    assert_eq!(items[0].hover, "Adds assets/a.psd to .gitattributes");
    assert_eq!(items[1].hover, "Adds *.psd to .gitattributes");
    assert_eq!(items[2].hover, "Adds assets/** to .gitattributes (subfolders included)");

    assert_eq!(track_items("Makefile", cx(&ok, &psd)).len(), 1);
    let git_file = track_items("assets/.gitattributes", cx(&ok, &psd));
    assert_eq!(git_file.len(), 2, "no extension: just the file and its folder");
    assert_eq!(git_file[0].blocked, Some(glowfly_core::lfs::PatternProblem::GitFile.message(Lang::En)));
    assert_eq!(git_file[1].blocked, None);

    let subs = [submodule("libs/core")];
    assert!(track_items("libs/core", TrackContext { install: &ok, patterns: &psd, submodules: &subs }).is_empty());

    let st = state();
    let cx = TrackContext::of(&st);
    assert!(cx.patterns.is_empty() && cx.submodules.is_empty() && *cx.install == LfsInstall::Unknown);
}

#[test]
fn track_path_starts_a_job() {
    let (mut st, dir) = open("lfs-track-path");
    st.lfs.install = installed();
    let fx = lfs(&mut st, LfsMsg::TrackPath(TrackTarget::Folder("assets/**".into())));
    assert_eq!(git_cmds(&fx), Some(vec![args(&["lfs", "track", "--", "assets/**"])]));
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some("Track assets/** in LFS"));
    let fx = lfs(&mut st, LfsMsg::TrackPath(TrackTarget::Extension("*.bin".into())));
    assert!(git_cmds(&fx).is_none(), "one job at a time");
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some("Track assets/** in LFS"));
    cleanup(&dir);

    let (mut st, dir) = open("lfs-track-path-file");
    st.lfs.install = installed();
    let fx = lfs(&mut st, LfsMsg::TrackPath(TrackTarget::File("a b.bin".into())));
    assert_eq!(git_cmds(&fx), Some(vec![args(&["lfs", "track", "--filename", "--", "a b.bin"])]));
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some("Track a b.bin in LFS"));
    cleanup(&dir);
}

#[test]
fn track_path_refusals() {
    let (mut st, dir) = open("lfs-track-refuse");
    let psd = || LfsMsg::TrackPath(TrackTarget::Extension("*.psd".into()));
    st.lfs.install = LfsInstall::Missing;
    assert!(git_cmds(&lfs(&mut st, psd())).is_none());
    assert_eq!(last_log(&st), "Git LFS isn't installed.");
    for install in [LfsInstall::Checking, LfsInstall::Unknown] {
        st.lfs.install = install;
        assert!(git_cmds(&lfs(&mut st, psd())).is_none());
        assert_eq!(last_log(&st), "Still checking whether Git LFS is installed.");
    }
    st.lfs.install = installed();
    st.snap.as_mut().unwrap().lfs_patterns = vec![pattern("*.psd")];
    assert!(git_cmds(&lfs(&mut st, psd())).is_none());
    assert_eq!(last_log(&st), "*.psd is already tracked.");
    assert!(st.job.is_none());

    let mut empty = state();
    empty.lfs.install = installed();
    assert!(lfs(&mut empty, psd()).is_empty(), "no repository open");
    cleanup(&dir);
}

#[test]
fn track_path_end_to_end() {
    if glowfly_core::lfs::installed().is_none() {
        eprintln!("track_path_end_to_end: git-lfs is not installed, skipping");
        return;
    }
    let dir = scratch_repo("lfs-track-e2e");
    std::fs::create_dir_all(dir.join("art")).unwrap();
    std::fs::write(dir.join("art/a.psd"), "pixels\n").unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "art"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Lfs(LfsMsg::Detect));
    assert!(matches!(st.lfs.install, LfsInstall::Installed(_)), "{:?}", st.lfs.install);
    assert!(track_items("art/a.psd", TrackContext::of(&st))[1].blocked.is_none());

    drive(&mut st, Msg::Lfs(LfsMsg::TrackPath(TrackTarget::Extension("*.psd".into()))));
    assert!(
        st.toasts.iter().any(|t| t.text == "Track *.psd in LFS succeeded"),
        "{:?}",
        st.toasts.iter().map(|t| &t.text).collect::<Vec<_>>()
    );
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(snap.lfs_patterns.iter().map(|p| p.raw.as_str()).collect::<Vec<_>>(), ["*.psd"]);
    assert!(snap.unstaged.iter().any(|f| f.path == ".gitattributes"), "{:?}", snap.unstaged.iter().map(|f| &f.path).collect::<Vec<_>>());
    let items = track_items("art/a.psd", TrackContext::of(&st));
    assert_eq!(items[1].blocked.as_deref(), Some("*.psd is already tracked."));
    cleanup(&dir);
}

#[test]
fn tracking_a_file_keeps_no_undo_history() {
    assert!(!super::undo::keeps_history(&[track_filename_args("a.bin")]));
}
