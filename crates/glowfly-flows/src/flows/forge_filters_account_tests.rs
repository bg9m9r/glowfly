//! Pull request and issue filters across changes of service or account: answers from a tracker
//! no longer used, a new sign-in, and looking the signed-in user up only once.

use super::forge_filters_tests::{filter_calls, open, pr, reply, send};
use super::update;
use crate::effect::{FilterCall, FilterResult};
use crate::msg::{FilterMsg, ForgeMsg, Msg};
use crate::state::forge_filters::{MeState, OptionsState, connect_issues_prompt, connect_prompt, issues_sign_in, pr_view};
use crate::state::{AppState, Status};
use glowfly_core::forge::{
    Account, Auth, Builtin, Connection, FilterChoice, FilterKind, FilterOptions, Issue, IssueFacets, Me, PrFacets, Provider, PullRequest,
    Target,
};
use glowfly_core::i18n::Lang;
use glowfly_core::settings::TrackerChoice;
use std::time::Instant;

fn choose(kind: FilterKind, b: Builtin) -> FilterMsg {
    FilterMsg::Choose { kind, choice: FilterChoice::Builtin(b) }
}

fn host(st: &AppState) -> Target {
    st.session.forge.as_ref().unwrap().host.clone().unwrap()
}

fn user(id: &str) -> Me {
    Me { id: id.into(), name: id.into(), ..Default::default() }
}

fn issue(key: &str) -> Issue {
    Issue {
        key: key.into(),
        title: format!("I{key}"),
        state: "open".into(),
        kind: "Issue".into(),
        assignee: None,
        url: String::new(),
        facets: IssueFacets::default(),
    }
}

fn created() -> (FilterChoice, glowfly_core::forge::Conditions) {
    (FilterChoice::Builtin(Builtin::IssuesCreatedByMe), Builtin::IssuesCreatedByMe.conditions())
}

