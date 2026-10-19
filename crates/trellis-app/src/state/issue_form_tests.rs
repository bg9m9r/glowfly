//! The Create issue dialog's form: the issue sent, the live checks, the loaded choices and
//! defaults, another project or board, and why the dialog can't be opened.

use super::*;
use crate::state::ForgeState;
use std::time::Instant;
use trellis_core::forge::{Account, Person};
use trellis_core::settings::Settings;

fn github() -> Target {
    Target { provider: Provider::GitHub, host: "github.com".into(), owner: "o".into(), project: String::new(), repo: "r".into() }
}

fn jira(project: &str) -> Target {
    Target::jira(&Account { provider: Provider::JiraCloud, host: "acme.atlassian.net".into(), org: String::new() }, project)
}

fn named(key: &str, title: &str) -> Named {
    Named { key: key.into(), title: title.into() }
}

fn person(id: &str) -> Person {
    Person { id: id.into(), name: id.to_uppercase() }
}

#[test]
fn labels_and_the_issue_sent() {
    let mut f = IssueForm::new(github());
    assert_eq!((f.title_label(), f.heading().as_str()), ("Title", "Create issue"));
    assert_eq!(IssueForm::new(jira("P")).title_label(), "Summary");
    assert_eq!(IssueForm::new(jira(" P ")).project, "P", "the project filter");
    f.title = "  Crash\r\n on   start \n".into();
    f.description = "\n Steps \n\n".into();
    f.labels = vec!["Bug".into(), "ui".into(), "bug".into(), " ".into()];
    let n = f.to_new();
    assert_eq!((n.title.as_str(), n.description.as_str()), ("Crash on   start", "Steps"));
    assert_eq!(n.labels, ["Bug", "ui"]);
}

#[test]
fn problems_include_the_assignee_maximum() {
    let mut f = IssueForm::new(github());
    assert_eq!(f.problems(), ["Enter a title."]);
    assert_eq!(f.submit_blocked().as_deref(), Some("Enter a title."));
    f.title = "Crash".into();
    f.people = vec!["a".into(), "b".into()];
    assert!(f.problems().is_empty(), "nothing known about the maximum yet");
    f.options_loaded(CreateOptions { people: vec![person("a"), person("b")], max_assignees: 1, ..Default::default() });
    assert_eq!(f.problems(), ["Only one assignee on this service"]);
    assert_eq!(too_many(10), "At most 10 assignees");
    f.people.pop();
    assert!(f.submit_blocked().is_none());
}

#[test]
fn loaded_options_pick_a_type_and_drop_what_is_not_offered() {
    let mut f = IssueForm::new(jira("P"));
    f.title = "Login".into();
    assert!(f.problems().is_empty(), "the type is asked for once the types are loaded");
    assert_eq!(f.submit_blocked().as_deref(), Some("Wait for the issue types to load."));
    f.people = vec!["gone".into(), "acc-1".into()];
    f.epic = Some("P-9".into());
    let opts = CreateOptions {
        types: vec![named("1", "Bug"), named("2", "Task"), named("3", "Epic")],
        people: vec![person("acc-1")],
        epics: vec![named("P-1", "Accounts")],
        labels_free: true,
        max_assignees: 1,
        ..Default::default()
    };
    f.options_loaded(opts.clone());
    assert_eq!(f.kind.as_deref(), Some("2"), "Task");
    assert_eq!((f.people.as_slice(), f.epic.as_deref()), (&["acc-1".to_owned()][..], None));
    assert!(f.epic_shown());
    f.epic = Some("P-1".into());
    assert_eq!(f.to_new().epic.as_deref(), Some("P-1"));
    f.kind = Some("3".into());
    assert!(!f.epic_shown(), "an epic has no epic");
    assert_eq!(f.to_new().epic, None);
    let mut f = IssueForm::new(jira("P"));
    f.options_loaded(CreateOptions { types: vec![named("7", "Story")], ..opts });
    assert_eq!(f.kind.as_deref(), Some("7"), "else the first");

    let mut gl = IssueForm::new(Target { provider: Provider::GitLab, host: "gitlab.com".into(), ..github() });
    gl.labels = vec!["new-one".into()];
    gl.options_loaded(CreateOptions {
        types: vec![named("incident", "Incident"), named("issue", "Issue")],
        labels_free: true,
        ..Default::default()
    });
    assert_eq!((gl.kind.as_deref(), gl.labels.len()), (Some("issue"), 1));
    let mut gh = IssueForm::new(github());
    gh.labels = vec!["BUG".into(), "made-up".into()];
    gh.options_loaded(CreateOptions { labels: vec!["bug".into()], ..Default::default() });
    assert_eq!((gh.kind, gh.labels), (None, vec!["BUG".to_owned()]), "only known labels on GitHub");
}

