//! Headless tests for publishing a repository with no remote to GitHub. The GitHub calls are
//! answered by hand; the git side (adding `origin`, pushing) runs for real against a bare repo.

use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::{AccountEffect, Effect};
use crate::msg::JobMsg;
use crate::msg::Msg;
use crate::msg::PublishMsg;
use crate::msg::RepoMsg;
use crate::state::publish::PublishForm;
use crate::state::{AppState, Modal};
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::forge::{Account, Connection, CreatedRepo, NewRepo, Provider};

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn form(st: &AppState) -> &PublishForm {
    match &st.modal {
        Some(Modal::Publish(f)) => f,
        _ => panic!("the publish dialog is not open"),
    }
}

fn form_mut(st: &mut AppState) -> &mut PublishForm {
    match &mut st.modal {
        Some(Modal::Publish(f)) => f,
        _ => panic!("the publish dialog is not open"),
    }
}

fn github() -> Account {
    Account { provider: Provider::GitHub, host: "github.com".into(), org: String::new() }
}

/// A repository with no remote, opened, and a push attempted.
fn pushed_without_remote(name: &str) -> (AppState, PathBuf, Vec<Effect>) {
    let dir = scratch_repo(name);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let fx = run(&mut st, Msg::Job(JobMsg::Push { force: false }));
    (st, dir, fx)
}

#[test]
fn push_without_a_remote_offers_github() {
    let (mut st, dir, fx) = pushed_without_remote("publish-offer");
    let f = form(&st);
    assert_eq!((f.branch.as_str(), f.private, f.loading), ("main", true, true));
    assert!(f.name.starts_with("trellis-app-test-publish-offer"), "named after the folder: {}", f.name);
    assert_eq!(f.accounts, [github()], "github.com when nothing is connected");
    assert!(matches!(&fx[..], [Effect::Account(AccountEffect::Owners { account, .. })] if *account == github()));

    // Owners arrive: the user's login is selected and creation becomes possible.
    run(&mut st, Msg::Publish(PublishMsg::PublishOwners { account: github(), result: Ok(vec!["ada".into(), "acme".into()]) }));
    let f = form(&st);
    assert_eq!((f.user.as_deref(), f.owner.as_str(), f.loading), (Some("ada"), "ada", false));

    form_mut(&mut st).owner = "acme".into();
    form_mut(&mut st).name = "demo".into();
    form_mut(&mut st).description = "  A demo  ".into();
    let fx = run(&mut st, Msg::Publish(PublishMsg::Publish));
    let want = NewRepo { owner: "acme".into(), name: "demo".into(), description: "A demo".into(), private: true };
    assert!(
        matches!(&fx[..], [Effect::Account(AccountEffect::CreateRepo { user, repo, branch, .. })] if user == "ada" && *repo == want && branch == "main"),
        "{fx:?}"
    );
    assert!(form(&st).creating);
    assert!(run(&mut st, Msg::Publish(PublishMsg::Publish)).is_empty(), "only one create at a time");

    // A failure (e.g. the name is taken) stays in the dialog so the name can be changed.
    let workdir = form(&st).workdir.clone();
    run(&mut st, Msg::Publish(PublishMsg::Published { workdir, branch: "main".into(), result: Err("name already exists".into()) }));
    let f = form(&st);
    assert!(!f.creating && f.error.as_deref() == Some("name already exists"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn invalid_names_and_sign_in_failures_create_nothing() {
    let (mut st, dir, _) = pushed_without_remote("publish-invalid");
    assert!(run(&mut st, Msg::Publish(PublishMsg::Publish)).is_empty(), "owners not loaded yet");
    run(&mut st, Msg::Publish(PublishMsg::PublishOwners { account: github(), result: Ok(vec!["ada".into()]) }));
    form_mut(&mut st).name = "has space".into();
    assert!(run(&mut st, Msg::Publish(PublishMsg::Publish)).is_empty());

    // Owners for an account no longer selected are ignored; a sign-in failure is shown.
    let other = Account { provider: Provider::GitHubEnterprise, host: "git.acme.io".into(), org: String::new() };
    run(&mut st, Msg::Publish(PublishMsg::PublishOwners { account: other, result: Ok(vec!["x".into()]) }));
    assert_eq!(form(&st).owners, ["ada"]);
    run(&mut st, Msg::Publish(PublishMsg::PublishOwners { account: github(), result: Err("401".into()) }));
    assert!(form(&st).error.as_deref().is_some_and(|e| e.contains("401")));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn connected_accounts_are_offered_github_com_first() {
    let dir = scratch_repo("publish-accounts");
    let mut st = state();
    let ghe = Account { provider: Provider::GitHubEnterprise, host: "git.acme.io".into(), org: String::new() };
    let gitlab = Account { provider: Provider::GitLab, host: "gitlab.com".into(), org: String::new() };
    for a in [&ghe, &gitlab, &github()] {
        st.settings.connections.push(Connection { account: a.clone(), user: "ada".into(), method: "token".into() });
    }
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    run(&mut st, Msg::Publish(PublishMsg::PublishDialog));
    assert_eq!(form(&st).accounts, [github(), ghe.clone()], "GitHub accounts only");

    let fx = run(&mut st, Msg::Publish(PublishMsg::PublishAccount(1)));
    assert!(matches!(&fx[..], [Effect::Account(AccountEffect::Owners { account, .. })] if *account == ghe));
    assert!(form(&st).user.is_none() && form(&st).loading);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn created_repository_becomes_origin_and_is_pushed() {
    let (mut st, dir, _) = pushed_without_remote("publish-push");
    let bare = dir.with_extension("remote.git");
    let _ = std::fs::remove_dir_all(&bare);
    git(&dir, &["init", "-q", "--bare", &bare.to_string_lossy()]);

    let created = CreatedRepo {
        full_name: "ada/demo".into(),
        clone_url: bare.to_string_lossy().into_owned(),
        html_url: "https://github.com/ada/demo".into(),
    };
    drive(&mut st, Msg::Publish(PublishMsg::Published { workdir: dir.clone(), branch: "main".into(), result: Ok(created) }));
    assert!(st.modal.is_none(), "the dialog closes");
    assert!(st.log.iter().any(|l| l.text.contains("https://github.com/ada/demo")));
    let snap = st.snap.as_ref().unwrap();
    assert!(snap.remotes.iter().any(|(r, b)| r == "origin" && b.iter().any(|(n, _)| n == "main")), "{:?}", snap.remotes);
    assert!(snap.head_info().is_some_and(|b| b.upstream.is_some()), "main tracks origin/main");

    // With a remote now, Push pushes instead of offering to publish again.
    let fx = run(&mut st, Msg::Job(JobMsg::Push { force: false }));
    assert!(matches!(&fx[..], [Effect::RunGit { .. }]) && st.modal.is_none(), "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&bare);
}

#[test]
fn nothing_to_publish_without_commits() {
    let dir = trellis_core::paths::temp_dir().join(format!("trellis-app-test-publish-empty-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "-q", "-b", "main"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    run(&mut st, Msg::Publish(PublishMsg::PublishDialog));
    assert!(st.modal.is_none());
    assert!(st.log.last().is_some_and(|l| !l.ok && l.text.contains("Make a commit")));
    let _ = std::fs::remove_dir_all(&dir);
}