#[test]
fn answers_from_the_previous_tracker_are_dropped() {
    let (mut st, dir) = open("ffa-tracker");
    st.session.forge.as_mut().unwrap().filters.me = MeState::Known(user("ada"));
    send(&mut st, choose(FilterKind::Issues, Builtin::IssuesCreatedByMe));
    let github = host(&st);
    let jira = Account { provider: Provider::JiraCloud, host: "acme.atlassian.net".into(), org: String::new() };
    update(&mut st, Msg::Forge(ForgeMsg::SetTracker(TrackerChoice::Jira { account: jira, project: "AC".into() })), Instant::now());
    let fs = st.session.forge.as_ref().unwrap();
    assert_eq!((fs.host.as_ref(), fs.tracker.as_ref().map(|t| t.provider)), (Some(&github), Some(Provider::JiraCloud)));
    assert_eq!(fs.filters.issues.as_ref().map(|l| &l.status), Some(&Status::Idle), "the Jira search is shown");

    // GitHub is still the code host, but no longer the tracker.
    let (choice, conditions) = created();
    reply(&mut st, github.clone(), FilterResult::Issues { choice, conditions, epoch: 0, me: None, result: Ok(vec![issue("7")]) });
    reply(&mut st, github, FilterResult::Options(FilterKind::Issues, Ok(FilterOptions::default())));
    let fs = st.session.forge.as_ref().unwrap();
    let list = fs.filters.issues.as_ref().unwrap();
    assert_eq!((list.items.len(), &list.status), (0, &Status::Idle));
    assert!(!fs.cache.contains_key("7"));
    assert_eq!(fs.filters.issue_options, OptionsState::NotAsked);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_new_sign_in_forgets_the_previous_user() {
    let (mut st, dir) = open("ffa-account");
    send(&mut st, choose(FilterKind::Prs, Builtin::MyPrs));
    let h = host(&st);
    reply(&mut st, h.clone(), FilterResult::Me { epoch: 0, result: Ok(user("alice")) });
    assert_eq!(st.session.forge.as_ref().unwrap().filters.me, MeState::Known(user("alice")));

    let account = h.account();
    update(&mut st, Msg::Forge(ForgeMsg::Disconnect(account.clone())), Instant::now());
    assert_eq!(st.session.forge.as_ref().unwrap().filters.me, MeState::Unknown);
    let connected = ForgeMsg::Connected { account, method: "access token".into(), token: None, result: Ok("bob".into()) };
    let fx = update(&mut st, Msg::Forge(connected), Instant::now());
    let calls = filter_calls(&fx);
    let Some(&(_, &FilterCall::Me { epoch })) = calls.iter().find(|(_, c)| matches!(c, FilterCall::Me { .. })) else {
        panic!("the new account is looked up: {calls:?}")
    };
    // A lookup sent under the old sign-in comes back late and is dropped.
    reply(&mut st, h.clone(), FilterResult::Me { epoch: 0, result: Ok(user("alice")) });
    assert_eq!(st.session.forge.as_ref().unwrap().filters.me, MeState::Asking);
    reply(&mut st, h, FilterResult::Me { epoch, result: Ok(user("bob")) });
    assert_eq!(st.session.forge.as_ref().unwrap().filters.me, MeState::Known(user("bob")));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_issue_search_that_looked_up_the_user_saves_asking_again() {
    let (mut st, dir) = open("ffa-once");
    let fx = send(&mut st, choose(FilterKind::Issues, Builtin::IssuesCreatedByMe));
    assert!(matches!(filter_calls(&fx)[..], [(_, FilterCall::Issues { me: None, .. })]));
    let (choice, conditions) = created();
    let h = host(&st);
    reply(&mut st, h, FilterResult::Issues { choice, conditions, epoch: 0, me: Some(user("ada")), result: Ok(vec![issue("7")]) });
    assert_eq!(st.session.forge.as_ref().unwrap().filters.me, MeState::Known(user("ada")));

    let fx = update(&mut st, Msg::Forge(ForgeMsg::ForgeRefresh { interactive: false }), Instant::now());
    let calls = filter_calls(&fx);
    assert!(matches!(calls[..], [(_, FilterCall::Issues { me: Some(m), .. })] if m.id == "ada"), "{calls:?}");
    assert!(filter_calls(&send(&mut st, choose(FilterKind::Prs, Builtin::MyPrs))).is_empty(), "known");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_user_without_teams_keeps_the_sign_in_and_personal_requests() {
    let (mut st, dir) = open("ffa-no-teams");
    let h = host(&st);
    let key = h.account().key();
    st.session.forge.as_mut().unwrap().auth.insert(key.clone(), Auth::from_token(Provider::GitHub, None, "tok", "test"));
    send(&mut st, choose(FilterKind::Prs, Builtin::NeedsMyReview));
    let asked = |id, f: PrFacets| PullRequest { facets: PrFacets { author_id: "grace".into(), ..f }, ..pr(id, "grace") };
    st.session.forge.as_mut().unwrap().prs = vec![
        asked(1, PrFacets { awaiting_team_ids: vec!["o/core".into()], ..Default::default() }),
        asked(2, PrFacets { awaiting_ids: vec!["ada".into()], ..Default::default() }),
    ];
    // The teams couldn't be read: the user comes back without any.
    reply(&mut st, h, FilterResult::Me { epoch: 0, result: Ok(Me { teams: vec![], ..user("ada") }) });
    let fs = st.session.forge.as_ref().unwrap();
    assert_eq!(fs.filters.me, MeState::Known(user("ada")));
    assert!(fs.auth.contains_key(&key), "still signed in");
    let filters = st.settings.forge_filters_for(&st.snap.as_ref().unwrap().real_workdir);
    assert_eq!(pr_view(Lang::En, fs, &filters).rows.iter().map(|p| p.id).collect::<Vec<_>>(), [2], "the team-only request isn't listed");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_account_not_connected_is_offered_a_connection() {
    let (mut st, dir) = open("ffa-connect");
    let fs = st.session.forge.as_mut().unwrap();
    // As the runner reports `resolve_auth` finding no credentials at all.
    fs.host_status = Status::Error("authentication failed: not connected to GitHub".into());
    fs.tracker_status = Status::Error("authentication failed: not connected to GitHub".into());
    let fs = st.session.forge.as_ref().unwrap();
    assert_eq!(connect_prompt(Lang::En, fs, &[]).as_deref(), Some("Connect GitHub to see its pull requests here."));
    let issues = connect_issues_prompt(Lang::En, fs, &[], true);
    assert_eq!(issues, Some(("Connect GitHub to see its issues here.".to_owned(), false)), "the PR section's Connect… covers it");
    let collapsed = connect_issues_prompt(Lang::En, fs, &[], false);
    assert_eq!(collapsed.map(|c| c.1), Some(true), "with PULL REQUESTS collapsed, its own Connect…");
    // Connected but failing (an expired token): the error and Sign in show instead.
    let connected = [Connection { account: host(&st).account(), user: "ada".into(), method: "access token".into() }];
    assert_eq!(connect_prompt(Lang::En, fs, &connected), None);
    assert_eq!(connect_issues_prompt(Lang::En, fs, &connected, true), None);
    assert!(!issues_sign_in(fs, true), "one Sign in for the account, in PULL REQUESTS");
    assert!(issues_sign_in(fs, false), "its own while PULL REQUESTS is collapsed");
    st.session.forge.as_mut().unwrap().host_status = Status::Ok;
    assert_eq!(connect_prompt(Lang::En, st.session.forge.as_ref().unwrap(), &[]), None, "it answered");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_empty_pull_request_list_speaks_the_language() {
    let (st, dir) = open("ffa-empty-de");
    let fs = st.session.forge.as_ref().unwrap();
    let filters = st.settings.forge_filters_for(&st.snap.as_ref().unwrap().real_workdir);
    assert_eq!(pr_view(Lang::En, fs, &filters).empty.as_deref(), Some("No open pull requests"));
    assert_eq!(pr_view(Lang::De, fs, &filters).empty.as_deref(), Some("Keine offenen Pull Requests"));
    let _ = std::fs::remove_dir_all(dir);
}

/// Credentials from Git Credential Manager or `GITHUB_TOKEN` (no connection in Preferences) that
/// fail offline or on a rate limit keep their cause and Sign in rather than asking to connect.
#[test]
fn a_failure_with_credentials_is_not_a_missing_connection() {
    let (mut st, dir) = open("ffa-offline");
    for cause in ["network error: offline", "HTTP 403: API rate limit exceeded"] {
        st.session.forge.as_mut().unwrap().host_status = Status::Error(cause.into());
        assert_eq!(connect_prompt(Lang::En, st.session.forge.as_ref().unwrap(), &[]), None, "{cause}");
    }
    let _ = std::fs::remove_dir_all(dir);
}
