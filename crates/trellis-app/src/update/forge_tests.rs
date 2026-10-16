//! Which hosting services the app contacts, and with which credentials.

use super::tests::state;
use super::update;
use crate::effect::{Effect, ForgeCall, ForgeError, ForgeReply, ForgeResult};
use crate::msg::ForgeMsg;
use crate::msg::Msg;
use crate::state::{AppState, ForgeState, ProjectKeys, Status};
use std::time::Instant;
use trellis_core::forge::{Account, Connection, Provider, Target};

fn target(provider: Provider, host: &str) -> Target {
    Target { provider, host: host.into(), owner: "o".into(), project: String::new(), repo: "r".into() }
}

fn with_host(t: &Target) -> AppState {
    let mut st = state();
    st.forge = Some(ForgeState::new(Some(t.clone()), "origin".into(), Some(t.clone())));
    st
}

/// The `connected` flag of each forge call `msg` starts.
fn forge_calls(st: &mut AppState, msg: Msg) -> Vec<bool> {
    update(st, msg, Instant::now())
        .into_iter()
        .filter_map(|e| match e {
            Effect::Forge { connected, .. } => Some(connected),
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
        let fs = st.forge.as_ref().unwrap();
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
    st.forge = Some(ForgeState::new(Some(target(Provider::GitHub, "github.com")), "origin".into(), Some(Target::jira(&jira, project))));
    st
}

/// Whether the tracker refresh also asks for the site's project keys.
fn asks_project_keys(st: &mut AppState) -> Option<bool> {
    update(st, Msg::Forge(ForgeMsg::ForgeRefresh { interactive: false }), Instant::now()).into_iter().find_map(|e| match e {
        Effect::Forge { call: ForgeCall::MyIssues { project_keys }, .. } => Some(project_keys),
        _ => None,
    })
}

fn project_keys_reply(st: &mut AppState, result: Result<Option<Vec<String>>, ForgeError>) {
    let target = st.forge.as_ref().unwrap().tracker.clone().unwrap();
    let reply = ForgeReply { target, auth: None, results: vec![ForgeResult::ProjectKeys(result)] };
    update(st, Msg::Forge(ForgeMsg::ForgeReplied(reply)), Instant::now());
}

fn refs(st: &AppState, text: &str) -> Vec<String> {
    st.forge.as_ref().unwrap().issue_refs(text)
}

#[test]
fn jira_keys_are_checked_against_the_sites_projects() {
    let mut st = with_jira("");
    assert_eq!(asks_project_keys(&mut st), Some(true));
    // Until the projects are known, words that are never projects are skipped.
    assert_eq!(refs(&st, "Fix UTF-8 in PROJ-12"), ["PROJ-12"]);

    project_keys_reply(&mut st, Ok(Some(vec!["ABC".into()])));
    assert_eq!(refs(&st, "PROJ-12 ABC-3"), ["ABC-3"]);
    assert_eq!(st.forge.as_ref().unwrap().branch_issue("feature/UTF-8-fix"), None);
    assert_eq!(st.forge.as_ref().unwrap().branch_issue("feature/ABC-3-fix").as_deref(), Some("ABC-3"));
    assert_eq!(asks_project_keys(&mut st), Some(false));

    // Too many projects to list: the word list applies, and they aren't asked for again.
    let mut st = with_jira("");
    project_keys_reply(&mut st, Ok(None));
    assert_eq!(st.forge.as_ref().unwrap().jira_projects, ProjectKeys::TooMany);
    assert_eq!(refs(&st, "UTF-8 and PROJ-12"), ["PROJ-12"]);
    assert_eq!(asks_project_keys(&mut st), Some(false));

    // A failed read is tried again on the next refresh.
    let mut st = with_jira("");
    project_keys_reply(&mut st, Err(ForgeError { message: "boom".into(), auth: false }));
    assert_eq!(st.forge.as_ref().unwrap().jira_projects, ProjectKeys::Unknown);
    assert_eq!(asks_project_keys(&mut st), Some(true));
}

#[test]
fn a_jira_project_filter_applies_to_text_and_branch_names() {
    let mut st = with_jira(" proj ");
    assert_eq!(asks_project_keys(&mut st), Some(false));
    assert_eq!(refs(&st, "PROJ-1, ABC-2 and UTF-8"), ["PROJ-1"]);
    let fs = st.forge.as_ref().unwrap();
    assert_eq!(fs.branch_issue("feature/ABC-3-x"), None);
    assert_eq!(fs.branch_issue("feature/PROJ-4-x").as_deref(), Some("PROJ-4"));
}
