//! The marks on a key card and the agent pill's states.

use super::*;
use crate::state::ssh_use::{Choices, RepoTarget};
use std::path::{Path, PathBuf};
use trellis_core::ssh_keys::AgentKey;

fn home() -> PathBuf {
    trellis_core::paths::temp_dir().join("trellis-ssh-marks")
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
    let listing = SshListing { home: home(), dir, keys, keygen: None };
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

    page.repo = Some(RepoTarget { name: "trellis".into(), workdir: home(), real: home(), trusted: true });
    page.choice = Some(Choices { global: Ok(KeyChoice::Key(ed)), repo: Some(Ok(KeyChoice::Key(rsa))) });
    assert_eq!(marks(&page, &listing, 1), [KeyMark::Repo("trellis".into())]);

    page.choice = Some(Choices { global: Ok(KeyChoice::Unset), repo: None });
    assert_eq!(marks(&page, &listing, 0), [KeyMark::TriedByDefault]);
    assert_eq!(marks(&page, &listing, 1), [KeyMark::TriedByDefault]);
    assert!(marks(&page, &listing, 2).is_empty(), "outside the .ssh folder");

    let held = AgentKey { bits: None, fingerprint: "SHA256:work_key".into(), comment: String::new(), kind: "ED25519".into() };
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
    let key = || AgentKey { bits: None, fingerprint: "SHA256:x".into(), comment: String::new(), kind: "ED25519".into() };
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
