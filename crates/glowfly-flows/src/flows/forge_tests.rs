//! Which hosting services the app contacts, and with which credentials.

use super::tests::{drive, git, scratch_repo, state};
use super::update;
use glowfly_core::forge::{self, Account, Auth, Connection, HELPER_SOURCE, Provider, Target};
use glowfly_core::i18n::Lang;
use glowfly_model::effect::NetworkEffect;
use glowfly_model::effect::SaveEffect;
use glowfly_model::effect::{Effect, ForgeCall, ForgeError, ForgeReply, ForgeResult, FoundAuth};
use glowfly_model::msg::ForgeMsg;
use glowfly_model::msg::{Msg, RepoMsg};
use glowfly_model::state::forge_filters::connect_prompt;
use glowfly_model::state::{AppState, ForgeState, ProjectKeys, Status};
use std::time::Instant;

fn target(provider: Provider, host: &str) -> Target {
    Target { provider, host: host.into(), owner: "o".into(), project: String::new(), repo: "r".into() }
}

fn with_host(t: &Target) -> AppState {
    let mut st = state();
    st.session.forge = Some(ForgeState::new(Some(t.clone()), "origin".into(), Some(t.clone())));
    st
}

/// The `connected` flag of each forge call `msg` starts.
fn forge_calls(st: &mut AppState, msg: Msg) -> Vec<bool> {
    update(st, msg, Instant::now())
        .into_iter()
        .filter_map(|e| match e {
            Effect::Network(NetworkEffect::Forge { connected, .. }) => Some(connected),
            _ => None,
        })
        .collect()
}

#[test]
fn guessed_self_hosted_servers_are_never_contacted() {
    for t in [target(Provider::GitHubEnterprise, "github.evil.example"), target(Provider::GitLabSelfManaged, "gitlab.evil.example")] {
        let mut st = with_host(&t);
        for interactive in [false, true] {
            assert!(forge_calls(&mut st, Msg::Forge(ForgeMsg::ForgeRefresh { interactive })).is_empty(), "{t:?}");
        }
        let fs = st.session.forge.as_ref().unwrap();
        assert!(matches!(&fs.host_status, Status::Error(e) if e.contains("not connected")), "{:?}", fs.host_status);
        assert_eq!(fs.loading, 0);
    }
}

#[test]
fn connected_self_hosted_servers_and_public_hosts_are_contacted() {
    let ghe = target(Provider::GitHubEnterprise, "github.acme.io");
    let mut st = with_host(&ghe);
    st.settings.connections.push(Connection { account: ghe.account(), user: "ada".into(), method: "access token".into() });
    assert_eq!(forge_calls(&mut st, Msg::Forge(ForgeMsg::ForgeRefresh { interactive: false })), [true]);

    let mut st = with_host(&target(Provider::GitHub, "github.com"));
    assert_eq!(forge_calls(&mut st, Msg::Forge(ForgeMsg::ForgeRefresh { interactive: false })), [false], "public, not connected");
}

/// A GitHub host beside a connected Jira Cloud tracker with project filter `project`.
fn with_jira(project: &str) -> AppState {
    let jira = Account { provider: Provider::JiraCloud, host: "acme.atlassian.net".into(), org: String::new() };
    let mut st = state();
    st.settings.connections.push(Connection { account: jira.clone(), user: "ada".into(), method: "access token".into() });
    st.session.forge =
        Some(ForgeState::new(Some(target(Provider::GitHub, "github.com")), "origin".into(), Some(Target::jira(&jira, project))));
    st
}

/// Whether the tracker refresh also asks for the site's project keys.
fn asks_project_keys(st: &mut AppState) -> Option<bool> {
    update(st, Msg::Forge(ForgeMsg::ForgeRefresh { interactive: false }), Instant::now()).into_iter().find_map(|e| match e {
        Effect::Network(NetworkEffect::Forge { call: ForgeCall::MyIssues { project_keys }, .. }) => Some(project_keys),
        _ => None,
    })
}

