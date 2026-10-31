//! Preferences > SSH snapshots for keys from outside the .ssh folder's pairs and for Windows'
//! OpenSSH: keys ~/.ssh/config names (with the entries that can't be listed), keys only the
//! agent holds, and the Use Windows' OpenSSH / Use Git's own ssh row. Built from state only.

use super::snapshot_tests::harness;
use super::snapshot_tests_ssh::{HOME, home_key, key_in, listing, ssh_add, ssh_mut, ssh_page, two_keys};
use crate::state::AppState;
use crate::state::ssh_use::Choices;
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::ThemeChoice;
use glowfly_core::ssh_keys::{
    AgentKey, AgentReach, AgentStatus, ConfigIdentity, ConfigStatus, KeyChoice, Unresolved, WinService, WinSshFacts,
};
use std::path::PathBuf;

const WIN_SSH: &str = "C:/Windows/System32/OpenSSH/ssh.exe";
const BUNDLED: &str = "C:/Program Files/Git/usr/bin/ssh.exe";

/// A harness tall enough for the whole page with the extra blocks.
fn taller(st: AppState, dark: bool) -> egui_kittest::Harness<'static, (AppState, Vec<crate::msg::Msg>)> {
    let mut h = harness(st, dark);
    h.set_size(eframe::egui::vec2(1400.0, 1400.0));
    h.run();
    h
}

fn entry(file: PathBuf, line: usize, section: &str, raw: &str, status: ConfigStatus) -> ConfigIdentity {
    ConfigIdentity { raw: raw.into(), file, line, section: section.into(), status }
}

#[test]
fn prefs_ssh_config_keys() {
    let ssh = PathBuf::from(HOME).join(".ssh");
    let config = ssh.join("config");
    let work = key_in(PathBuf::from(HOME).join("work"), "id_work", "ED25519", Some(256), Some("SHA256:w0rkK3yF1ngerpr1nt"), "ada@work");
    let mut keys = two_keys();
    keys.push(work.clone());
    let mut l = listing(keys, true);
    l.config = vec![
        entry(config.clone(), 4, "Host github.com", "~/.ssh/id_ed25519", ConfigStatus::Key(home_key("id_ed25519"))),
        entry(ssh.join("config.d").join("work"), 2, "Host work", "~/work/id_work", ConfigStatus::Key(work.private.clone())),
        entry(config.clone(), 9, "Host *.corp", "~/.ssh/%h", ConfigStatus::Unresolved(Unresolved::HostToken('h'))),
        entry(config.clone(), 12, "", "~/.ssh/old_key", ConfigStatus::Missing(home_key("old_key"))),
        entry(config, 15, "Match host build", "~/.ssh/ci", ConfigStatus::PublicOnly(home_key("ci"))),
    ];
    let st = ssh_page(ThemeChoice::Dark, Some(l));
    let mut h = taller(st, true);
    assert_eq!(h.query_all_by_label("In ssh config").count(), 2);
    assert_eq!(h.query_all_by_label("Remove from list").count(), 0, "config keys aren't remembered");
    for note in [
        "~/.ssh/%h (config, line 9, Host *.corp): depends on the host it connects to (%h)",
        "~/.ssh/old_key (config, line 12): there is no file there",
        "~/.ssh/ci (config, line 15, Match host build): only ci.pub is there; ssh uses it to pick that key from the agent",
    ] {
        assert!(h.query_by_label(note).is_some(), "{note}");
    }
    assert!(h.query_by_label_contains("Also named in").is_some());
    h.check_snapshot("prefs_ssh_config_keys");

    h.get_by_label("All repositories: Let ssh decide").click();
    h.run();
    assert!(h.query_by_label("id_work  ED25519").is_some(), "a config key can be chosen");
}

fn agent_key(fingerprint: &str, comment: &str, kind: &str, bits: u32, public_key: Option<&str>) -> AgentKey {
    AgentKey {
        bits: Some(bits),
        fingerprint: fingerprint.into(),
        comment: comment.into(),
        kind: kind.into(),
        public_key: public_key.map(str::to_owned),
    }
}

