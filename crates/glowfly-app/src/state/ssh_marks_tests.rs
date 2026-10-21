//! The marks on a key card and the agent pill's states.

use super::*;
use crate::state::ssh_use::{Choices, RepoTarget};
use glowfly_core::ssh_keys::AgentKey;
use std::path::{Path, PathBuf};

fn home() -> PathBuf {
    glowfly_core::paths::temp_dir().join("glowfly-ssh-marks")
}

fn key(dir: &Path, name: &str) -> SshKeyPair {
    SshKeyPair {
        private: dir.join(name),
        public: dir.join(format!("{name}.pub")),
        key_type: "ssh-ed25519".into(),
        kind: "ED25519".into(),
        bits: None,
        fingerprint: Some(format!("SHA256:{name}")),
        comment: String::new(),
        public_key: String::new(),
    }
}

fn setup() -> (SshPage, SshListing) {
    let dir = home().join(".ssh");
    let keys = vec![key(&dir, "id_ed25519"), key(&dir, "id_rsa"), key(&home().join("keys"), "work_key")];
    let listing = SshListing { home: home(), dir, keys, ..Default::default() };
    let page = SshPage { listing: Some(listing.clone()), ..Default::default() };
    (page, listing)
}

fn marks(page: &SshPage, listing: &SshListing, i: usize) -> Vec<KeyMark> {
    key_marks(page, listing, &listing.keys[i])
}

#[test]
fn key_marks_rules() {
    let (mut page, listing) = setup();
    let (ed, rsa) = (listing.keys[0].private.clone(), listing.keys[1].private.clone());
    page.choice = Some(Choices { global: Ok(KeyChoice::Key(ed.clone())), repo: None });
    assert_eq!(marks(&page, &listing, 0), [KeyMark::Default]);
    assert!((0..3).all(|i| !marks(&page, &listing, i).contains(&KeyMark::TriedByDefault)), "a key is chosen");

    page.repo = Some(RepoTarget { name: "glowfly".into(), workdir: home(), real: home(), trusted: true });
    page.choice = Some(Choices { global: Ok(KeyChoice::Key(ed)), repo: Some(Ok(KeyChoice::Key(rsa))) });
    assert_eq!(marks(&page, &listing, 1), [KeyMark::Repo("glowfly".into())]);

    page.choice = Some(Choices { global: Ok(KeyChoice::Unset), repo: None });
    assert_eq!(marks(&page, &listing, 0), [KeyMark::TriedByDefault]);
    assert_eq!(marks(&page, &listing, 1), [KeyMark::TriedByDefault]);
    assert!(marks(&page, &listing, 2).is_empty(), "outside the .ssh folder");

    let held =
        AgentKey { bits: None, fingerprint: "SHA256:work_key".into(), comment: String::new(), kind: "ED25519".into(), public_key: None };
    page.agent = Some(AgentStatus { reach: Some(AgentReach::Keys(vec![held])), ..Default::default() });
    assert_eq!(marks(&page, &listing, 2), [KeyMark::InAgent]);

    page.choice = None;
    assert!(marks(&page, &listing, 0).is_empty(), "nothing before the choice is read");
    assert!(!outside_folder(&listing.keys[0], &listing));
    assert!(outside_folder(&listing.keys[2], &listing));
}

#[test]
fn agent_pill_states() {
    let with = |reach: AgentReach| AgentStatus { ssh_add: Some("ssh-add".into()), reach: Some(reach), ..Default::default() };
    let key = || AgentKey { bits: None, fingerprint: "SHA256:x".into(), comment: String::new(), kind: "ED25519".into(), public_key: None };
    assert_eq!(agent_pill(None), AgentPill::Checking);
    assert_eq!(agent_pill(Some(&with(AgentReach::Keys(vec![key(), key()])))), AgentPill::Running(2));
    assert_eq!(agent_pill(Some(&with(AgentReach::Keys(vec![])))), AgentPill::Running(0));
    assert_eq!(agent_pill(Some(&AgentStatus::default())), AgentPill::Unreachable, "no ssh-add");
    assert_eq!(agent_pill(Some(&with(AgentReach::Failed("boom".into())))), AgentPill::Unreachable);
    let no_agent = with(AgentReach::NoAgent);
    assert_eq!(agent_pill(Some(&AgentStatus { auth_sock_set: true, ..no_agent.clone() })), AgentPill::Unreachable);
    assert_eq!(agent_pill(Some(&AgentStatus { service: Some(WinService::Running), ..no_agent.clone() })), AgentPill::Unreachable);
    assert_eq!(agent_pill(Some(&AgentStatus { pageant: true, ..no_agent.clone() })), AgentPill::Unreachable);
    assert_eq!(agent_pill(Some(&AgentStatus { service: Some(WinService::Stopped), ..no_agent })), AgentPill::Off);
}