fn project_keys_reply(st: &mut AppState, result: Result<Option<Vec<String>>, ForgeError>) {
    let target = st.session.forge.as_ref().unwrap().tracker.clone().unwrap();
    let reply = ForgeReply { target, results: vec![ForgeResult::ProjectKeys(result)] };
    update(st, Msg::Forge(ForgeMsg::ForgeReplied(reply)), Instant::now());
}

fn refs(st: &AppState, text: &str) -> Vec<String> {
    st.session.forge.as_ref().unwrap().issue_refs(text)
}

#[test]
fn jira_keys_are_checked_against_the_sites_projects() {
    let mut st = with_jira("");
    assert_eq!(asks_project_keys(&mut st), Some(true));
    // Until the projects are known, words that are never projects are skipped.
    assert_eq!(refs(&st, "Fix UTF-8 in PROJ-12"), ["PROJ-12"]);

    project_keys_reply(&mut st, Ok(Some(vec!["ABC".into()])));
    assert_eq!(refs(&st, "PROJ-12 ABC-3"), ["ABC-3"]);
    assert_eq!(st.session.forge.as_ref().unwrap().branch_issue("feature/UTF-8-fix"), None);
    assert_eq!(st.session.forge.as_ref().unwrap().branch_issue("feature/ABC-3-fix").as_deref(), Some("ABC-3"));
    assert_eq!(asks_project_keys(&mut st), Some(false));

    // Too many projects to list: the word list applies, and they aren't asked for again.
    let mut st = with_jira("");
    project_keys_reply(&mut st, Ok(None));
    assert_eq!(st.session.forge.as_ref().unwrap().jira_projects, ProjectKeys::TooMany);
    assert_eq!(refs(&st, "UTF-8 and PROJ-12"), ["PROJ-12"]);
    assert_eq!(asks_project_keys(&mut st), Some(false));

    // A failed read is tried again on the next refresh.
    let mut st = with_jira("");
    project_keys_reply(&mut st, Err(ForgeError { message: "boom".into(), auth: false }));
    assert_eq!(st.session.forge.as_ref().unwrap().jira_projects, ProjectKeys::Unknown);
    assert_eq!(asks_project_keys(&mut st), Some(true));
}

#[test]
fn a_jira_project_filter_applies_to_text_and_branch_names() {
    let mut st = with_jira(" proj ");
    assert_eq!(asks_project_keys(&mut st), Some(false));
    assert_eq!(refs(&st, "PROJ-1, ABC-2 and UTF-8"), ["PROJ-1"]);
    let fs = st.session.forge.as_ref().unwrap();
    assert_eq!(fs.branch_issue("feature/ABC-3-x"), None);
    assert_eq!(fs.branch_issue("feature/PROJ-4-x").as_deref(), Some("PROJ-4"));
}

/// The issue the open repository's branch `branch` names (its tracker is GitHub's issues).
fn branch_issue(st: &AppState, branch: &str) -> Option<String> {
    st.session.forge.as_ref().expect("a GitHub remote").branch_issue(branch)
}

