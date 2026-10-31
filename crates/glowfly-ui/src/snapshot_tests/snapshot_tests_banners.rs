//! The shared banner style (`widgets/banner.rs`) in its kinds: git missing (an error), the saved
//! git gone with the git on PATH offered, git too old (a warning, light), the settings file that
//! couldn't be read (an error), and the operation strip while a job runs (progress, with its
//! spinner). The sign-in, Git LFS and older git banners in dark are in their own areas' tests.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use crate::git_env::GitEnvMsg;
use crate::msg::{Msg, PrefsMsg, RepoMsg};
use crate::state::git_env::Checking;
use crate::state::{AppState, RunningJob};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use git2::RepositoryState;
use glowfly_core::git_program::Candidate;
use glowfly_core::git_version::GitInfo;
use glowfly_core::settings::{LoadProblem, ThemeChoice};
use std::path::PathBuf;
use std::time::Instant;

type H = Harness<'static, (AppState, Vec<Msg>)>;

/// Clicks the button labelled `label` and returns what it pushed.
fn click(h: &mut H, label: &str) -> Vec<Msg> {
    h.state_mut().1.clear();
    h.get_by_label(label).click();
    h.run();
    std::mem::take(&mut h.state_mut().1)
}

fn detected(st: &mut AppState, info: GitInfo) {
    crate::update::update(st, Msg::GitEnv(GitEnvMsg::Detected { generation: 0, info }), Instant::now());
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
    assert!(h.query_by_label_contains("Use the git on PATH").is_none(), "nothing to offer");
    assert!(matches!(&click(&mut h, "Remind me later")[..], [Msg::GitEnv(GitEnvMsg::DismissBanner)]));
    for label in ["Don't show again", "How to update git"] {
        assert!(h.query_by_label(label).is_none(), "{label}: not while git can't run");
    }
    assert!(matches!(&click(&mut h, "Choose git…")[..], [Msg::GitEnv(GitEnvMsg::ShowPage)]));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn banner_git_path_fallback() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "banner-git-path-fallback");
    let saved = PathBuf::from("/opt/portable-git/bin/git");
    st.settings.git_program = Some(saved.to_string_lossy().into_owned());
    detected(&mut st, GitInfo { path: Some(saved), version: None, chosen: true });
    let path_git = Candidate { path: PathBuf::from("/usr/bin/git"), first_on_path: true, check: Ok((2, 45, 1)) };
    crate::update::update(&mut st, Msg::GitEnv(GitEnvMsg::PathGitChecked(Some(path_git))), Instant::now());
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("banner_git_path_fallback");
    let label = "Use the git on PATH (version 2.45.1)";
    assert!(h.query_by_label(label).is_some());
    assert!(matches!(&click(&mut h, label)[..], [Msg::GitEnv(GitEnvMsg::UsePathGit)]));
    assert!(matches!(&click(&mut h, "Remind me later")[..], [Msg::GitEnv(GitEnvMsg::DismissBanner)]));
    for label in ["Don't show again", "How to update git"] {
        assert!(h.query_by_label(label).is_none(), "{label}: not while git can't run");
    }
    assert!(matches!(&click(&mut h, "Choose git…")[..], [Msg::GitEnv(GitEnvMsg::ShowPage)]));
    h.state_mut().0.git.page.checking = Some(Checking { program: None, from_banner: true });
    h.run();
    assert!(click(&mut h, label).is_empty(), "disabled while the check runs");
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
    assert!(h.query_by_label_contains("Git 2.34.1 is older than 2.38").is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn banner_settings_unreadable() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "banner-settings-unreadable");
    let file = PathBuf::from("/home/me/.config/glowfly/settings.json");
    let detail = "could not read /home/me/.config/glowfly/settings.json: Permission denied (os error 13)".to_owned();
    let problem = LoadProblem { backup: None, reset: None, detail, locked: Some(file) };
    crate::update::update(&mut st, Msg::Prefs(PrefsMsg::SettingsUnreadable(problem)), Instant::now());
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("banner_settings_unreadable");
    assert!(h.query_by_label("Glowfly couldn't read its settings file").is_some());
    assert!(matches!(&click(&mut h, "Try again")[..], [Msg::Prefs(PrefsMsg::RetrySettingsRead)]));
    assert!(matches!(&click(&mut h, "Open settings folder")[..], [Msg::Prefs(PrefsMsg::OpenSettingsFolder)]));
    h.state_mut().0.settings_file.unreadable.as_mut().unwrap().reading = true;
    h.run();
    assert!(click(&mut h, "Try again").is_empty(), "disabled while a try runs");
    // Preferences doesn't save either, so it shows there too.
    h.state_mut().0.prefs = Some(crate::state::PrefsState::new(crate::state::PrefPage::General));
    h.run();
    assert!(h.query_by_label("Glowfly couldn't read its settings file").is_some(), "above Preferences");
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
