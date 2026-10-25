//! Headless tests for the Create issue dialog: opening it per tracker, loading its choices per
//! project, the checks that hold Create back, creating, and the replies (success with its
//! toast, cache, refresh and branch dialog; failure; after Cancel; for another tab), tab
//! switches and tracker changes.

use super::issue_tests::{done, err, github, jobs, open_repo, reply, run};
use super::tests::{drive, git};
use super::update;
use crate::effect::{Effect, ForgeCall, IssueCall, IssueReply};
use crate::msg::{IssueMsg, Msg, RepoMsg, TabsMsg};
use crate::state::issue_form::{IssueForm, OptionsState};
use crate::state::{AppState, Modal};
use glowfly_core::forge::filters::Named;
use glowfly_core::forge::{Account, Connection, CreateOptions, Issue, NewIssue, Provider, Target, branch_name_for};
use std::time::Instant;

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

fn refreshes(fx: &[Effect]) -> usize {
    fx.iter().filter(|e| matches!(e, Effect::Forge { call: ForgeCall::Refresh { .. }, .. })).count()
}

fn jira_tracker(st: &mut AppState, project: &str) -> Target {
    let account = Account { provider: Provider::JiraCloud, host: "acme.atlassian.net".into(), org: String::new() };
    st.settings.connections.push(Connection { account: account.clone(), user: "ada@example.com".into(), method: "token".into() });
    let t = Target::jira(&account, project);
    st.forge.as_mut().unwrap().tracker = Some(t.clone());
    t
}

fn created(key: &str, title: &str) -> Issue {
    Issue {
        key: key.into(),
        title: title.into(),
        state: "open".into(),
        kind: "Issue".into(),
        assignee: None,
        url: format!("https://github.com/o/r/issues/{key}"),
        facets: Default::default(),
    }
}

/// The GitHub repository with the dialog open and a valid title typed.
fn ready(name: &str) -> (AppState, std::path::PathBuf) {
    let (mut st, dir) = open_repo(name);
    run(&mut st, IssueMsg::CreateDialog);
    form_mut(&mut st).title = "Crash on start".into();
    (st, dir)
}

#[test]
fn the_dialog_opens_and_asks_for_its_choices() {
    let (mut st, dir) = open_repo("issue-create-open");
    let fx = run(&mut st, IssueMsg::CreateDialog);
    assert_eq!(options_calls(&fx), [String::new()]);
    let f = form(&st);
    assert_eq!((&f.target, &f.options), (&github(), &OptionsState::Loading));
    assert!(run(&mut st, IssueMsg::CreateDialog).is_empty(), "already open");
    st.modal = None;

    // Jira: the project filter fills the project in; without one nothing is asked yet.
    jira_tracker(&mut st, "PROJ");
    let fx = run(&mut st, IssueMsg::CreateDialog);
    assert_eq!(options_calls(&fx), ["PROJ"]);
    assert_eq!(form(&st).project, "PROJ");
    st.modal = None;
    jira_tracker(&mut st, "");
    assert!(options_calls(&run(&mut st, IssueMsg::CreateDialog)).is_empty());
    assert_eq!(form(&st).options, OptionsState::NotAsked);
    st.modal = None;

    // Bitbucket Data Center has no tracker of its own: nothing opens.
    let dc = Target { provider: Provider::BitbucketDataCenter, host: "bitbucket.acme.io".into(), ..github() };
    let fs = st.forge.as_mut().unwrap();
    fs.host = Some(dc);
    fs.tracker = None;
    assert!(run(&mut st, IssueMsg::CreateDialog).is_empty());
    assert!(st.modal.is_none());
    done(st, dir);
}