#[test]
fn the_repository_chooses_whether_year_like_branch_names_name_issues() {
    let dir = scratch_repo("year-branches");
    git(&dir, &["remote", "add", "origin", "https://github.com/o/r.git"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    assert_eq!(branch_issue(&st, "2024-cleanup"), None);
    assert_eq!(branch_issue(&st, "20241016-release"), None);
    assert_eq!(branch_issue(&st, "123-fix").as_deref(), Some("123"));

    let fx = update(&mut st, Msg::Forge(ForgeMsg::SetYearBranches(true)), Instant::now());
    assert!(fx.iter().any(|e| matches!(e, Effect::Save(SaveEffect::Settings(..)))), "the choice is saved");
    assert_eq!(branch_issue(&st, "2024-cleanup").as_deref(), Some("2024"));
    assert_eq!(branch_issue(&st, "20241016-release").as_deref(), Some("20241016"));

    // Kept for the repository: opening it again reads it.
    let mut again = state();
    again.settings = st.settings.clone();
    drive(&mut again, Msg::Repo(RepoMsg::Open(dir)));
    assert_eq!(branch_issue(&again, "2024-cleanup").as_deref(), Some("2024"));

    update(&mut again, Msg::Forge(ForgeMsg::SetYearBranches(false)), Instant::now());
    assert_eq!(branch_issue(&again, "2024-cleanup"), None);
    assert!(again.settings.repo_year_branches.is_empty(), "off leaves no entry");
}

/// The forge effects `msg` starts, as (interactive, has a held sign-in, connected).
fn forge_sends(st: &mut AppState, msg: Msg) -> Vec<(bool, bool, bool)> {
    update(st, msg, Instant::now())
        .into_iter()
        .filter_map(|e| match e {
            Effect::Network(NetworkEffect::Forge { interactive, auth, connected, .. }) => Some((interactive, auth.is_some(), connected)),
            _ => None,
        })
        .collect()
}

fn reply(st: &mut AppState, result: ForgeResult) {
    let target = st.session.forge.as_ref().unwrap().host.clone().unwrap();
    update(st, Msg::Forge(ForgeMsg::ForgeReplied(ForgeReply { target, results: vec![result] })), Instant::now());
}

/// The runner's report of a credential its lookup found, sent before the call's own result.
fn found(st: &mut AppState, auth: Auth, asked: bool) {
    let account = st.session.forge.as_ref().unwrap().host.as_ref().unwrap().account();
    let found = FoundAuth { account, dir: AppState::outside_dir(), connected: false, asked, auth };
    update(st, Msg::Forge(ForgeMsg::AuthFound(Box::new(found))), Instant::now());
}

#[test]
fn a_profile_without_the_account_is_answered_once_then_waits_for_sign_in() {
    let mut st = with_host(&target(Provider::GitHub, "github.com"));
    // The first background request goes out and shows the spinner until the runner answers.
    assert_eq!(forge_sends(&mut st, Msg::Forge(ForgeMsg::ForgeRefresh { interactive: false })), [(false, false, false)]);
    assert_eq!(st.session.forge.as_ref().unwrap().loading, 1);

    // The runner found no environment token and asked nobody else.
    let none = glowfly_core::error::Error::Auth(forge::not_connected_text(Provider::GitHub, None)).to_string();
    reply(&mut st, ForgeResult::AuthFailed(none));
    let fs = st.session.forge.as_ref().unwrap();
    assert_eq!(fs.loading, 0);
    assert_eq!(fs.host_status, Status::Error("not connected to GitHub".into()));
    assert!(connect_prompt(Lang::En, fs, &[]).is_some(), "the sidebar offers Connect…");

    // A forced background refresh asks nobody: no effect, no spinner.
    assert!(forge_sends(&mut st, Msg::Repo(RepoMsg::Refresh)).is_empty());
    assert_eq!(st.session.forge.as_ref().unwrap().loading, 0);

    // Sign in asks, with the sign-in window allowed.
    assert_eq!(forge_sends(&mut st, Msg::Forge(ForgeMsg::ForgeRefresh { interactive: true })), [(true, false, false)]);
    let signed_in = Auth::from_token(Provider::GitHub, None, "t", HELPER_SOURCE);
    found(&mut st, signed_in, true);
    reply(&mut st, ForgeResult::Prs(Ok(vec![])));
    assert_eq!(st.session.forge.as_ref().unwrap().host_status, Status::Ok);
    // The sign-in it returned is held, so later background requests carry it.
    assert_eq!(forge_sends(&mut st, Msg::Repo(RepoMsg::Refresh)), [(false, true, false)]);
}

#[test]
fn a_connected_account_is_asked_again_after_not_connected() {
    let gh = target(Provider::GitHub, "github.com");
    let mut st = with_host(&gh);
    st.settings.connections.push(Connection { account: gh.account(), user: "ada".into(), method: "access token".into() });
    st.session.forge.as_mut().unwrap().host_status = Status::Error("not connected to GitHub".into());
    assert_eq!(forge_sends(&mut st, Msg::Repo(RepoMsg::Refresh)), [(false, false, true)]);

    // Only a not-connected answer is skipped: other failures of an unconnected account are retried.
    let mut st = with_host(&gh);
    st.session.forge.as_mut().unwrap().host_status = Status::Error("authentication failed: bad credentials".into());
    assert_eq!(forge_sends(&mut st, Msg::Repo(RepoMsg::Refresh)), [(false, false, false)]);
}
