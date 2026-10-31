//! Headless tests for the Create dialog on Trello (all boards and one board), Azure Boards and
//! Bitbucket Cloud, a token that may only read, Bitbucket Data Center pointing to Jira, and a
//! card created for another tab.

use super::issue_tests::{done, err, github, jobs, open_repo, reply, run};
use crate::effect::{Effect, IssueCall, IssueReply};
use crate::msg::{IssueMsg, Msg};
use crate::state::issue_form::{IssueForm, OptionsState};
use crate::state::palette::shown;
use crate::state::{AppState, Modal};
use glowfly_core::forge::filters::Named;
use glowfly_core::forge::{Account, Auth, Connection, CreateOptions, Issue, NewIssue, Provider, Target};

const BOARD: &str = "5f1a2b3c4d5e6f7a8b9c0d1e";

fn form(st: &AppState) -> &IssueForm {
    match &st.modal {
        Some(Modal::CreateIssue(f)) => f,
        _ => panic!("the dialog is open"),
    }
}

fn form_mut(st: &mut AppState) -> &mut IssueForm {
    match &mut st.modal {
        Some(Modal::CreateIssue(f)) => f,
        _ => panic!("the dialog is open"),
    }
}

fn options_calls(fx: &[Effect]) -> Vec<String> {
    jobs(fx)
        .into_iter()
        .filter_map(|j| match &j.call {
            IssueCall::Options(p) => Some(p.clone()),
            _ => None,
        })
        .collect()
}

fn creates(fx: &[Effect]) -> Vec<NewIssue> {
    jobs(fx)
        .into_iter()
        .filter_map(|j| match &j.call {
            IssueCall::Create(n) => Some((**n).clone()),
            _ => None,
        })
        .collect()
}

fn named(key: &str, title: &str) -> Named {
    Named { key: key.into(), title: title.into() }
}

fn options(st: &AppState, project: &str, result: CreateOptions) -> IssueMsg {
    reply(st, IssueReply::Options { project: project.into(), result: Ok(result) })
}

/// Makes `t` the tracker, connected.
fn tracker(st: &mut AppState, t: Target) -> Target {
    st.settings.connections.push(Connection { account: t.account(), user: "Ada".into(), method: "token".into() });
    st.session.forge.as_mut().unwrap().tracker = Some(t.clone());
    t
}

fn trello(st: &mut AppState, board: &str) -> Target {
    let account = Account { provider: Provider::Trello, host: "trello.com".into(), org: String::new() };
    tracker(st, Target::trello(&account, board, if board.is_empty() { "" } else { "Website" }))
}

fn card(key: &str, title: &str) -> Issue {
    Issue {
        key: key.into(),
        title: title.into(),
        state: "To do".into(),
        kind: "Website".into(),
        assignee: None,
        url: format!("https://trello.com/c/{key}"),
        facets: Default::default(),
    }
}

fn lists() -> CreateOptions {
    CreateOptions {
        types: vec![named("l1", "To do"), named("l2", "Doing")],
        default_type: Some("l1".into()),
        labels: vec!["Bug".into()],
        max_assignees: usize::MAX,
        ..Default::default()
    }
}

#[test]
fn trello_with_all_boards_asks_for_a_board_first() {
    let (mut st, dir) = open_repo("issue-trello-all");
    trello(&mut st, "");
    let fx = run(&mut st, IssueMsg::CreateDialog);
    assert_eq!(options_calls(&fx), [String::new()], "the boards");
    let msg = options(&st, "", CreateOptions { boards: vec![named(BOARD, "Website")], ..Default::default() });
    run(&mut st, msg);
    assert!(matches!(form(&st).options, OptionsState::Loaded(_)));
    assert_eq!(form(&st).boards.len(), 1);
    form_mut(&mut st).title = "Fix login".into();
    assert_eq!(form(&st).submit_blocked().as_deref(), Some("Choose a board."));
    let fx = run(&mut st, IssueMsg::SetProject(BOARD.into()));
    assert_eq!(options_calls(&fx), [BOARD]);
    let late = options(&st, "", CreateOptions { boards: vec![named("6a6a6a6a6a6a6a6a6a6a6a6a", "Ops")], ..Default::default() });
    run(&mut st, late);
    assert_eq!(form(&st).options, OptionsState::Loading, "an answer for the old board is dropped");
    let msg = options(&st, BOARD, lists());
    run(&mut st, msg);
    assert_eq!((form(&st).kind.as_deref(), form(&st).boards.len()), (Some("l1"), 1));
    let sent = creates(&run(&mut st, IssueMsg::Create));
    assert_eq!(sent.len(), 1);
    assert_eq!((sent[0].project.as_str(), sent[0].kind.as_deref(), sent[0].title.as_str()), (BOARD, Some("l1"), "Fix login"));
    done(st, dir);
}

#[test]
fn trello_with_one_board_asks_for_its_lists_at_once() {
    let (mut st, dir) = open_repo("issue-trello-one");
    trello(&mut st, BOARD);
    let fx = run(&mut st, IssueMsg::CreateDialog);
    assert_eq!(options_calls(&fx), [BOARD]);
    assert!(form(&st).board_fixed());
    done(st, dir);
}

