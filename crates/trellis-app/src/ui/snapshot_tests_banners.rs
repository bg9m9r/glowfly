//! The shared banner style (`widgets/banner.rs`) in its kinds: git missing (an error), git too
//! old (a warning, light), and the operation strip while a job runs (progress, with its
//! spinner). The sign-in, Git LFS and older git banners in dark are in their own areas' tests.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use crate::git_env::GitEnvMsg;
use crate::msg::{Msg, RepoMsg};
use crate::state::{AppState, RunningJob};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use git2::RepositoryState;
use std::time::Instant;
use trellis_core::git_version::GitInfo;
use trellis_core::settings::ThemeChoice;

type H = Harness<'static, (AppState, Vec<Msg>)>;

/// Clicks the button labelled `label` and returns what it pushed.
fn click(h: &mut H, label: &str) -> Vec<Msg> {
    h.state_mut().1.clear();
    h.get_by_label(label).click();
    h.run();
    std::mem::take(&mut h.state_mut().1)
}

fn detected(st: &mut AppState, info: GitInfo) {
    crate::update::update(st, Msg::GitEnv(GitEnvMsg::Detected(info)), Instant::now());
}

#[test]
fn banner_git_missing() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "banner-git-missing");
    detected(&mut st, GitInfo { path: None, version: None, chosen: false });
    assert!(st.git.git_unusable());
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("banner_git_missing");
    assert!(h.query_by_label_contains("couldn't find git on your PATH").is_some());
    assert!(matches!(&click(&mut h, "Dismiss")[..], [Msg::GitEnv(GitEnvMsg::DismissBanner)]));
    assert!(matches!(&click(&mut h, "Choose git…")[..], [Msg::GitEnv(GitEnvMsg::ShowPage)]));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn banner_git_too_old_light() {
    let (mut st, dir) = with_repo(ThemeChoice::Light, "banner-git-old");
    detected(&mut st, GitInfo { path: Some("/usr/bin/git".into()), version: Some((2, 34, 1)), chosen: false });
    assert!(!st.git.git_unusable());
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("banner_git_too_old_light");
    assert!(h.query_by_label_contains("Your git is 2.34.1").is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn op_strip_progress() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "op-strip-progress");
    st.snap.as_mut().unwrap().state = RepositoryState::CherryPick;
    st.job = Some(RunningJob {
        label: "Cherry-pick".into(),
        workdir: st.workdir(),
        quiet: false,
        clear_commit: false,
        then_open: None,
        then_push: false,
        then_pr: None,
        then_link: false,
        keeps_undo: false,
        fetches: false,
    });
    st.settings.show_details_panel = false;
    // The spinners keep repainting: a fixed number of frames keeps the image reproducible.
    let mut h = harness(st, true);
    h.run_steps(4);
    h.check_snapshot("op_strip_progress");
    h.state_mut().1.clear();
    h.get_by_label("Show work in progress").click();
    h.run_steps(2);
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Repo(RepoMsg::ShowInProgress))));
    drop(h);
    remove_fixture(&dir);
}
