//! Preferences save-model snapshots: the Save / Revert bar under an edited Identity page (and
//! what its buttons push, Save waiting while a save is written, telling Esc when it has the
//! keyboard), a refused save's error on the page, the "Save your changes?" question, the
//! "Saved" tick beside a title, Git Flow's actions waiting while its form is edited, and the
//! Integrations page with an account connected, adding another, and with nothing connected.

use super::snapshot_tests::{harness, harness_sized, remove_fixture, settings, with_repo};
use super::snapshot_tests_gitflow::initialized;
use crate::msg::{Msg, PrefsMsg};
use crate::state::prefs::GitConfigValues;
use crate::state::{AppState, PrefPage, PrefsState};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui;
use egui_kittest::kittest::Queryable;
use glowfly_core::forge::{Account, Connection, Provider};
use glowfly_core::settings::ThemeChoice;
use std::time::Instant;

/// The repository's Identity page with its config read and the name edited.
fn identity_edited(name: &str) -> (AppState, std::path::PathBuf) {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, name);
    let mut prefs = PrefsState::new(PrefPage::RepoIdentity);
    let git = GitConfigValues { user_name: "Ada".into(), user_email: "ada@example.com".into(), ..Default::default() };
    prefs.git_saved = Some(git.clone());
    prefs.git = Some(GitConfigValues { repo_name: "Ada Lovelace".into(), ..git });
    st.prefs = Some(prefs);
    (st, dir)
}

fn pushed(msgs: &[Msg], want: fn(&Msg) -> bool) -> bool {
    msgs.iter().any(want)
}

