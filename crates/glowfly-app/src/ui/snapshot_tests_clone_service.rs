//! The Clone dialog's Your repositories tab: a loaded list with two accounts to choose from, a
//! filter, private tags and a picked repository; no connected account; a sign-in error with
//! Retry; and the list loading.

use super::snapshot_tests::{harness, render, settings};
use crate::effect::ForgeError;
use crate::state::clone::CloneForm;
use crate::state::clone_service::{CloneSource, ServicePicker};
use crate::state::{AppState, Modal};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use glowfly_core::forge::{Account, Connection, HostedRepo, MyRepos, Provider};
use glowfly_core::settings::ThemeChoice;
use std::time::Instant;

fn account(provider: Provider, host: &str) -> Account {
    Account { provider, host: host.into(), org: String::new() }
}

fn repo(full: &str, private: bool) -> HostedRepo {
    HostedRepo {
        owner: full.split('/').next().unwrap().into(),
        full_name: full.into(),
        https_url: format!("https://github.com/{full}.git"),
        ssh_url: format!("git@github.com:{full}.git"),
        web_url: String::new(),
        private,
    }
}

fn with_picker(theme: ThemeChoice, service: ServicePicker) -> AppState {
    let mut st = AppState::new(settings(theme), Vec::new(), Instant::now());
    st.settings.connections =
        service.accounts.iter().map(|a| Connection { account: a.clone(), user: "ada".into(), method: "access token".into() }).collect();
    let checked = Some((r"C:\src".into(), Some(r"C:\src".into())));
    let mut form = CloneForm { source: CloneSource::Service, parent_checked: checked, ..CloneForm::new(r"C:\src".into()) };
    if let Some(r) = service.picked.and_then(|i| service.list.repos.get(i)) {
        form.url = r.https_url.clone();
    }
    form.service = ServicePicker { filled: form.url.clone(), ..service };
    st.modal = Some(Modal::Clone(form));
    st
}

fn accounts() -> Vec<Account> {
    vec![account(Provider::GitHub, "github.com"), account(Provider::GitLab, "gitlab.com")]
}

#[test]
fn clone_service_list() {
    let list = MyRepos {
        repos: vec![repo("ada/app", false), repo("team/api", true), repo("ada/notes", true), repo("team/data", false)],
        truncated: false,
    };
    let loaded = Some(accounts()[0].clone());
    let p = ServicePicker { accounts: accounts(), loaded, list, filter: "a".into(), picked: Some(1), ..ServicePicker::default() };
    render("clone_service_list", with_picker(ThemeChoice::Dark, p), true);
}

#[test]
fn clone_service_no_accounts() {
    render("clone_service_no_accounts", with_picker(ThemeChoice::Light, ServicePicker::default()), false);
}

#[test]
fn clone_service_error() {
    let error = Some(ForgeError { message: "GitHub didn't accept the saved token.".into(), auth: true });
    let p = ServicePicker { accounts: accounts(), loaded: Some(accounts()[0].clone()), error, ..ServicePicker::default() };
    render("clone_service_error", with_picker(ThemeChoice::Dark, p), true);
}

#[test]
fn clone_service_loading() {
    let p = ServicePicker {
        accounts: vec![account(Provider::GitHub, "github.com")],
        loading: true,
        loaded: Some(account(Provider::GitHub, "github.com")),
        ..ServicePicker::default()
    };
    // The spinner keeps repainting, so a fixed number of frames.
    let mut h = harness(with_picker(ThemeChoice::Light, p), false);
    h.run_steps(3);
    assert!(h.query_by_label("Loading your repositories…").is_some());
    h.check_snapshot("clone_service_loading");
}