#[test]
fn another_project_clears_its_choices() {
    let mut f = IssueForm::new(jira("P"));
    f.kind = Some("2".into());
    f.epic = Some("P-1".into());
    f.people = vec!["acc-1".into()];
    f.labels = vec!["ui".into()];
    f.set_project(" Q ");
    assert_eq!((f.project.as_str(), f.project_text.as_str()), ("Q", "Q"));
    assert_eq!((f.kind, f.epic, f.people.len(), f.labels.len()), (None, None, 0, 1));
    assert_eq!(f.options, OptionsState::NotAsked);
}

#[test]
fn why_creating_is_blocked() {
    let mut st = AppState::new(Settings::default(), Vec::new(), Instant::now());
    assert_eq!(create_blocked(&st).as_deref(), Some("No issue tracker for this repository"));
    st.forge = Some(ForgeState::new(Some(github()), "origin".into(), None));
    assert_eq!(create_blocked(&st).as_deref(), Some("No issue tracker for this repository"));
    let bb = Target { provider: Provider::Bitbucket, host: "bitbucket.org".into(), ..github() };
    st.forge = Some(ForgeState::new(Some(bb.clone()), "origin".into(), Some(bb)));
    assert_eq!(create_blocked(&st), None, "Bitbucket Cloud issues");
    let az = Target { provider: Provider::AzureDevOps, host: "dev.azure.com".into(), ..github() };
    st.forge = Some(ForgeState::new(None, String::new(), Some(az)));
    assert_eq!(create_blocked(&st), None, "Azure Boards work items");
    let dc = Target { provider: Provider::BitbucketDataCenter, host: "bitbucket.acme.io".into(), ..github() };
    st.forge = Some(ForgeState::new(Some(dc), "origin".into(), None));
    assert_eq!(
        create_blocked(&st).as_deref(),
        Some("Bitbucket Data Center has no issue tracker of its own; choose Jira under Repo-specific preferences > Issue tracker")
    );
    let ghe = Target { provider: Provider::GitHubEnterprise, host: "git.acme.io".into(), ..github() };
    st.forge = Some(ForgeState::new(Some(ghe.clone()), "origin".into(), Some(ghe)));
    assert_eq!(create_blocked(&st).as_deref(), Some("Connect GitHub Enterprise Server at git.acme.io in Preferences first"));
    st.forge = Some(ForgeState::new(Some(github()), "origin".into(), Some(github())));
    assert_eq!(create_blocked(&st), None);
}

fn trello(board: &str) -> Target {
    let account = Account { provider: Provider::Trello, host: "trello.com".into(), org: String::new() };
    Target::trello(&account, board, if board.is_empty() { "" } else { "Website" })
}

fn azure() -> Target {
    Target {
        provider: Provider::AzureDevOps,
        host: "dev.azure.com".into(),
        owner: "contoso".into(),
        project: "Web".into(),
        repo: "r".into(),
    }
}

