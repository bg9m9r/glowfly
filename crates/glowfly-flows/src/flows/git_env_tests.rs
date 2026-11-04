//! Headless tests for the git Glowfly runs (the startup check, the banner, features an old git
//! lacks) and for repositories git refuses because another user owns them.

use super::branch_tests::with_origin;
use super::tests::{drive, scratch_repo, state};
use super::update;
use glowfly_core::git_version::GitInfo;
use glowfly_core::safe_directory::Refusal;
use glowfly_model::effect::{Effect, LoadKind, LocalOp};
use glowfly_model::git_env::{GitEnvEffect, GitEnvMsg};
use glowfly_model::msg::{JobMsg, Msg, RepoMsg, StageMsg};
use glowfly_model::state::{AppState, Modal};
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
    assert_eq!(st.git_banner(), None, "nothing to say before the answer");
    git_version(&mut st, (2, 45, 0));
    assert_eq!(st.git_banner(), None);
    git_version(&mut st, (2, 34, 1));
    let banner = st.git_banner().expect("a banner");
    assert!(banner.contains("2.34.1") && banner.contains("2.38"), "{banner}");
    let problem = st.git.info.as_ref().and_then(GitInfo::problem).unwrap();
    assert!(st.log.last().is_some_and(|l| !l.ok && l.text == problem), "logged in full");
    run(&mut st, GitEnvMsg::DismissBanner);
    assert_eq!(st.git_banner(), None);
    for m in [
        GitEnvMsg::Detect,
        GitEnvMsg::DismissBanner,
        GitEnvMsg::HideBannerForGood,
        GitEnvMsg::Detected { generation: 0, info: GitInfo::default() },
    ] {
        assert!(m.on_inactive().is_none());
    }
}

#[test]
fn remind_me_later_is_for_the_session_and_dont_show_again_saves_the_version() {
    let mut st = state();
    git_version(&mut st, (2, 34, 1));
    let banner = st.git_banner().expect("a banner");
    assert!(banner.starts_with("Git 2.34.1 is older than 2.38"), "{banner}");
    assert!(
        st.log.last().is_some_and(|l| l.text.starts_with("Your git is 2.34.1;") && l.text.contains("these are off:")),
        "full text logged"
    );

    let fx = run(&mut st, GitEnvMsg::HideBannerForGood);
    assert_eq!(st.settings.git_banner_hidden_for.as_deref(), Some("2.34.1"));
    assert!(fx.iter().any(|e| matches!(e, Effect::SaveSettings(s) if s.git_banner_hidden_for.as_deref() == Some("2.34.1"))), "{fx:?}");
    assert_eq!(st.git_banner(), None);
    assert!(!st.git.banner_dismissed, "hidden through the setting");
    let log = st.log.last().expect("a log line");
    assert!(log.ok && log.text.contains("won't show again for this version"), "{}", log.text);
    assert!(!st.git.has(glowfly_core::git_version::Feature::MergeTree), "features stay off");

    // A restart with the same settings and the same git.
    let mut again = AppState::new(st.settings.clone(), Vec::new(), Instant::now());
    git_version(&mut again, (2, 34, 1));
    assert_eq!(again.git_banner(), None);
    // Another (still too old) git shows it again.
    git_version(&mut again, (2, 37, 7));
    assert!(again.git_banner().is_some_and(|b| b.starts_with("Git 2.37.7 is older")));

    // Remind me later only hides it until a restart.
    run(&mut again, GitEnvMsg::DismissBanner);
    assert!(again.git.banner_dismissed && again.git_banner().is_none());
    assert_eq!(again.settings.git_banner_hidden_for.as_deref(), Some("2.34.1"), "unchanged");
    let mut restarted = AppState::new(again.settings.clone(), Vec::new(), Instant::now());
    git_version(&mut restarted, (2, 37, 7));
    assert!(restarted.git_banner().is_some());
}

