//! The Create issue dialog: GitHub with labels and an assignee chosen, Jira with its project,
//! type and epic and a missing summary (shown once Create is tried), an Azure Boards work item with its type, tags and
//! parent, Trello's board and write-token note, Bitbucket's kind and priority, and creating
//! then failing with the server's words.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use crate::msg::{IssueMsg, Msg};
use crate::state::issue_form::{IssueForm, OptionsState};
use crate::state::{AppState, ForgeState, Modal, ProjectKeys, Status};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::Key;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use std::path::PathBuf;
use trellis_core::forge::filters::Named;
use trellis_core::forge::{Account, CreateOptions, Person, Provider, Target};
use trellis_core::settings::ThemeChoice;

fn github() -> Target {
    Target { provider: Provider::GitHub, host: "github.com".into(), owner: "acme".into(), project: String::new(), repo: "demo".into() }
}

fn person(id: &str, name: &str) -> Person {
    Person { id: id.into(), name: name.into() }
}

fn named(key: &str, title: &str) -> Named {
    Named { key: key.into(), title: title.into() }
}

fn with_form(theme: ThemeChoice, name: &str, form: IssueForm, f: impl FnOnce(&mut ForgeState)) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    // The ISSUES header's + is a "Create issue" button too.
    st.settings.show_left_panel = false;
    let mut fs = ForgeState::new(Some(github()), "origin".into(), Some(form.target.clone()));
    fs.host_status = Status::Ok;
    fs.tracker_status = Status::Ok;
    f(&mut fs);
    st.forge = Some(fs);
    st.modal = Some(Modal::CreateIssue(Box::new(form)));
    (st, dir)
}

fn github_form() -> IssueForm {
    let mut form = IssueForm::new(github());
    form.options_loaded(CreateOptions {
        labels: vec!["bug".into(), "docs".into(), "ui".into()],
        people: vec![person("ada", "ada"), person("linus", "linus")],
        max_assignees: 10,
        ..Default::default()
    });
    form.title = "Crash on start when the config folder is missing".into();
    form.description = "Starting without ~/.config/demo panics.".into();
    form.labels = vec!["bug".into(), "ui".into()];
    form.people = vec!["ada".into()];
    form.then_branch = true;
    form
}

