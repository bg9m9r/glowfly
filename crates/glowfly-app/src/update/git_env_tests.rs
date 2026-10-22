//! Headless tests for the git Glowfly runs (the startup check, the banner, features an old git
//! lacks) and for repositories git refuses because another user owns them.

use super::branch_tests::with_origin;
use super::tests::{drive, scratch_repo, state};
use super::update;
use crate::effect::{Effect, LoadKind, LocalOp};
use crate::git_env::{GitEnvEffect, GitEnvMsg};
use crate::msg::{JobMsg, Msg, RepoMsg, StageMsg};
use crate::state::{AppState, Modal};
use glowfly_core::git_version::GitInfo;
use std::path::PathBuf;
use std::time::Instant;

fn run(st: &mut AppState, m: GitEnvMsg) -> Vec<Effect> {
    update(st, Msg::GitEnv(m), Instant::now())
}

fn git_version(st: &mut AppState, v: (u32, u32, u32)) {
    run(st, GitEnvMsg::Detected { generation: 0, info: GitInfo { path: Some("/usr/bin/git".into()), version: Some(v), chosen: false } });
}

#[test]
fn startup_finds_git_and_an_old_one_shows_a_banner_until_dismissed() {
    let mut st = state();
    let fx = run(&mut st, GitEnvMsg::Detect);
    assert!(matches!(&fx[..], [Effect::GitEnv(GitEnvEffect::Detect { .. })]), "{fx:?}");
    assert_eq!(st.git.banner(), None, "nothing to say before the answer");
    git_version(&mut st, (2, 45, 0));
    assert_eq!(st.git.banner(), None);
    git_version(&mut st, (2, 34, 1));
    let banner = st.git.banner().expect("a banner");
    assert!(banner.contains("2.34.1") && banner.contains("2.38"), "{banner}");
    assert!(st.log.last().is_some_and(|l| !l.ok && l.text == banner), "logged too");
    run(&mut st, GitEnvMsg::DismissBanner);
    assert_eq!(st.git.banner(), None);
    for m in [GitEnvMsg::Detect, GitEnvMsg::DismissBanner, GitEnvMsg::Detected { generation: 0, info: GitInfo::default() }] {
        assert!(m.on_inactive().is_none());
    }
}

#[test]
fn discarding_needs_git_restore() {
    let dir = scratch_repo("git-env-discard");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    git_version(&mut st, (2, 22, 0));
    let fx = update(&mut st, Msg::Stage(StageMsg::DiscardFile("a.txt".into())), Instant::now());
    assert!(fx.is_empty(), "{fx:?}");
    let text = &st.toasts.last().expect("a toast").text;
    assert!(text.contains("needs git 2.23 or newer (you have 2.22.0)"), "{text}");
    git_version(&mut st, (2, 23, 0));
    let fx = update(&mut st, Msg::Stage(StageMsg::DiscardFile("a.txt".into())), Instant::now());
    assert!(matches!(&fx[..], [Effect::LocalOp { op: LocalOp::DiscardFile(p), .. }] if p == "a.txt"), "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn force_push_leaves_out_force_if_includes_for_an_old_git() {
    let (mut st, work, bare) = with_origin("git-env-force");
    super::tests::git(&work, &["commit", "-q", "--amend", "-m", "rewritten"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    git_version(&mut st, (2, 29, 2));
    let fx = update(&mut st, Msg::Job(JobMsg::Push { force: true }), Instant::now());
    match &fx[..] {
        [Effect::RunGit { cmds, .. }] => {
            assert!(cmds[0].iter().any(|a| a.starts_with("--force-with-lease=")), "{cmds:?}");
            assert!(!cmds[0].iter().any(|a| a == "--force-if-includes"), "{cmds:?}");
        }
        other => panic!("expected a push, got {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&work);
    let _ = std::fs::remove_dir_all(&bare);
}

#[test]
fn a_repository_owned_by_another_user_is_explained_and_can_be_trusted() {
    let mut st = state();
    let root = PathBuf::from("/srv/shared/app");
    let fx = update(&mut st, Msg::Repo(RepoMsg::Open(root.clone())), Instant::now());
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadRepo { .. })), "{fx:?}");
    let generation = st.load_gen;
    run(&mut st, GitEnvMsg::NotOwned { generation, kind: LoadKind::Open, root: root.clone() });
    assert!(!st.loading);
    let log = st.log.last().expect("a log line");
    assert!(!log.ok && log.text.contains("safe.directory") && !log.text.contains("not a git repository"), "{}", log.text);
    let Some(Modal::Confirm { body, then, .. }) = &st.modal else { panic!("expected the question") };
    assert!(body.contains("another user") && body.contains("safe.directory"), "{body}");
    assert!(matches!(&**then, Msg::GitEnv(GitEnvMsg::AddSafeDirectory(p)) if *p == root));

    let fx = run(&mut st, GitEnvMsg::AddSafeDirectory(root.clone()));
    assert!(matches!(&fx[..], [Effect::GitEnv(GitEnvEffect::AddSafeDirectory(p))] if *p == root), "{fx:?}");
    let fx = run(&mut st, GitEnvMsg::SafeDirectoryAdded { root: root.clone(), result: Ok(()) });
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadRepo { path, .. } if *path == root)), "opens again: {fx:?}");
    run(&mut st, GitEnvMsg::SafeDirectoryAdded { root: root.clone(), result: Err("locked".into()) });
    assert!(st.toasts.last().is_some_and(|t| !t.ok && t.text.contains("locked")));
}

#[test]
fn a_stale_owner_refusal_asks_nothing() {
    let mut st = state();
    let root = PathBuf::from("/srv/shared/app");
    update(&mut st, Msg::Repo(RepoMsg::Open(root.clone())), Instant::now());
    let stale = st.load_gen.wrapping_sub(1);
    run(&mut st, GitEnvMsg::NotOwned { generation: stale, kind: LoadKind::Open, root: root.clone() });
    assert!(st.modal.is_none());
    // For a tab that is no longer active, only the log line is kept.
    let inactive = GitEnvMsg::NotOwned { generation: 1, kind: LoadKind::Open, root }.on_inactive();
    assert!(matches!(inactive, Some(crate::msg::Inactive::Log(t)) if t.contains("safe.directory")));
}