#[test]
fn azure_waits_for_the_types_then_picks_the_default() {
    let (mut st, dir) = open_repo("issue-azure-create");
    let az = Target {
        provider: Provider::AzureDevOps,
        host: "dev.azure.com".into(),
        owner: "contoso".into(),
        project: "Web".into(),
        repo: "r".into(),
    };
    tracker(&mut st, az);
    run(&mut st, IssueMsg::CreateDialog);
    form_mut(&mut st).title = "Login page".into();
    assert_eq!(form(&st).submit_blocked().as_deref(), Some("Wait for the work item types to load."));
    assert!(creates(&run(&mut st, IssueMsg::Create)).is_empty(), "held until the types load");
    let opts = CreateOptions {
        types: vec![named("Bug", "Bug"), named("User Story", "User Story")],
        default_type: Some("User Story".into()),
        labels_free: true,
        max_assignees: 1,
        ..Default::default()
    };
    let msg = options(&st, "", opts);
    run(&mut st, msg);
    assert_eq!(form(&st).kind.as_deref(), Some("User Story"));
    let sent = creates(&run(&mut st, IssueMsg::Create));
    assert_eq!(sent[0].kind.as_deref(), Some("User Story"));
    let created = Issue {
        key: "12".into(),
        title: "Login page".into(),
        state: "New".into(),
        kind: "User Story".into(),
        assignee: None,
        url: String::new(),
        facets: Default::default(),
    };
    let msg = reply(&st, IssueReply::Created(Ok(created)));
    run(&mut st, msg);
    assert!(st.modal.is_none());
    let toast = st.toasts.last().expect("a toast");
    assert_eq!(toast.text, "Created work item AB#12: Login page");
    assert!(toast.action.as_ref().is_some_and(|a| a.label == "View" && matches!(&*a.msg, Msg::Issue(IssueMsg::Open(k)) if k == "12")));
    done(st, dir);
}

#[test]
fn bitbucket_sends_the_default_priority_and_shows_a_turned_off_tracker() {
    let (mut st, dir) = open_repo("issue-bitbucket-create");
    tracker(&mut st, Target { provider: Provider::Bitbucket, host: "bitbucket.org".into(), ..github() });
    run(&mut st, IssueMsg::CreateDialog);
    let opts = CreateOptions {
        types: vec![named("bug", "Bug"), named("task", "Task")],
        priorities: vec![named("minor", "Minor"), named("major", "Major")],
        default_type: Some("bug".into()),
        default_priority: Some("major".into()),
        max_assignees: 1,
        ..Default::default()
    };
    let msg = options(&st, "", opts);
    run(&mut st, msg);
    form_mut(&mut st).title = "Crash".into();
    let sent = creates(&run(&mut st, IssueMsg::Create));
    assert_eq!((sent[0].kind.as_deref(), sent[0].priority.as_deref()), (Some("bug"), Some("major")));
    st.modal = None;

    run(&mut st, IssueMsg::CreateDialog);
    let off = "This repository's issue tracker is turned off on Bitbucket. Turn it on in the repository's settings on Bitbucket to \
               create and view issues.";
    let msg = reply(&st, IssueReply::Options { project: String::new(), result: Err(err(off)) });
    run(&mut st, msg);
    assert_eq!(form(&st).options, OptionsState::Failed(off.into()));
    form_mut(&mut st).title = "Crash".into();
    assert_eq!(creates(&run(&mut st, IssueMsg::Create)).len(), 1, "creating stays possible");
    done(st, dir);
}

#[test]
fn a_read_only_trello_token_keeps_the_dialog_and_the_sign_in() {
    let (mut st, dir) = open_repo("issue-trello-read-only");
    let t = trello(&mut st, BOARD);
    let key = t.account().key();
    st.session.forge.as_mut().unwrap().auth.insert(key.clone(), Auth::from_token(Provider::Trello, Some("k"), "tok", "test"));
    run(&mut st, IssueMsg::CreateDialog);
    let msg = options(&st, BOARD, lists());
    run(&mut st, msg);
    form_mut(&mut st).title = "Fix login".into();
    assert_eq!(creates(&run(&mut st, IssueMsg::Create)).len(), 1);
    let why = "access denied: this Trello token can read cards but not create them (or you only observe this board). Allow a token \
               that can also write, then paste it in Preferences > Integrations.";
    let msg = reply(&st, IssueReply::Created(Err(err(why))));
    run(&mut st, msg);
    let f = form(&st);
    assert_eq!((f.busy, f.error.as_deref()), (false, Some(why)));
    assert!(st.session.forge.as_ref().unwrap().auth.contains_key(&key), "the token is kept");
    assert!(st.log.last().is_some_and(|l| !l.ok && l.text.starts_with("Creating the card failed: ")));
    done(st, dir);
}

#[test]
fn bitbucket_data_center_points_to_jira() {
    let (mut st, dir) = open_repo("issue-bbdc");
    let fs = st.session.forge.as_mut().unwrap();
    fs.host = Some(Target { provider: Provider::BitbucketDataCenter, host: "bitbucket.acme.io".into(), ..github() });
    fs.tracker = None;
    assert!(run(&mut st, IssueMsg::CreateDialog).is_empty());
    assert!(st.modal.is_none());
    let entry = shown(&st, "").into_iter().find(|s| s.entry.label == "Create issue…").map(|s| s.entry.detail);
    let why = "Bitbucket Data Center has no issue tracker of its own; choose Jira under Preferences > This repository > Issue tracker";
    assert_eq!(entry, Some(Some(why.into())));
    done(st, dir);
}

#[test]
fn a_card_created_for_another_tab_is_logged_with_its_folder() {
    let (mut st, dir) = open_repo("issue-trello-elsewhere");
    let t = trello(&mut st, BOARD);
    let msg = IssueMsg::Replied {
        workdir: glowfly_core::paths::temp_dir().join("other-repo"),
        target: t,
        generation: st.session.issue.generation,
        reply: Box::new(IssueReply::Created(Ok(card("AbCd1234", "Fix login")))),
    };
    run(&mut st, msg);
    assert!(st.log.last().is_some_and(|l| l.ok && l.text == "Created card AbCd1234 in other-repo"));
    done(st, dir);
}