fn agent_with(keys: Vec<AgentKey>) -> AgentStatus {
    AgentStatus { reach: Some(AgentReach::Keys(keys)), ..Default::default() }
}

fn agent_key(fingerprint: &str, public_key: Option<&str>) -> AgentKey {
    AgentKey {
        bits: None,
        fingerprint: fingerprint.into(),
        comment: String::new(),
        kind: "ED25519".into(),
        public_key: public_key.map(str::to_owned),
    }
}

#[test]
fn config_marks_and_the_windows_ssh_choice() {
    use glowfly_core::ssh_keys::{ConfigIdentity, ConfigStatus};
    let (mut page, mut listing) = setup();
    let work = listing.keys[2].private.clone();
    let config = home().join(".ssh").join("config");
    let entry = |line: usize, section: &str, status: ConfigStatus| ConfigIdentity {
        raw: "~/keys/work_key".into(),
        file: config.clone(),
        line,
        section: section.into(),
        status,
    };
    listing.config = vec![
        entry(2, "Host github.com", ConfigStatus::Key(work.clone())),
        entry(5, "", ConfigStatus::Missing(home().join("gone"))),
        entry(9, "Host gitlab.com", ConfigStatus::Key(work)),
    ];
    let places = vec![
        ConfigWhere { file: config.clone(), line: 2, section: "Host github.com".into() },
        ConfigWhere { file: config, line: 9, section: "Host gitlab.com".into() },
    ];
    page.choice = Some(Choices { global: Ok(KeyChoice::WindowsSsh("C:/Windows/System32/OpenSSH/ssh.exe".into())), repo: None });
    assert_eq!(marks(&page, &listing, 2), [KeyMark::InConfig(places)]);
    assert_eq!(marks(&page, &listing, 0), [KeyMark::TriedByDefault], "Windows' OpenSSH tries the default names too");

    // ssh-keygen missing: no fingerprint, matched by the public key body.
    let mut unprinted = listing.keys[1].clone();
    unprinted.fingerprint = None;
    unprinted.public_key = "ssh-ed25519 AAAAbody rsa".into();
    listing.keys[1] = unprinted;
    page.agent = Some(agent_with(vec![agent_key("SHA256:other", Some("ssh-ed25519 AAAAbody agent comment"))]));
    assert_eq!(marks(&page, &listing, 1), [KeyMark::TriedByDefault, KeyMark::InAgent]);
}

#[test]
fn remembered_keys_and_agent_only_keys() {
    let (mut page, listing) = setup();
    let extra = vec![home().join("keys").join("work_key")];
    assert!(remembered(&listing.keys[2], &extra));
    assert!(!remembered(&listing.keys[0], &extra), "in the folder, not added with Use existing");
    assert!(!remembered(&listing.keys[2], &[]), "outside the folder but not remembered (named in the config)");

    assert!(agent_only(&page).is_empty(), "no agent check yet");
    let mut by_body = listing.keys[1].clone();
    by_body.fingerprint = None;
    by_body.public_key = "ssh-ed25519 AAAAbody x".into();
    let mut listing = listing;
    listing.keys[1] = by_body;
    page.listing = Some(listing);
    page.agent = Some(agent_with(vec![
        agent_key("SHA256:id_ed25519", None),
        agent_key("SHA256:body", Some("ssh-ed25519 AAAAbody agent")),
        agent_key("SHA256:lonely", Some("ssh-ed25519 AAAAlonely agent")),
    ]));
    let only: Vec<&str> = agent_only(&page).iter().map(|k| k.fingerprint.as_str()).collect();
    assert_eq!(only, ["SHA256:lonely"]);
    page.listing = None;
    assert!(agent_only(&page).is_empty(), "no listing yet");
}
