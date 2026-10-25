//! Headless tests for the Clone dialog's Your repositories tab: which accounts it offers, when it
//! loads, dropping stale answers, and filling the URL from a picked repository.

use super::tests::state;
use super::update;
use crate::effect::{CloneEffect, Effect, ForgeError};
use crate::msg::{CloneMsg, Msg};
use crate::state::clone::CloneForm;
use crate::state::clone_service::{CloneSource, ServicePicker, listing_accounts, visible};
use crate::state::{AppState, Modal};
use glowfly_core::forge::{Account, Connection, HostedRepo, MyRepos, Provider};
use std::time::Instant;

fn account(provider: Provider, host: &str) -> Account {
    let org = if provider == Provider::AzureDevOps { "org".into() } else { String::new() };
    Account { provider, host: host.into(), org }
}

fn connect(st: &mut AppState, accounts: &[Account]) {
    st.settings.connections =
        accounts.iter().map(|a| Connection { account: a.clone(), user: "ada".into(), method: "access token".into() }).collect();
}

fn run(st: &mut AppState, m: CloneMsg) -> Vec<Effect> {
    update(st, Msg::Clone(m), Instant::now())
}

fn form(st: &mut AppState) -> &mut CloneForm {
    match st.modal.as_mut() {
        Some(Modal::Clone(f)) => f,
        _ => panic!("no clone dialog"),
    }
}

fn picker(st: &mut AppState) -> &mut ServicePicker {
    &mut form(st).service
}

/// The accounts the effects ask for.
fn loads(fx: &[Effect]) -> Vec<Account> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Clone(CloneEffect::ListRepos { account, .. }) => Some(account.clone()),
            _ => None,
        })
        .collect()
}

fn repo(full: &str, ssh: &str) -> HostedRepo {
    HostedRepo {
        owner: full.split('/').next().unwrap().into(),
        full_name: full.into(),
        https_url: format!("https://github.com/{full}.git"),
        ssh_url: ssh.into(),
        web_url: String::new(),
        private: false,
    }
}

fn github() -> Account {
    account(Provider::GitHub, "github.com")
}

fn gitlab() -> Account {
    account(Provider::GitLab, "gitlab.com")
}

/// The dialog open on GitHub and GitLab accounts, the tab shown and GitHub's list answered.
fn loaded() -> AppState {
    let mut st = state();
    connect(&mut st, &[github(), gitlab()]);
    run(&mut st, CloneMsg::Dialog);
    run(&mut st, CloneMsg::Source(CloneSource::Service));
    let list = MyRepos { repos: vec![repo("ada/App", "git@github.com:ada/App.git"), repo("team/api", "")], truncated: false };
    run(&mut st, CloneMsg::ReposLoaded { account: github(), result: Box::new(Ok(list)) });
    st
}

#[test]
fn only_services_that_list_repositories_are_offered() {
    let azure = account(Provider::AzureDevOps, "dev.azure.com");
    let jira = account(Provider::JiraCloud, "x.atlassian.net");
    let bbdc = account(Provider::BitbucketDataCenter, "bb.corp.io");
    let ghe = account(Provider::GitHubEnterprise, "git.acme.io");
    let conns: Vec<Connection> = [gitlab(), azure, jira, ghe.clone(), github(), bbdc]
        .into_iter()
        .map(|account| Connection { account, user: "ada".into(), method: "token".into() })
        .collect();
    assert_eq!(listing_accounts(&conns), [github(), ghe, gitlab()], "github.com first, then in Preferences order");

    let mut st = state();
    connect(&mut st, &[gitlab(), account(Provider::AzureDevOps, "dev.azure.com"), github()]);
    let fx = run(&mut st, CloneMsg::Dialog);
    assert!(loads(&fx).is_empty(), "nothing loads until the tab shows");
    assert_eq!(picker(&mut st).accounts, [github(), gitlab()]);
}

#[test]
fn the_tab_loads_once_per_account() {
    let mut st = state();
    connect(&mut st, &[github(), gitlab()]);
    run(&mut st, CloneMsg::Dialog);
    let fx = run(&mut st, CloneMsg::Source(CloneSource::Service));
    assert_eq!(loads(&fx), [github()]);
    assert_eq!(form(&mut st).source, CloneSource::Service);
    assert!(picker(&mut st).loading);
    run(&mut st, CloneMsg::Source(CloneSource::Url));
    assert!(run(&mut st, CloneMsg::Source(CloneSource::Service)).is_empty(), "already loading");

    let fx = run(&mut st, CloneMsg::ServiceAccount(1));
    assert_eq!(loads(&fx), [gitlab()]);
    assert_eq!(picker(&mut st).account, 1);
    assert!(run(&mut st, CloneMsg::ServiceAccount(1)).is_empty(), "the same account");
    assert!(run(&mut st, CloneMsg::ServiceAccount(5)).is_empty(), "out of range");
    assert_eq!(picker(&mut st).account, 1);

    // No account to list: the tab shows how to connect one and asks nothing.
    let mut st = state();
    run(&mut st, CloneMsg::Dialog);
    assert!(run(&mut st, CloneMsg::Source(CloneSource::Service)).is_empty());
}