#[test]
fn default_type_and_priority_are_picked_and_kept() {
    let bb = Target { provider: Provider::Bitbucket, host: "bitbucket.org".into(), ..github() };
    let mut f = IssueForm::new(bb);
    assert_eq!((f.title_label(), f.heading().as_str()), ("Title", "Create issue"));
    let opts = CreateOptions {
        types: vec![named("bug", "Bug"), named("task", "Task")],
        priorities: vec![named("minor", "Minor"), named("major", "Major")],
        default_type: Some("bug".into()),
        default_priority: Some("major".into()),
        max_assignees: 1,
        ..Default::default()
    };
    f.options_loaded(opts.clone());
    assert_eq!((f.kind.as_deref(), f.priority.as_deref()), (Some("bug"), Some("major")));
    f.kind = Some("task".into());
    f.priority = Some("minor".into());
    f.options_loaded(opts.clone());
    assert_eq!((f.kind.as_deref(), f.priority.as_deref()), (Some("task"), Some("minor")), "a choice still offered is kept");
    f.title = "Crash".into();
    assert_eq!(f.to_new().priority.as_deref(), Some("minor"));
    assert!(f.submit_blocked().is_none(), "Bitbucket needs no type to wait for");
    let mut f = IssueForm::new(azure());
    f.options_loaded(CreateOptions { types: vec![named("Bug", "Bug"), named("Task", "Task")], default_type: Some("Gone".into()), ..opts });
    assert_eq!(f.kind.as_deref(), Some("Bug"), "a default that isn't offered: the first");
}

#[test]
fn trello_boards_and_lists() {
    let f = IssueForm::new(trello("5f1a2b3c4d5e6f7a8b9c0d1e"));
    assert_eq!(
        (f.project.as_str(), f.board_fixed(), f.title_label(), f.heading().as_str()),
        ("5f1a2b3c4d5e6f7a8b9c0d1e", true, "Name", "Create card")
    );
    let mut f = IssueForm::new(trello(""));
    assert!(!f.board_fixed() && f.project.is_empty());
    f.title = "Fix".into();
    assert_eq!(f.problems(), ["Choose a board."]);
    f.options_loaded(CreateOptions { boards: vec![named("b1", "Website"), named("b2", "Ops")], ..Default::default() });
    assert_eq!(f.boards.len(), 2, "boards come with the options");
    f.set_project("b1");
    f.options = OptionsState::Loading;
    assert_eq!(f.submit_blocked().as_deref(), Some("Wait for the lists to load."));
    f.options_loaded(CreateOptions {
        types: vec![named("l1", "To do"), named("l2", "Doing")],
        default_type: Some("l1".into()),
        labels: vec!["Bug".into()],
        people: vec![person("m1")],
        max_assignees: usize::MAX,
        ..Default::default()
    });
    assert_eq!((f.kind.as_deref(), f.boards.len()), (Some("l1"), 2), "the first list; the boards are kept");
    f.labels = vec!["Bug".into()];
    f.people = vec!["m1".into()];
    assert!(f.submit_blocked().is_none());
    assert_eq!((f.to_new().project.as_str(), f.to_new().kind.as_deref()), ("b1", Some("l1")));
    f.set_project("b2");
    assert_eq!((f.kind.as_ref(), f.labels.len(), f.people.len(), f.boards.len()), (None, 0, 0, 2), "labels are per board");
}

#[test]
fn azure_parents_and_waiting_for_types() {
    let mut f = IssueForm::new(azure());
    f.title = "Login".into();
    f.options = OptionsState::Loading;
    assert!(f.problems().is_empty());
    assert_eq!(f.submit_blocked().as_deref(), Some("Wait for the work item types to load."));
    f.options_loaded(CreateOptions {
        types: vec![named("Epic", "Epic"), named("User Story", "User Story")],
        default_type: Some("User Story".into()),
        epics: vec![named("1", "Epic · Accounts")],
        labels_free: true,
        max_assignees: 1,
        ..Default::default()
    });
    assert!(f.epic_shown());
    f.epic = Some("1".into());
    assert_eq!(f.to_new().epic.as_deref(), Some("1"));
    f.kind = Some("Epic".into());
    assert!(!f.epic_shown(), "an epic has no parent picker");
    assert_eq!(f.to_new().epic, None);
    f.kind = None;
    f.options_loaded(CreateOptions { types: Vec::new(), ..Default::default() });
    assert_eq!(f.problems(), ["Choose a work item type."]);
}