#[test]
fn another_project_resets_its_choices_and_old_answers_are_dropped() {
    let (mut st, dir) = open_repo("issue-create-project");
    let t = jira_tracker(&mut st, "PROJ");
    run(&mut st, IssueMsg::CreateDialog);
    let f = form_mut(&mut st);
    f.kind = Some("1".into());
    f.epic = Some("PROJ-1".into());
    f.people = vec!["acc-1".into()];
    let fx = run(&mut st, IssueMsg::SetProject("OPS".into()));
    assert_eq!(options_calls(&fx), ["OPS"]);
    let f = form(&st);
    assert_eq!((f.project.as_str(), f.kind.as_ref(), f.epic.as_ref(), f.people.len()), ("OPS", None, None, 0));
    assert!(run(&mut st, IssueMsg::SetProject("OPS".into())).is_empty(), "the same project again");

    let opts = CreateOptions { types: vec![Named { key: "10".into(), title: "Task".into() }], max_assignees: 1, ..Default::default() };
    let old = IssueMsg::Replied {
        workdir: st.workdir().unwrap(),
        target: t.clone(),
        generation: 0,
        reply: Box::new(IssueReply::Options { project: "PROJ".into(), result: Ok(opts.clone()) }),
    };
    run(&mut st, old);
    assert_eq!(form(&st).options, OptionsState::Loading, "the answer for the old project");
    let msg = reply(&st, IssueReply::Options { project: "OPS".into(), result: Ok(opts) });
    run(&mut st, msg);
    assert_eq!(form(&st).kind.as_deref(), Some("10"));
    let msg = reply(&st, IssueReply::Options { project: "OPS".into(), result: Err(err("boom")) });
    run(&mut st, msg);
    assert_eq!(form(&st).options, OptionsState::Failed("boom".into()));
    assert_eq!(options_calls(&run(&mut st, IssueMsg::SetProject("OPS".into()))), ["OPS"], "Try again");
    done(st, dir);
}

#[test]
fn create_sends_the_issue_once_and_only_without_problems() {
    let (mut st, dir) = open_repo("issue-create-send");
    run(&mut st, IssueMsg::CreateDialog);
    assert!(run(&mut st, IssueMsg::Create).is_empty(), "no title yet");
    let f = form_mut(&mut st);
    f.title = " Crash\non start ".into();
    f.labels = vec!["bug".into(), "Bug".into()];
    f.people = vec!["ada".into()];
    let fx = run(&mut st, IssueMsg::Create);
    let sent = creates(&fx);
    assert_eq!(sent.len(), 1);
    assert_eq!((sent[0].title.as_str(), sent[0].labels.as_slice()), ("Crash on start", &["bug".to_owned()][..]));
    assert_eq!(sent[0].assignees, ["ada"]);
    assert!(form(&st).busy);
    assert!(creates(&run(&mut st, IssueMsg::Create)).is_empty(), "already being created");
    done(st, dir);
}

#[test]
fn a_created_issue_is_logged_toasted_cached_and_listed_again() {
    let (mut st, dir) = ready("issue-create-ok");
    run(&mut st, IssueMsg::Create);
    let msg = reply(&st, IssueReply::Created(Ok(created("12", "Crash on start"))));
    let fx = run(&mut st, msg);
    assert!(st.modal.is_none());
    assert!(st.log.last().is_some_and(|l| l.ok && l.text == "Created issue #12: Crash on start"));
    let toast = st.toasts.last().expect("a toast");
    assert_eq!(toast.text, "Created issue #12: Crash on start");
    let action = toast.action.as_ref().expect("with View");
    assert_eq!(action.label, "View");
    assert!(matches!(&*action.msg, Msg::Issue(IssueMsg::Open(k)) if k == "12"));
    assert_eq!(action.workdir, st.workdir());
    assert!(st.forge.as_ref().unwrap().cache.contains_key("12"));
    assert_eq!(refreshes(&fx), 1);
    // The View action opens the panel.
    let id = toast.id;
    update(&mut st, Msg::ToastAction(id), Instant::now());
    assert_eq!(st.issue.as_ref().map(|v| v.key.as_str()), Some("12"));
    done(st, dir);
}

#[test]
fn a_branch_can_follow() {
    let (mut st, dir) = ready("issue-create-branch");
    form_mut(&mut st).then_branch = true;
    run(&mut st, IssueMsg::Create);
    let issue = created("12", "Crash on start");
    let msg = reply(&st, IssueReply::Created(Ok(issue.clone())));
    run(&mut st, msg);
    match &st.modal {
        Some(Modal::NewBranch { start: None, name, checkout: true, .. }) => assert_eq!(name, &branch_name_for(&issue)),
        _ => panic!("the New branch dialog"),
    }
    done(st, dir);
}