#[test]
fn prefs_ssh_agent_only() {
    let mut st = ssh_page(ThemeChoice::Light, Some(listing(two_keys(), true)));
    let held = vec![
        agent_key("SHA256:3n9kV2qXcP0yLw8bTfR6uHjA1sZmDe4oGiNtYxKvQ7c", "ada@example.com", "ED25519", 256, None),
        agent_key(
            "SHA256:Yk4mQ8rVt2LwPz6nHc1sXa9bJd3eFg7iKo5uNyRqTwE",
            "yubikey-backup",
            "ED25519",
            256,
            Some("ssh-ed25519 AAAAyubi yubikey-backup"),
        ),
        agent_key("SHA256:Hq2nW7cLx5Rv8pTz1mBk4sYd6gJf3aNe9uKo0iPrXbM", "", "RSA", 3072, None),
    ];
    ssh_mut(&mut st).agent = Some(AgentStatus {
        ssh_add: ssh_add(),
        reach: Some(AgentReach::Keys(held)),
        service: Some(WinService::Running),
        ..Default::default()
    });
    let mut h = taller(st, false);
    assert_eq!(h.query_all_by_label("Agent only").count(), 2);
    assert!(h.query_by_label("Only in the agent").is_some());
    assert_eq!(h.query_all_by_label("Copy public key").count(), 4);
    assert_eq!(h.query_all_by_label("Copy public key").filter(|n| n.is_disabled()).count(), 1, "the agent gave no public line");
    assert_eq!(h.query_all_by_label("In agent").count(), 1, "id_ed25519 matches the agent's first key");
    h.check_snapshot("prefs_ssh_agent_only");

    h.get_by_label("All repositories: Let ssh decide").click();
    h.run();
    assert!(h.query_by_label("id_rsa_work  RSA 3072").is_some());
    assert!(h.query_by_label_contains("yubikey-backup  ").is_none(), "agent-only keys can't be chosen");
}

/// The SSH page with the agent service running and the Windows facts found, the global
/// core.sshCommand read as `global` (paths are labels only: nothing is resolved).
fn windows_page(theme: ThemeChoice, global: KeyChoice, version: (u32, u32)) -> AppState {
    let mut st = ssh_page(theme, Some(listing(two_keys(), true)));
    let p = ssh_mut(&mut st);
    p.choice = Some(Choices { global: Ok(global), repo: None });
    let facts =
        WinSshFacts { windows: Some(WIN_SSH.into()), windows_version: Some(version), bundled: Some(BUNDLED.into()), ..Default::default() };
    p.agent = Some(AgentStatus {
        ssh_add: ssh_add(),
        reach: Some(AgentReach::Keys(vec![])),
        service: Some(WinService::Running),
        win_ssh: Some(facts),
        ..Default::default()
    });
    st
}

#[test]
fn prefs_ssh_windows_offer() {
    let st = windows_page(ThemeChoice::Dark, KeyChoice::Unset, (9, 5));
    let mut h = taller(st, true);
    assert!(!h.get_by_label("Use Windows' OpenSSH").is_disabled(), "offered");
    assert!(h.query_by_label_contains("git runs Git for Windows' own ssh").is_some());
    assert!(h.query_by_label_contains("older than 8.4").is_none());
    h.check_snapshot("prefs_ssh_windows_offer");
}

#[test]
fn windows_offer_blocked_by_a_chosen_key() {
    let st = windows_page(ThemeChoice::Dark, KeyChoice::Key(home_key("id_ed25519")), (9, 5));
    let h = taller(st, true);
    assert!(h.get_by_label("Use Windows' OpenSSH").is_disabled());
    assert!(h.query_by_label("A key is chosen for all repositories. Choose Let ssh decide in Which key first.").is_some());
}

#[test]
fn prefs_ssh_windows_on() {
    let st = windows_page(ThemeChoice::Light, KeyChoice::WindowsSsh(WIN_SSH.into()), (7, 7));
    let mut h = taller(st, false);
    assert!(h.query_by_label("Use Git's own ssh").is_some_and(|n| !n.is_disabled()));
    assert!(h.query_by_label("All repositories: Windows' OpenSSH").is_some());
    assert!(h.query_by_label_contains("Windows' OpenSSH 7.7 is older than 8.4").is_some());
    assert!(h.query_by_label_contains("so ssh reads keys from the agent service").is_some());
    h.check_snapshot("prefs_ssh_windows_on");
}
