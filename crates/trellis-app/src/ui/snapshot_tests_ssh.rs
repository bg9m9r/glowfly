//! Preferences > SSH snapshots: two key cards with their marks, no keys without ssh-keygen, the
//! folder still being read, the Which key dropdowns (closed and open), the agent pill's states (Checking among them),
//! a key added with Use existing, and the Add to service form. The Generate SSH key dialog is in
//! `snapshot_tests_ssh_keygen`. Built from state only: no real home folder, git config, agent
//! or ssh-keygen.

use super::snapshot_tests::{harness, settings};
use crate::state::ssh_service::{UploadForm, UploadState};
use crate::state::ssh_use::{Choices, RepoTarget};
use crate::state::{AppState, PrefPage, PrefsState};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::forge::{Account, Connection, Provider};
use trellis_core::settings::ThemeChoice;
use trellis_core::ssh_keys::{AgentKey, AgentReach, AgentStatus, KeyChoice, KeyFileProblem, SshKeyPair, SshListing, WinService};

/// A home folder that is absolute on every OS (a Windows path is relative elsewhere, and the
/// Generate dialog refuses a relative save location).
pub(super) const HOME: &str = if cfg!(windows) { "C:/Users/ada" } else { "/Users/ada" };

fn key_in(dir: PathBuf, name: &str, kind: &str, bits: Option<u32>, fingerprint: Option<&str>, comment: &str) -> SshKeyPair {
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

fn key(name: &str, kind: &str, bits: Option<u32>, fingerprint: Option<&str>, comment: &str) -> SshKeyPair {
    key_in(PathBuf::from(HOME).join(".ssh"), name, kind, bits, fingerprint, comment)
}

pub(super) fn two_keys() -> Vec<SshKeyPair> {
    vec![
        key("id_ed25519", "ED25519", Some(256), Some("SHA256:3n9kV2qXcP0yLw8bTfR6uHjA1sZmDe4oGiNtYxKvQ7c"), "ada@example.com"),
        key("id_rsa_work", "RSA", Some(3072), Some("SHA256:Zp5LrW0eVb7nQk2HsXy9MfTc4GaUoJd1iCtBq8NwE3s"), ""),
    ]
}

fn ssh_add() -> Option<PathBuf> {
    Some(PathBuf::from("C:/Windows/System32/OpenSSH/ssh-add.exe"))
}

/// Preferences open on the SSH page with `listing` (None: still reading).
pub(super) fn ssh_page(theme: ThemeChoice, listing: Option<SshListing>) -> AppState {
    let mut st = AppState::new(settings(theme), Vec::new(), Instant::now());
    let mut prefs = PrefsState::new(PrefPage::Ssh);
    prefs.ssh.generation = 1;
    prefs.ssh.listing = listing;
    prefs.ssh.fill_location();
    // Nothing chosen and no agent, unless a test says otherwise (spinners would keep repainting).
    prefs.ssh.choice = Some(Choices { global: Ok(KeyChoice::Unset), repo: None });
    prefs.ssh.agent = Some(AgentStatus {
        ssh_add: ssh_add(),
        reach: Some(AgentReach::NoAgent),
        service: Some(WinService::Stopped),
        ..Default::default()
    });
    st.prefs = Some(prefs);
    st
}

/// A harness with a window tall enough for the whole page.
fn tall(st: AppState, dark: bool) -> egui_kittest::Harness<'static, (AppState, Vec<crate::msg::Msg>)> {
    let mut h = harness(st, dark);
    h.set_size(eframe::egui::vec2(1400.0, 1180.0));
    h.run();
    h
}

pub(super) fn listing(keys: Vec<SshKeyPair>, keygen: bool) -> SshListing {
    SshListing {
        home: HOME.into(),
        dir: PathBuf::from(HOME).join(".ssh"),
        keys,
        keygen: keygen.then(|| PathBuf::from("C:/Windows/System32/OpenSSH/ssh-keygen.exe")),
    }
}

fn ssh_mut(st: &mut AppState) -> &mut crate::state::ssh::SshPage {
    &mut st.prefs.as_mut().unwrap().ssh
}

