//! Preferences > SSH snapshots: two key pairs and a filled form, no keys without ssh-keygen,
//! the form's warnings, the folder still being read, the key git uses and the agent, and the Add
//! to service form. Built from state only: no real home folder, git config, agent or ssh-keygen.

use super::snapshot_tests::{harness, render, settings};
use crate::state::ssh_service::{UploadForm, UploadState};
use crate::state::ssh_use::{Choices, RepoTarget};
use crate::state::{AppState, PrefPage, PrefsState};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::forge::{Account, Connection, Provider};
use trellis_core::settings::ThemeChoice;
use trellis_core::ssh_keys::{AgentKey, AgentReach, AgentStatus, KeyChoice, SshKeyPair, SshListing, WinService};

const HOME: &str = "C:/Users/ada";

fn key(name: &str, kind: &str, bits: Option<u32>, fingerprint: Option<&str>, comment: &str) -> SshKeyPair {
    let dir = PathBuf::from(HOME).join(".ssh");
    SshKeyPair {
        private: dir.join(name),
        public: dir.join(format!("{name}.pub")),
        key_type: if kind == "RSA" { "ssh-rsa".into() } else { "ssh-ed25519".into() },
        kind: kind.into(),
        bits,
        fingerprint: fingerprint.map(str::to_owned),
        comment: comment.into(),
        public_key: format!("ssh-ed25519 AAAA {comment}"),
    }
}

fn two_keys() -> Vec<SshKeyPair> {
    vec![
        key("id_ed25519", "ED25519", Some(256), Some("SHA256:3n9kV2qXcP0yLw8bTfR6uHjA1sZmDe4oGiNtYxKvQ7c"), "ada@example.com"),
        key("id_rsa_work", "RSA", Some(3072), Some("SHA256:Zp5LrW0eVb7nQk2HsXy9MfTc4GaUoJd1iCtBq8NwE3s"), ""),
    ]
}

/// Preferences open on the SSH page with `listing` (None: still reading).
fn ssh_page(theme: ThemeChoice, listing: Option<SshListing>) -> AppState {
    let mut st = AppState::new(settings(theme), Vec::new(), Instant::now());
    let mut prefs = PrefsState::new(PrefPage::Ssh);
    prefs.ssh.generation = 1;
    prefs.ssh.listing = listing;
    prefs.ssh.fill_location();
    // Nothing chosen and no agent, unless a test says otherwise (spinners would keep repainting).
    prefs.ssh.choice = Some(Choices { global: Ok(KeyChoice::Unset), repo: None });
    prefs.ssh.agent = Some(AgentStatus {
        ssh_add: Some(PathBuf::from("C:/Windows/System32/OpenSSH/ssh-add.exe")),
        reach: Some(AgentReach::NoAgent),
        service: Some(WinService::Stopped),
        ..Default::default()
    });
    st.prefs = Some(prefs);
    st
}

/// A window tall enough for the whole page, form included.
fn render_tall(name: &str, st: AppState, dark: bool) {
    let mut h = harness(st, dark);
    h.set_size(eframe::egui::vec2(1400.0, 1180.0));
    h.run();
    h.check_snapshot(name);
}

fn listing(keys: Vec<SshKeyPair>, keygen: bool) -> SshListing {
    SshListing {
        home: HOME.into(),
        dir: PathBuf::from(HOME).join(".ssh"),
        keys,
        keygen: keygen.then(|| PathBuf::from("C:/Windows/System32/OpenSSH/ssh-keygen.exe")),
    }
}

#[test]
fn prefs_ssh() {
    let mut st = ssh_page(ThemeChoice::Dark, Some(listing(two_keys(), true)));
    let p = &mut st.prefs.as_mut().unwrap().ssh;
    p.location = "C:/Users/ada/.ssh/id_ed25519_new".into();
    p.prefill_comment("ada@example.com");
    render_tall("prefs_ssh", st, true);
}

#[test]
fn prefs_ssh_empty() {
    let st = ssh_page(ThemeChoice::Light, Some(listing(vec![], false)));
    assert!(!st.prefs.as_ref().unwrap().ssh.can_generate());
    render("prefs_ssh_empty", st, false);
}

#[test]
fn prefs_ssh_form_errors() {
    let mut st = ssh_page(ThemeChoice::Dark, Some(listing(two_keys(), true)));
    let p = &mut st.prefs.as_mut().unwrap().ssh;
    p.location = "C:/Users/ada/.ssh/id_ed25519".into();
    p.comment = "ada@example.com".into();
    p.passphrase = "one".into();
    p.confirm = "two".into();
    p.form_error = Some("ssh-keygen failed: Saving key failed: Permission denied".into());
    render_tall("prefs_ssh_form_errors", st, true);
}

#[test]
fn prefs_ssh_loading() {
    let mut st = ssh_page(ThemeChoice::Dark, None);
    let p = &mut st.prefs.as_mut().unwrap().ssh;
    (p.choice, p.agent) = (None, None);
    // The spinner keeps repainting; a fixed number of frames (egui's clock advances by a fixed
    // step) keeps the image reproducible.
    let mut h = harness(st, true);
    h.run_steps(4);
    h.check_snapshot("prefs_ssh_loading");
}

