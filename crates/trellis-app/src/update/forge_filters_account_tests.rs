//! Pull request and issue filters across changes of service or account: answers from a tracker
//! no longer used, a new sign-in, and looking the signed-in user up only once.

use super::forge_filters_tests::{filter_calls, open, reply, send};
use super::update;
use crate::effect::{FilterCall, FilterResult};
use crate::msg::{FilterMsg, ForgeMsg, Msg};
use crate::state::forge_filters::{MeState, OptionsState};
use crate::state::{AppState, Status};
use std::time::Instant;
use trellis_core::forge::{Account, Builtin, FilterChoice, FilterKind, FilterOptions, Issue, IssueFacets, Me, Provider, Target};
use trellis_core::settings::TrackerChoice;

fn choose(kind: FilterKind, b: Builtin) -> FilterMsg {
    FilterMsg::Choose { kind, choice: FilterChoice::Builtin(b) }
}

fn host(st: &AppState) -> Target {
    st.forge.as_ref().unwrap().host.clone().unwrap()
}

fn user(id: &str) -> Me {
    Me { id: id.into(), name: id.into() }
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

fn created() -> (FilterChoice, trellis_core::forge::Conditions) {
    (FilterChoice::Builtin(Builtin::IssuesCreatedByMe), Builtin::IssuesCreatedByMe.conditions())
}

#[test]
fn answers_from_the_previous_tracker_are_dropped() {
    let (mut st, dir) = open("ffa-tracker");
    st.forge.as_mut().unwrap().filters.me = MeState::Known(user("ada"));
    send(&mut st, choose(FilterKind::Issues, Builtin::IssuesCreatedByMe));
    let github = host(&st);
    let jira = Account { provider: Provider::JiraCloud, host: "acme.atlassian.net".into(), org: String::new() };
    update(&mut st, Msg::Forge(ForgeMsg::SetTracker(TrackerChoice::Jira { account: jira, project: "AC".into() })), Instant::now());
    let fs = st.forge.as_ref().unwrap();
    assert_eq!((fs.host.as_ref(), fs.tracker.as_ref().map(|t| t.provider)), (Some(&github), Some(Provider::JiraCloud)));
    assert_eq!(fs.filters.issues.as_ref().map(|l| &l.status), Some(&Status::Idle), "the Jira search is shown");

    // GitHub is still the code host, but no longer the tracker.
    let (choice, conditions) = created();
    reply(&mut st, github.clone(), FilterResult::Issues { choice, conditions, epoch: 0, me: None, result: Ok(vec![issue("7")]) });
    reply(&mut st, github, FilterResult::Options(FilterKind::Issues, Ok(FilterOptions::default())));
    let fs = st.forge.as_ref().unwrap();
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
    assert_eq!(st.forge.as_ref().unwrap().filters.me, MeState::Known(user("alice")));

    let account = h.account();
    update(&mut st, Msg::Forge(ForgeMsg::Disconnect(account.clone())), Instant::now());
    assert_eq!(st.forge.as_ref().unwrap().filters.me, MeState::Unknown);
    let connected = ForgeMsg::Connected { account, method: "access token".into(), token: None, result: Ok("bob".into()) };
    let fx = update(&mut st, Msg::Forge(connected), Instant::now());
    let calls = filter_calls(&fx);
    let Some(&(_, &FilterCall::Me { epoch })) = calls.iter().find(|(_, c)| matches!(c, FilterCall::Me { .. })) else {
        panic!("the new account is looked up: {calls:?}")
    };
    // A lookup sent under the old sign-in comes back late and is dropped.
    reply(&mut st, h.clone(), FilterResult::Me { epoch: 0, result: Ok(user("alice")) });
    assert_eq!(st.forge.as_ref().unwrap().filters.me, MeState::Asking);
    reply(&mut st, h, FilterResult::Me { epoch, result: Ok(user("bob")) });
    assert_eq!(st.forge.as_ref().unwrap().filters.me, MeState::Known(user("bob")));
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
    assert_eq!(st.forge.as_ref().unwrap().filters.me, MeState::Known(user("ada")));

    let fx = update(&mut st, Msg::Forge(ForgeMsg::ForgeRefresh { interactive: false }), Instant::now());
    let calls = filter_calls(&fx);
    assert!(matches!(calls[..], [(_, FilterCall::Issues { me: Some(m), .. })] if m.id == "ada"), "{calls:?}");
    assert!(filter_calls(&send(&mut st, choose(FilterKind::Prs, Builtin::MyPrs))).is_empty(), "known");
    let _ = std::fs::remove_dir_all(dir);
}