fn home_key(name: &str) -> PathBuf {
    PathBuf::from(HOME).join(".ssh").join(name)
}

fn trellis_repo() -> RepoTarget {
    RepoTarget { name: "trellis".into(), workdir: "C:/src/trellis".into(), real: "C:/src/trellis".into(), trusted: true }
}

#[test]
fn prefs_ssh() {
    let st = ssh_page(ThemeChoice::Dark, Some(listing(two_keys(), true)));
    let mut h = tall(st, true);
    assert_eq!(h.query_all_by_label("Use for git…").count(), 0);
    assert_eq!(h.query_all_by_label("Copy public key").count(), 2);
    assert_eq!(h.query_all_by_label("Tried by default").count(), 1, "id_ed25519 only, not id_rsa_work");
    for label in ["Generate…", "Use existing…", "All repositories: Let ssh decide", "Off"] {
        assert!(h.query_by_label(label).is_some(), "{label}");
    }
    h.check_snapshot("prefs_ssh");
}

#[test]
fn prefs_ssh_light() {
    let mut st = ssh_page(ThemeChoice::Light, Some(listing(two_keys(), true)));
    let p = ssh_mut(&mut st);
    p.repo = Some(trellis_repo());
    p.choice =
        Some(Choices { global: Ok(KeyChoice::Key(home_key("id_ed25519"))), repo: Some(Ok(KeyChoice::Key(home_key("id_rsa_work")))) });
    let mut h = tall(st, false);
    for label in ["All repositories: id_ed25519", "trellis only: id_rsa_work", "Default", "Used in trellis"] {
        assert!(h.query_by_label(label).is_some(), "{label}");
    }
    assert_eq!(h.query_all_by_label("Tried by default").count(), 0, "a key is chosen for all repositories");
    h.check_snapshot("prefs_ssh_light");
}

#[test]
fn prefs_ssh_empty() {
    let st = ssh_page(ThemeChoice::Light, Some(listing(vec![], false)));
    let mut h = harness(st, false);
    h.run();
    assert!(h.get_by_label("Generate…").is_disabled(), "no ssh-keygen");
    h.check_snapshot("prefs_ssh_empty");
}

#[test]
fn prefs_ssh_loading() {
    let mut st = ssh_page(ThemeChoice::Dark, None);
    let p = ssh_mut(&mut st);
    (p.choice, p.agent) = (None, None);
    // The spinner keeps repainting; a fixed number of frames (egui's clock advances by a fixed
    // step) keeps the image reproducible.
    let mut h = harness(st, true);
    h.run_steps(4);
    h.check_snapshot("prefs_ssh_loading");
}

#[test]
fn prefs_ssh_git_choice() {
    let mut st = ssh_page(ThemeChoice::Dark, Some(listing(two_keys(), true)));
    let p = ssh_mut(&mut st);
    p.repo = Some(trellis_repo());
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
        ssh_add: ssh_add(),
        reach: Some(AgentReach::Keys(vec![work])),
        service: Some(WinService::Running),
        ..Default::default()
    });
    let mut h = tall(st, true);
    for (label, count) in [("Add to agent", 2), ("Add to service…", 2), ("Stop using", 0), ("Use local SSH agent", 1), ("In agent", 1)] {
        assert_eq!(h.query_all_by_label(label).count(), count, "{label}");
    }
    assert!(h.query_all_by_label("Add to agent").all(|n| !n.is_disabled()), "an agent is running");
    assert!(h.query_by_label_contains("Set outside Trellis: ssh -v").is_some());
    assert!(h.query_by_label("trellis only: Custom command").is_some());
    assert!(h.query_by_label("Running, 1 key").is_some());
    h.check_snapshot("prefs_ssh_git_choice");
}