#[test]
fn stale_answers_are_dropped_and_errors_shown() {
    let mut st = state();
    connect(&mut st, &[github(), gitlab()]);
    run(&mut st, CloneMsg::Dialog);
    run(&mut st, CloneMsg::Source(CloneSource::Service));
    let one = || Box::new(Ok(MyRepos { repos: vec![repo("ada/x", "")], truncated: false }));
    // For another account than the one chosen.
    run(&mut st, CloneMsg::ReposLoaded { account: gitlab(), result: one() });
    assert!(picker(&mut st).loading && picker(&mut st).list.repos.is_empty());
    // A switch drops what the previous account's answer would have shown.
    run(&mut st, CloneMsg::ServiceAccount(1));
    run(&mut st, CloneMsg::ReposLoaded { account: github(), result: one() });
    assert!(picker(&mut st).list.repos.is_empty());
    let err = ForgeError { message: "Bad credentials".into(), auth: true };
    run(&mut st, CloneMsg::ReposLoaded { account: gitlab(), result: Box::new(Err(err.clone())) });
    assert!(!picker(&mut st).loading);
    assert_eq!(picker(&mut st).error, Some(err));
    let fx = run(&mut st, CloneMsg::RetryRepos);
    assert_eq!(loads(&fx), [gitlab()]);
    assert!(picker(&mut st).loading && picker(&mut st).error.is_none());

    // With no dialog open, an answer changes nothing.
    st.modal = None;
    assert!(run(&mut st, CloneMsg::ReposLoaded { account: gitlab(), result: one() }).is_empty());
    assert!(st.modal.is_none());
}

#[test]
fn picking_fills_the_url() {
    let mut st = loaded();
    assert_eq!(picker(&mut st).list.repos.len(), 2);
    run(&mut st, CloneMsg::PickRepo(0));
    assert_eq!(form(&mut st).url, "https://github.com/ada/App.git");
    assert_eq!(form(&mut st).name(), "App");
    run(&mut st, CloneMsg::UseSsh(true));
    assert_eq!(form(&mut st).url, "git@github.com:ada/App.git");
    // No SSH URL: the https one.
    run(&mut st, CloneMsg::PickRepo(1));
    assert_eq!(form(&mut st).url, "https://github.com/team/api.git");
    run(&mut st, CloneMsg::PickRepo(9));
    assert_eq!(picker(&mut st).picked, Some(1), "out of range is ignored");

    // A URL typed since is the user's: switching keeps it.
    run(&mut st, CloneMsg::PickRepo(0));
    form(&mut st).url = "https://example.com/mine.git".into();
    run(&mut st, CloneMsg::UseSsh(false));
    assert_eq!(form(&mut st).url, "https://example.com/mine.git");
    assert!(!picker(&mut st).ssh);
}

#[test]
fn the_filter_ignores_letter_case() {
    let mut st = loaded();
    picker(&mut st).filter = " APP ".into();
    assert_eq!(visible(picker(&mut st)), [0]);
    picker(&mut st).filter = "a".into();
    assert_eq!(visible(picker(&mut st)), [0, 1]);
    picker(&mut st).filter = "zzz".into();
    assert!(visible(picker(&mut st)).is_empty());
}

#[test]
fn signing_in_never_runs_in_the_active_repository() {
    // A held (untrusted) repository's own credential helper must not run: the list signs in
    // from a folder outside any repository, whichever tab is active.
    let (mut st, dir) = super::trust_held_tests::held_repo("clone-service-held");
    connect(&mut st, &[github()]);
    run(&mut st, CloneMsg::Dialog);
    let fx = run(&mut st, CloneMsg::Source(CloneSource::Service));
    let cwds: Vec<_> = fx
        .iter()
        .filter_map(|e| match e {
            Effect::Clone(CloneEffect::ListRepos { cwd, .. }) => Some(cwd.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(cwds.len(), 1, "the tab loads");
    assert!(!cwds[0].starts_with(&dir) && !super::tests::same_place(&cwds[0], &dir), "signed in from the held repository");
    assert!(git2::Repository::discover(&cwds[0]).is_err(), "{} is inside a repository", cwds[0].display());
}