fn ssh_mut(st: &mut AppState) -> &mut crate::state::ssh::SshPage {
    &mut st.prefs.as_mut().unwrap().ssh
}

fn home_key(name: &str) -> PathBuf {
    PathBuf::from(HOME).join(".ssh").join(name)
}

#[test]
fn prefs_ssh_git_choice() {
    let mut st = ssh_page(ThemeChoice::Dark, Some(listing(two_keys(), true)));
    let p = ssh_mut(&mut st);
    p.repo = Some(RepoTarget { name: "trellis".into(), workdir: "C:/src/trellis".into(), real: "C:/src/trellis".into(), trusted: true });
    p.choice = Some(Choices {
        global: Ok(KeyChoice::Key(home_key("id_ed25519"))),
        repo: Some(Ok(KeyChoice::Other { value: "ssh -v".into(), key: None })),
    });
    let work = AgentKey {
        bits: Some(3072),
        fingerprint: "SHA256:Zp5LrW0eVb7nQk2HsXy9MfTc4GaUoJd1iCtBq8NwE3s".into(),
        comment: "work".into(),
        kind: "RSA".into(),
    };
    p.agent = Some(AgentStatus {
        ssh_add: Some(PathBuf::from("C:/Windows/System32/OpenSSH/ssh-add.exe")),
        reach: Some(AgentReach::Keys(vec![work])),
        service: Some(WinService::Running),
        ..Default::default()
    });
    let mut h = harness(st, true);
    h.run();
    // One set of buttons per key; Stop using for each level that is set.
    for (label, count) in [("Use for git…", 2), ("Add to agent", 2), ("Add to service…", 2), ("Stop using", 2), ("Use local SSH agent", 1)]
    {
        assert_eq!(h.query_all_by_label(label).count(), count, "{label}");
    }
    assert!(h.query_all_by_label("Add to agent").all(|n| !n.is_disabled()), "an agent is running");
    assert!(h.query_by_label_contains("Set outside Trellis: ssh -v").is_some());
    assert!(h.query_by_label_contains("Agent running: 1 key loaded").is_some());
    h.check_snapshot("prefs_ssh_git_choice");
}

#[test]
fn prefs_ssh_agent_off() {
    let mut st = ssh_page(ThemeChoice::Light, Some(listing(two_keys(), true)));
    st.settings.use_ssh_agent = true;
    let p = ssh_mut(&mut st);
    p.agent = Some(AgentStatus {
        ssh_add: Some(PathBuf::from("C:/Windows/System32/OpenSSH/ssh-add.exe")),
        reach: Some(AgentReach::NoAgent),
        service: Some(WinService::Disabled),
        git_ssh_env: Some("GIT_SSH_COMMAND".into()),
        git_ssh_command_set: true,
        ..Default::default()
    });
    let mut h = harness(st, false);
    h.run();
    assert!(h.query_all_by_label("Use for git…").all(|n| n.is_disabled()), "the agent setting is on");
    assert!(h.query_all_by_label("Add to agent").all(|n| n.is_disabled()), "no agent");
    assert!(h.query_by_label_contains("is disabled (an administrator").is_some());
    assert!(h.query_by_label_contains("GIT_SSH_COMMAND is set").is_some());
    h.check_snapshot("prefs_ssh_agent_off");
}

#[test]
fn prefs_ssh_no_ssh_add() {
    let mut st = ssh_page(ThemeChoice::Dark, Some(listing(two_keys(), true)));
    ssh_mut(&mut st).agent = Some(AgentStatus { auth_sock_set: false, ..Default::default() });
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label_contains("ssh-add wasn't found").is_some());
    h.check_snapshot("prefs_ssh_no_ssh_add");
}

fn with_form(theme: ThemeChoice, state: UploadState, title: &str) -> AppState {
    let mut st = ssh_page(theme, Some(listing(two_keys(), true)));
    let github = Account { provider: Provider::GitHub, host: "github.com".into(), org: String::new() };
    st.settings.connections = vec![Connection { account: github.clone(), user: "ada".into(), method: "token".into() }];
    let mut form = UploadForm::new(home_key("id_ed25519"), github, "ada@example.com".into());
    form.title = title.into();
    form.state = state;
    ssh_mut(&mut st).upload = Some(form);
    st
}

#[test]
fn prefs_ssh_add_to_service() {
    let st = with_form(ThemeChoice::Dark, UploadState::Editing, "Work laptop");
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("Add key").is_some_and(|n| !n.is_disabled()));
    h.check_snapshot("prefs_ssh_add_to_service");
}

#[test]
fn prefs_ssh_service_fallback() {
    let message = crate::state::ssh_service::fallback_note("GitHub");
    let st = with_form(ThemeChoice::Light, UploadState::Failed { message, fallback: true }, "");
    let mut h = harness(st, false);
    h.run();
    assert!(h.query_by_label("Copy key and open settings").is_some());
    h.check_snapshot("prefs_ssh_service_fallback");
}

#[test]
fn prefs_ssh_service_already() {
    let st = with_form(ThemeChoice::Dark, UploadState::AlreadyThere("Laptop".into()), "");
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label_contains("GitHub already has this key").is_some());
    h.check_snapshot("prefs_ssh_service_already");
}