#[test]
fn prefs_ssh_agent_off() {
    let mut st = ssh_page(ThemeChoice::Light, Some(listing(two_keys(), true)));
    st.settings.use_ssh_agent = true;
    ssh_mut(&mut st).agent = Some(AgentStatus {
        ssh_add: ssh_add(),
        reach: Some(AgentReach::NoAgent),
        service: Some(WinService::Disabled),
        git_ssh_env: Some("GIT_SSH_COMMAND".into()),
        git_ssh_command_set: true,
        ..Default::default()
    });
    let mut h = tall(st, false);
    assert!(h.query_all_by_label("Add to agent").all(|n| n.is_disabled()), "no agent");
    assert!(h.query_by_label("Off").is_some());
    assert!(h.query_by_label_contains("Turn off Use local SSH agent").is_some());
    assert!(h.query_by_label_contains("is disabled (an administrator").is_some());
    assert!(h.query_by_label_contains("GIT_SSH_COMMAND is set").is_some());
    h.check_snapshot("prefs_ssh_agent_off");
}

#[test]
fn prefs_ssh_agent_checking() {
    let mut st = ssh_page(ThemeChoice::Dark, Some(listing(two_keys(), true)));
    ssh_mut(&mut st).agent = None;
    let mut h = tall(st, true);
    assert!(h.query_by_label("Checking…").is_some(), "the pill while the agent is checked");
    h.check_snapshot("prefs_ssh_agent_checking");
}

#[test]
fn prefs_ssh_no_ssh_add() {
    let mut st = ssh_page(ThemeChoice::Dark, Some(listing(two_keys(), true)));
    ssh_mut(&mut st).agent = Some(AgentStatus { auth_sock_set: false, ..Default::default() });
    let mut h = tall(st, true);
    assert!(h.query_by_label("Not reachable").is_some());
    assert!(h.query_by_label_contains("ssh-add wasn't found").is_some());
    h.check_snapshot("prefs_ssh_no_ssh_add");
}

#[test]
fn prefs_ssh_extra_key() {
    let deploy_dir = PathBuf::from(HOME).join("keys");
    let deploy = key_in(deploy_dir.clone(), "deploy_key", "ED25519", Some(256), Some("SHA256:dEpL0yK3y"), "ci@example.com");
    let mut keys = two_keys();
    keys.push(deploy);
    let mut st = ssh_page(ThemeChoice::Dark, Some(listing(keys, true)));
    let p = ssh_mut(&mut st);
    p.choice = Some(Choices { global: Ok(KeyChoice::Key(deploy_dir.join("deploy_key"))), repo: None });
    p.pick_error = Some(("work_key".into(), KeyFileProblem::NoPublicKey));
    let mut h = tall(st, true);
    assert_eq!(h.query_all_by_label("Remove from list").count(), 1, "only the key outside .ssh");
    assert_eq!(h.query_all_by_label("Default").count(), 1);
    assert!(h.query_by_label("All repositories: deploy_key").is_some());
    assert!(h.query_by_label_contains("work_key has no work_key.pub next to it").is_some());
    h.check_snapshot("prefs_ssh_extra_key");
}

#[test]
fn prefs_ssh_which_key_open() {
    let st = ssh_page(ThemeChoice::Light, Some(listing(two_keys(), true)));
    let mut h = tall(st, false);
    h.get_by_label("All repositories: Let ssh decide").click();
    h.run();
    for label in ["Let ssh decide", "id_ed25519  ED25519", "id_rsa_work  RSA 3072"] {
        assert!(h.query_by_label(label).is_some(), "{label}");
    }
    h.check_snapshot("prefs_ssh_which_key_open");
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
    let mut h = tall(st, true);
    assert!(h.query_by_label("Add key").is_some_and(|n| !n.is_disabled()));
    h.check_snapshot("prefs_ssh_add_to_service");
}

#[test]
fn prefs_ssh_service_fallback() {
    let message = crate::state::ssh_service::fallback_note(trellis_core::i18n::Lang::En, "GitHub");
    let st = with_form(ThemeChoice::Light, UploadState::Failed { message, fallback: true }, "");
    let mut h = tall(st, false);
    assert!(h.query_by_label("Copy key and open settings").is_some());
    h.check_snapshot("prefs_ssh_service_fallback");
}

#[test]
fn prefs_ssh_service_already() {
    let st = with_form(ThemeChoice::Dark, UploadState::AlreadyThere("Laptop".into()), "");
    let mut h = tall(st, true);
    assert!(h.query_by_label_contains("GitHub already has this key").is_some());
    h.check_snapshot("prefs_ssh_service_already");
}