#[test]
fn prefs_identity_unsaved() {
    let (st, dir) = identity_edited("prefs-identity-unsaved");
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("prefs_identity_unsaved");
    assert!(h.query_by_label("Unsaved changes").is_some());
    h.get_by_label("Revert").click();
    h.run();
    assert!(pushed(&h.state().1, |m| matches!(m, Msg::Prefs(PrefsMsg::RevertPage))));
    h.get_by_label("Save").click();
    h.run();
    assert!(pushed(&h.state().1, |m| matches!(m, Msg::Prefs(PrefsMsg::SavePage))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn the_bar_tells_esc_when_it_has_the_keyboard() {
    let (st, dir) = identity_edited("prefs-bar-focus");
    let mut h = harness(st, true);
    h.run();
    assert!(!h.state().0.view.prefs_bar_focused, "nothing focused yet");
    h.get_by_label("Revert").focus();
    h.run();
    assert!(h.state().0.view.prefs_bar_focused, "Esc now reverts");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn prefs_save_failed() {
    let (mut st, dir) = identity_edited("prefs-save-failed");
    let error = "error: could not lock config file .git/config: File exists";
    st.prefs.as_mut().unwrap().save_error = Some((PrefPage::RepoIdentity, error.into()));
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("prefs_save_failed");
    assert!(h.query_by_label(&format!("Not saved: {error}")).is_some(), "the error shows on the page");
    assert!(h.query_by_label("Saved").is_none(), "no tick");
    assert!(h.query_by_label("Unsaved changes").is_some(), "the edits wait on the bar");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn prefs_save_waits_while_written() {
    let (mut st, dir) = identity_edited("prefs-save-writing");
    let p = st.prefs.as_mut().unwrap();
    let written = p.git.clone();
    p.saving.push(crate::state::prefs_save::PendingSave { ticket: 1, page: PrefPage::RepoIdentity, written });
    let mut h = harness(st, true);
    h.run();
    h.get_by_label("Save").click();
    h.run();
    assert!(!pushed(&h.state().1, |m| matches!(m, Msg::Prefs(PrefsMsg::SavePage))), "Save waits for the write");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn prefs_unsaved_question() {
    let (mut st, dir) = identity_edited("prefs-unsaved-question");
    crate::update::update(&mut st, Msg::Prefs(PrefsMsg::Navigate(PrefPage::General)), Instant::now());
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("prefs_unsaved_question");
    assert!(h.query_by_label("Identity has changes that aren't saved yet.").is_some());
    h.get_by_label("Discard").click();
    h.run();
    assert!(pushed(&h.state().1, |m| matches!(m, Msg::ConfirmAlt)));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn prefs_saved_tick() {
    let mut st = AppState::new(settings(ThemeChoice::Dark), Vec::new(), Instant::now());
    let mut prefs = PrefsState::new(PrefPage::DiffViewer);
    prefs.saved.mark();
    prefs.saved.tick(Instant::now(), false);
    st.prefs = Some(prefs);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("prefs_saved_tick");
    assert!(h.query_by_label("Saved").is_some());
    assert!(h.query_by_label("Unsaved changes").is_none(), "app settings have no bar");
}

#[test]
fn gitflow_actions_wait_while_edited() {
    let (mut st, dir) = initialized(ThemeChoice::Dark, "gitflow-edited-wait");
    let mut prefs = PrefsState::new(PrefPage::RepoGitFlow);
    prefs.gitflow.fill(st.snap.as_ref().unwrap());
    prefs.gitflow.form.feature = "feat/".into();
    st.prefs = Some(prefs);
    let mut h = harness_sized(st, true, egui::vec2(1400.0, 1100.0));
    h.run();
    assert!(h.query_by_label("Save or revert your changes first.").is_some());
    assert!(h.query_by_label("Unsaved changes").is_some());
    assert!(h.query_by_label("Create missing branches").is_none(), "nothing is missing");
    for label in ["Start feature…", "Finish…"] {
        if let Some(b) = h.query_by_label(label) {
            b.click();
        }
    }
    h.run();
    assert!(h.state().1.iter().all(|m| !matches!(m, Msg::GitFlow(_))), "disabled while edited");
    drop(h);
    remove_fixture(&dir);
}

fn github(user: &str) -> Connection {
    let account = Account { provider: Provider::GitHub, host: "github.com".into(), org: String::new() };
    Connection { account, user: user.into(), method: "access token".into() }
}

fn integrations(connections: Vec<Connection>, adding: bool) -> AppState {
    let mut st = AppState::new(settings(ThemeChoice::Dark), Vec::new(), Instant::now());
    st.settings.connections = connections;
    let mut prefs = PrefsState::new(PrefPage::Integrations);
    prefs.int_provider = Provider::GitHub;
    prefs.int_host = "github.com".into();
    prefs.int_adding = adding;
    st.prefs = Some(prefs);
    st
}

#[test]
fn prefs_integrations_connected() {
    let mut h = harness_sized(integrations(vec![github("ada-l")], false), true, egui::vec2(1400.0, 900.0));
    h.run();
    h.check_snapshot("prefs_integrations_connected");
    assert!(h.query_by_label("Connected").is_some());
    assert!(h.query_by_label("ada-l via access token").is_some());
    assert!(h.query_by_label("Personal access token").is_none(), "the form waits for Add another account");
    assert!(h.query_by_label("Connected accounts").is_none());
    h.get_by_label("Add another account").click();
    h.run();
    assert!(pushed(&h.state().1, |m| matches!(m, Msg::Prefs(PrefsMsg::ShowAccountForm(true)))));
    h.get_by_label("Disconnect").click();
    h.run();
    assert!(pushed(&h.state().1, |m| matches!(m, Msg::Forge(crate::msg::ForgeMsg::Disconnect(_)))));
}

#[test]
fn prefs_integrations_adding() {
    let mut h = harness_sized(integrations(vec![github("ada-l")], true), true, egui::vec2(1400.0, 1000.0));
    h.run();
    h.check_snapshot("prefs_integrations_adding");
    assert!(h.query_by_label("Connect to GitHub").is_some());
    assert!(h.query_by_label("Add another account").is_none());
    h.get_by_label("Cancel").click();
    h.run();
    assert!(pushed(&h.state().1, |m| matches!(m, Msg::Prefs(PrefsMsg::ShowAccountForm(false)))));
}

#[test]
fn prefs_integrations_empty() {
    let mut h = harness_sized(integrations(Vec::new(), false), true, egui::vec2(1400.0, 1000.0));
    h.run();
    h.check_snapshot("prefs_integrations_empty");
    assert!(h.query_by_label("Connect to GitHub").is_some(), "the form shows at once");
    assert!(h.query_by_label("Cancel").is_none(), "nothing to go back to");
    assert!(h.query_by_label("Add another account").is_none());
}