#[test]
fn a_failure_keeps_the_dialog_with_the_message() {
    let (mut st, dir) = ready("issue-create-err");
    run(&mut st, IssueMsg::Create);
    let msg = reply(&st, IssueReply::Created(Err(err("the server said “title: can't be blank”"))));
    run(&mut st, msg);
    let f = form(&st);
    assert!(!f.busy);
    assert_eq!(f.error.as_deref(), Some("the server said “title: can't be blank”"));
    assert!(st.log.last().is_some_and(|l| !l.ok && l.text.starts_with("Creating the issue failed: ")));
    // The dialog shows it; the failure toast only points to the log.
    assert!(st.toasts.iter().all(|t| t.ok || t.action.as_ref().is_some_and(|a| a.label == "Details")));
    done(st, dir);
}

#[test]
fn after_cancel_or_for_another_tab_the_reply_is_only_reported() {
    let (mut st, dir) = ready("issue-create-cancel");
    run(&mut st, IssueMsg::Create);
    let failed = reply(&st, IssueReply::Created(Err(err("boom"))));
    let ok = reply(&st, IssueReply::Created(Ok(created("13", "Late"))));
    update(&mut st, Msg::CloseModal, Instant::now());
    run(&mut st, failed);
    assert!(st.modal.is_none());
    assert!(st.toasts.last().is_some_and(|t| !t.ok && t.text.contains("boom")));
    run(&mut st, ok);
    assert!(st.modal.is_none());
    assert!(st.log.last().is_some_and(|l| l.ok && l.text == "Created issue #13: Late"));

    let elsewhere = glowfly_core::paths::temp_dir().join("other-repo");
    let msg = IssueMsg::Replied {
        workdir: elsewhere,
        target: github(),
        generation: st.issue_gen,
        reply: Box::new(IssueReply::Created(Ok(created("14", "Elsewhere")))),
    };
    let toasts = st.toasts.len();
    let fx = run(&mut st, msg);
    assert!(fx.iter().all(|e| !matches!(e, Effect::Forge { .. })));
    assert_eq!(st.toasts.len(), toasts, "only logged");
    assert!(st.log.last().is_some_and(|l| l.text == "Created issue #14 in other-repo"));
    assert!(!st.forge.as_ref().unwrap().cache.contains_key("14"));
    done(st, dir);
}

#[test]
fn the_dialog_is_parked_with_its_tab() {
    let (mut st, dir) = ready("issue-create-tabs");
    form_mut(&mut st).labels = vec!["bug".into()];
    update(&mut st, Msg::Tabs(TabsMsg::NewTab), Instant::now());
    assert!(st.modal.is_none(), "parked with its tab");
    assert!(run(&mut st, IssueMsg::Create).is_empty(), "nothing is sent from the other tab");
    let fx = update(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)), Instant::now());
    assert_eq!((form(&st).title.as_str(), &form(&st).labels), ("Crash on start", &vec!["bug".to_owned()]));
    assert_eq!(options_calls(&fx), [String::new()], "the choices still loading are asked again");

    // One being created isn't kept: its reply is only logged.
    run(&mut st, IssueMsg::Create);
    assert!(form(&st).busy);
    update(&mut st, Msg::Tabs(TabsMsg::SelectTab(1)), Instant::now());
    update(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)), Instant::now());
    assert!(st.modal.is_none());
    done(st, dir);
}

#[test]
fn calls_go_only_to_the_dialogs_tracker_and_a_new_one_closes_it() {
    let (mut st, dir) = ready("issue-create-tracker");
    st.forge.as_mut().unwrap().tracker = Some(Target { repo: "r2".into(), ..github() });
    assert!(creates(&run(&mut st, IssueMsg::Create)).is_empty());
    assert!(!form(&st).busy && form(&st).error.as_deref().is_some_and(|e| e.contains("changed")));
    form_mut(&mut st).options = OptionsState::Failed("timed out".into());
    assert!(options_calls(&run(&mut st, IssueMsg::SetProject(String::new()))).is_empty(), "Try again");
    assert!(matches!(&form(&st).options, OptionsState::Failed(e) if e.contains("changed")));
    st.forge.as_mut().unwrap().tracker = Some(github());

    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(st.modal.is_some(), "the same tracker keeps it");
    git(&dir, &["remote", "set-url", "origin", "https://github.com/o/r2"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert_eq!(st.forge.as_ref().unwrap().tracker.as_ref().map(|t| t.repo.as_str()), Some("r2"));
    assert!(st.modal.is_none(), "an edited remote closes it");
    done(st, dir);
}