#[test]
fn dont_show_again_never_hides_a_git_that_cant_run() {
    let mut st = state();
    run(&mut st, GitEnvMsg::Detected { generation: 0, info: GitInfo::default() });
    let before = st.git_banner().expect("the can't-run banner");
    assert!(run(&mut st, GitEnvMsg::HideBannerForGood).is_empty());
    assert_eq!(st.settings.git_banner_hidden_for, None);
    assert_eq!(st.git_banner(), Some(before.clone()));

    let mut fine = state();
    git_version(&mut fine, (2, 45, 0));
    assert!(run(&mut fine, GitEnvMsg::HideBannerForGood).is_empty(), "nothing is missing");
    assert_eq!(fine.settings.git_banner_hidden_for, None);

    // A saved version never hides the banner of a git that can't run.
    let mut hidden = AppState::new(
        glowfly_core::settings::Settings { git_banner_hidden_for: Some("2.34.1".into()), ..Default::default() },
        Vec::new(),
        Instant::now(),
    );
    run(&mut hidden, GitEnvMsg::Detected { generation: 0, info: GitInfo::default() });
    assert_eq!(hidden.git_banner(), Some(before));
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
    // feature: main, the default branch, is never force pushed.
    super::tests::git(&work, &["switch", "-q", "feature"]);
    super::tests::git(&work, &["commit", "-q", "--amend", "-m", "rewritten"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    git_version(&mut st, (2, 29, 2));
    let fx = update(&mut st, Msg::Job(JobMsg::Push { force: true }), Instant::now());
    match &fx[..] {
        [Effect::Job(glowfly_model::effect::JobEffect::RunGit { cmds, .. })] => {
            assert!(cmds[0].iter().any(|a| a.starts_with("--force-with-lease=")), "{cmds:?}");
            assert!(!cmds[0].iter().any(|a| a == "--force-if-includes"), "{cmds:?}");
        }
        other => panic!("expected a push, got {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&work);
    let _ = std::fs::remove_dir_all(&bare);
}

/// A refusal of `root`, owned by `owner` (`admins`: the Administrators group).
fn refusal(root: &std::path::Path, owner: Option<&str>, admins: bool) -> Refusal {
    Refusal { root: root.to_path_buf(), owner: owner.map(str::to_owned), admins }
}

#[test]
fn a_repository_owned_by_another_user_is_explained_and_can_be_trusted() {
    let mut st = state();
    let root = PathBuf::from("/srv/shared/app");
    let fx = update(&mut st, Msg::Repo(RepoMsg::Open(root.clone())), Instant::now());
    assert!(fx.iter().any(|e| matches!(e, Effect::Load(glowfly_model::effect::LoadEffect::Repo { .. }))), "{fx:?}");
    let generation = st.load_gen;
    run(&mut st, GitEnvMsg::NotOwned { generation, kind: LoadKind::Open, refusal: refusal(&root, Some(r"OTHERPC\alice"), false) });
    assert!(!st.loading);
    let log = st.log.last().expect("a log line");
    assert!(!log.ok && log.text.contains(r"OTHERPC\alice") && !log.text.contains("safe.directory"), "{}", log.text);
    assert!(st.logs.announce.is_none() && st.toasts.is_empty(), "the dialog explains; no toast repeats it");
    let value = glowfly_core::safe_directory::config_value(&root, cfg!(windows));
    let Some(Modal::ConfirmDetails { title, body, details, button, then }) = &st.modal else { panic!("expected the question") };
    assert_eq!((title.as_str(), button.as_str()), ("Trust this folder?", "Trust this folder"));
    assert!(body.contains(r"belongs to OTHERPC\alice, not to your account") && body.contains("until you trust it"), "{body}");
    assert!(!body.contains("safe.directory") && !body.contains("git config"), "plain words only: {body}");
    // On Windows the config value uses forward slashes while the body shows the folder the
    // native way; elsewhere the two spellings are the same, so only Windows can tell them apart.
    if cfg!(windows) {
        assert!(!body.contains(&value), "the body names the folder, not the config value: {body}");
    }
    assert!(details.contains("network share") && details.contains("removable drive"), "the common causes: {details}");
    assert!(details.contains("git config --global --add safe.directory") && details.contains(&value), "{details}");
    assert!(matches!(&**then, Msg::GitEnv(GitEnvMsg::AddSafeDirectory(p)) if *p == root));
    let fx = update(&mut st, Msg::ConfirmAccepted, Instant::now());
    assert!(matches!(&fx[..], [Effect::GitEnv(GitEnvEffect::AddSafeDirectory(p))] if *p == root), "{fx:?}");
    assert!(st.modal.is_none());

    let fx = run(&mut st, GitEnvMsg::AddSafeDirectory(root.clone()));
    assert!(matches!(&fx[..], [Effect::GitEnv(GitEnvEffect::AddSafeDirectory(p))] if *p == root), "{fx:?}");
    let fx = run(&mut st, GitEnvMsg::SafeDirectoryAdded { root: root.clone(), result: Ok(()) });
    assert!(
        fx.iter().any(|e| matches!(e, Effect::Load(glowfly_model::effect::LoadEffect::Repo { path, .. }) if *path == root)),
        "opens again: {fx:?}"
    );
    assert!(st.log.iter().any(|l| l.ok && l.text.starts_with("Trusted ")), "logged");
    run(&mut st, GitEnvMsg::SafeDirectoryAdded { root: root.clone(), result: Err("locked\n".into()) });
    assert!(st.toasts.last().is_some_and(|t| !t.ok && t.text.ends_with("Git said: locked")), "{:?}", st.toasts.last().map(|t| &t.text));
}

#[test]
fn the_question_names_the_owner_as_plainly_as_it_can() {
    let root = PathBuf::from("/srv/shared/app");
    let body = |r: Refusal| {
        let mut st = state();
        update(&mut st, Msg::Repo(RepoMsg::Open(root.clone())), Instant::now());
        let generation = st.load_gen;
        run(&mut st, GitEnvMsg::NotOwned { generation, kind: LoadKind::Open, refusal: r });
        let Some(Modal::ConfirmDetails { body, .. }) = st.modal else { panic!("expected the question") };
        (body, st.log.last().map(|l| l.text.clone()).unwrap_or_default())
    };
    let (admins, log) = body(refusal(&root, Some(r"BUILTIN\Administrators"), true));
    assert!(admins.contains("belongs to the Administrators group") && admins.contains("run as administrator"), "{admins}");
    assert!(!admins.contains("BUILTIN"), "{admins}");
    assert!(log.contains("the Administrators group"), "{log}");
    let (unknown, log) = body(refusal(&root, None, false));
    assert!(unknown.contains("belongs to another user account, not to yours"), "{unknown}");
    assert!(log.contains("another user account"), "{log}");
}

#[test]
fn a_stale_owner_refusal_asks_nothing() {
    let mut st = state();
    let root = PathBuf::from("/srv/shared/app");
    update(&mut st, Msg::Repo(RepoMsg::Open(root.clone())), Instant::now());
    let stale = st.load_gen.wrapping_sub(1);
    run(&mut st, GitEnvMsg::NotOwned { generation: stale, kind: LoadKind::Open, refusal: refusal(&root, None, false) });
    assert!(st.modal.is_none());
    // For a tab that is no longer active, only the log line is kept.
    let inactive = GitEnvMsg::NotOwned { generation: 1, kind: LoadKind::Open, refusal: refusal(&root, None, false) }.on_inactive();
    let shown = glowfly_core::native_path::shown(&root);
    assert!(matches!(inactive, Some(glowfly_model::msg::Inactive::Log(t)) if t.contains(&shown)));
}

#[test]
fn the_daemon_probe_follows_each_check() {
    let mut st = state();
    let info = GitInfo { path: Some("/usr/bin/git".into()), version: Some((2, 45, 0)), chosen: false };
    let probes =
        |fx: &[Effect]| fx.iter().filter(|e| matches!(e, Effect::GitEnv(GitEnvEffect::ProbeMonitor { generation: 0, .. }))).count();
    let fx = run(&mut st, GitEnvMsg::Detected { generation: 0, info: info.clone() });
    assert_eq!(probes(&fx), usize::from(!cfg!(target_os = "linux")), "Linux isn't offered the daemon: {fx:?}");
    run(&mut st, GitEnvMsg::MonitorProbed { generation: 0, daemon: Some(false) });
    assert_eq!(st.git.monitor_daemon, Some(false));
    // An answer about the git used before a switch is dropped.
    run(&mut st, GitEnvMsg::MonitorProbed { generation: 7, daemon: Some(true) });
    assert_eq!(st.git.monitor_daemon, Some(false));
    // A git that can't run isn't asked, and the last answer is forgotten.
    let fx = run(&mut st, GitEnvMsg::Detected { generation: 0, info: GitInfo { version: None, ..info } });
    assert_eq!(probes(&fx), 0, "{fx:?}");
    assert_eq!(st.git.monitor_daemon, None);
}
