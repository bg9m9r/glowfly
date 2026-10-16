//! The Create issue dialog: GitHub with labels and an assignee chosen, Jira with its project,
//! type and epic and a missing summary, and creating then failing with the server's words.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use crate::msg::{IssueMsg, Msg};
use crate::state::issue_form::{IssueForm, OptionsState};
use crate::state::{AppState, ForgeState, Modal, ProjectKeys, Status};
use crate::ui::snapshot_check::CheckSnapshot;
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
    });
    form.epic = Some("PROJ-1".into());
    let (st, dir) = with_form(ThemeChoice::Dark, "issue-create-jira", form, |fs| {
        fs.jira_projects = ProjectKeys::Known(["PROJ".to_owned(), "OPS".to_owned()].into_iter().collect());
    });
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("create_issue_jira_dark");
    assert!(h.query_by_label("Enter a summary.").is_some());
    assert!(h.get_by_role_and_label(Role::Button, "Create issue").is_disabled());
    assert!(h.query_by_label("Summary").is_some() && h.query_by_label("Epic").is_some());
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
    assert!(h.query_by_label("Enter a title.").is_some());
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