#[test]
fn create_issue_github() {
    let (st, dir) = with_form(ThemeChoice::Light, "issue-create-gh", github_form(), |_| {});
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("create_issue_github");
    assert!(!h.get_by_role_and_label(Role::Button, "Create issue").is_disabled());
    assert!(h.query_by_label("Loading labels, people and types…").is_none());
    h.get_by_role_and_label(Role::Button, "Create issue").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Issue(IssueMsg::Create))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn create_issue_jira_dark() {
    let account = Account { provider: Provider::JiraCloud, host: "acme.atlassian.net".into(), org: String::new() };
    let mut form = IssueForm::new(Target::jira(&account, "PROJ"));
    form.options_for = "PROJ".into();
    form.options_loaded(CreateOptions {
        labels: vec!["auth".into()],
        people: vec![person("acc-1", "Ada Lovelace")],
        types: vec![named("10001", "Bug"), named("10002", "Task"), named("10003", "Epic")],
        epics: vec![named("PROJ-1", "Accounts and sign-in"), named("PROJ-2", "Search")],
        labels_free: true,
        max_assignees: 1,
        ..Default::default()
    });
    form.epic = Some("PROJ-1".into());
    let (st, dir) = with_form(ThemeChoice::Dark, "issue-create-jira", form, |fs| {
        fs.jira_projects = ProjectKeys::Known(["PROJ".to_owned(), "OPS".to_owned()].into_iter().collect());
    });
    let mut h = harness(st, true);
    h.run();
    assert_eq!(h.query_all_by_label("Enter a summary.").count(), 1, "nobody typed a summary yet: only the footer says it");
    // Enter on the disabled Create shows it, under the Summary field.
    h.press_key(Key::Enter);
    h.run();
    h.check_snapshot("create_issue_jira_dark");
    assert!(h.query_by_label("Enter a summary.").is_some());
    assert!(h.get_by_role_and_label(Role::Button, "Create issue").is_disabled());
    // Each label is shown and names its field.
    assert!(h.query_all_by_label("Summary").next().is_some() && h.query_all_by_label("Epic").next().is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn create_issue_busy_error() {
    let mut form = github_form();
    form.busy = true;
    let (st, dir) = with_form(ThemeChoice::Dark, "issue-create-busy", form, |_| {});
    let mut h = harness(st, true);
    h.run();
    assert!(h.get_by_label("Creating…").is_disabled());
    assert!(!h.get_by_label("Cancel").is_disabled(), "Cancel stays");
    if let Some(Modal::CreateIssue(f)) = h.state_mut().0.modal.as_mut() {
        f.busy = false;
        f.error = Some("the server said \u{201c}Validation Failed: assignees is invalid\u{201d}".into());
    }
    h.run();
    h.check_snapshot("create_issue_busy_error");
    assert!(h.query_by_label_contains("Couldn't create the issue: the server said").is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn loading_choices_and_a_failed_load() {
    let mut form = IssueForm::new(github());
    form.options = OptionsState::Loading;
    let (st, dir) = with_form(ThemeChoice::Dark, "issue-create-loading", form, |_| {});
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("Loading labels, people and types…").is_some());
    assert_eq!(h.query_all_by_label("Enter a title.").count(), 1, "nobody typed a title yet: only the footer says it");
    if let Some(Modal::CreateIssue(f)) = h.state_mut().0.modal.as_mut() {
        f.options = OptionsState::Failed("timed out".into());
    }
    h.run();
    h.get_by_label("Try again").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Issue(IssueMsg::SetProject(p)) if p.is_empty())));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn create_issue_azure_dark() {
    let azure = Target {
        provider: Provider::AzureDevOps,
        host: "dev.azure.com".into(),
        owner: "contoso".into(),
        project: "Fabrikam".into(),
        repo: "web".into(),
    };
    let mut form = IssueForm::new(azure);
    form.options_loaded(CreateOptions {
        labels: vec!["ux".into(), "backend".into()],
        people: vec![person("ada@contoso.com", "Ada Lovelace"), person("grace@contoso.com", "Grace Hopper")],
        types: ["Bug", "Epic", "Feature", "Task", "User Story"].map(|t| named(t, t)).to_vec(),
        default_type: Some("User Story".into()),
        epics: vec![named("1", "Epic · Accounts"), named("2", "Feature · Search")],
        labels_free: true,
        max_assignees: 1,
        ..Default::default()
    });
    form.labels = vec!["ux".into(), "sign-in".into()];
    form.people = vec!["ada@contoso.com".into()];
    form.epic = Some("1".into());
    let (st, dir) = with_form(ThemeChoice::Dark, "issue-create-azure", form, |_| {});
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("create_issue_azure_dark");
    assert_eq!(h.query_all_by_label("Create work item").count(), 2, "the heading and the button");
    assert!(h.get_by_role_and_label(Role::Button, "Create work item").is_disabled(), "no title yet");
    for label in ["Type", "Tags", "Assigned to", "Parent"] {
        assert!(h.query_by_label(label).is_some(), "{label}");
    }
    if let Some(Modal::CreateIssue(f)) = h.state_mut().0.modal.as_mut() {
        f.title = "Sign in with a work account".into();
    }
    h.run();
    assert!(!h.get_by_role_and_label(Role::Button, "Create work item").is_disabled());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn trello_asks_for_a_board_and_a_write_token() {
    let account = Account { provider: Provider::Trello, host: "trello.com".into(), org: String::new() };
    let mut form = IssueForm::new(Target::trello(&account, "", ""));
    let url = "https://trello.com/1/authorize?expiration=never&scope=read,write&response_type=token&name=Trellis&key=k";
    form.options_loaded(CreateOptions {
        boards: vec![named("5f1a2b3c4d5e6f7a8b9c0d1e", "Website")],
        write_token_url: Some(url.into()),
        ..Default::default()
    });
    form.title = "Fix login".into();
    let (st, dir) = with_form(ThemeChoice::Dark, "issue-create-trello", form, |_| {});
    let mut h = harness(st, true);
    h.run();
    for label in ["Board", "Name", "Members", "Creating cards needs a Trello token allowed to read and write."] {
        assert!(h.query_all_by_label(label).next().is_some(), "{label}");
    }
    assert!(h.query_all_by_label("Labels").next().is_none(), "no labels before a board");
    assert!(h.get_by_role_and_label(Role::Button, "Create card").is_disabled());
    assert_eq!(h.query_all_by_label("Choose a board.").count(), 1, "nothing tried yet: only the footer says it");
    h.press_key(Key::Enter);
    h.run();
    assert_eq!(h.query_all_by_label("Choose a board.").count(), 1, "a submit shows it under its field, and only there");
    h.get_by_label("Allow a write token").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::OpenUrl(u) if u.contains("scope=read,write"))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn bitbucket_has_kind_and_priority_but_no_labels() {
    let bb = Target { provider: Provider::Bitbucket, host: "bitbucket.org".into(), ..github() };
    let mut form = IssueForm::new(bb);
    form.options_loaded(CreateOptions {
        people: vec![person("{a}", "Ada")],
        types: vec![named("bug", "Bug"), named("task", "Task")],
        priorities: vec![named("minor", "Minor"), named("major", "Major")],
        default_type: Some("bug".into()),
        default_priority: Some("major".into()),
        max_assignees: 1,
        ..Default::default()
    });
    let (st, dir) = with_form(ThemeChoice::Light, "issue-create-bitbucket", form, |_| {});
    let mut h = harness(st, true);
    h.run();
    for label in ["Kind", "Priority", "Assignees"] {
        assert!(h.query_by_label(label).is_some(), "{label}");
    }
    assert!(h.query_by_label("Labels").is_none());
    drop(h);
    remove_fixture(&dir);
}
