//! Headless tests for the keys that come from outside the .ssh folder's pairs: copying an
//! agent-only key's public line, and choosing, forgetting and reloading keys ~/.ssh/config names.

use super::ssh_tests::{home, listing, loaded, page, page_mut, pair, ssh};
use crate::effect::{Effect, UiEffect};
use crate::ssh::{SshEffect, SshMsg};
use crate::state::ssh::COPIED;
use crate::state::ssh_use::Choices;
use glowfly_core::ssh_keys::{AgentKey, AgentReach, AgentStatus, ConfigIdentity, ConfigStatus, KeyChoice, SshKeyPair};

fn agent_key(fingerprint: &str, public_key: Option<&str>) -> AgentKey {
    AgentKey {
        bits: Some(256),
        fingerprint: fingerprint.into(),
        comment: "agent".into(),
        kind: "ED25519".into(),
        public_key: public_key.map(str::to_owned),
    }
}

fn copies(fx: &[Effect]) -> Vec<String> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Ui(UiEffect::CopyText(t)) => Some(t.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn copies_an_agent_only_key() {
    let mut st = loaded(vec![pair("id_ed25519", "ada")], true);
    let keys = vec![agent_key("SHA256:a", Some("ssh-ed25519 AAAAa agent")), agent_key("SHA256:b", None)];
    page_mut(&mut st).agent = Some(AgentStatus { reach: Some(AgentReach::Keys(keys)), ..Default::default() });

    let fx = ssh(&mut st, SshMsg::CopyAgentKey("SHA256:a".into()));
    assert_eq!(copies(&fx), ["ssh-ed25519 AAAAa agent"]);
    assert_eq!(st.toasts.last().map(|t| (t.ok, t.text.as_str())), Some((true, COPIED)));

    let fx = ssh(&mut st, SshMsg::CopyAgentKey("SHA256:b".into()));
    assert!(copies(&fx).is_empty());
    let last = st.toasts.last().expect("a toast");
    assert!(!last.ok && last.text.contains("didn't give this key's public key"), "{}", last.text);

    let toasts = st.toasts.len();
    assert!(ssh(&mut st, SshMsg::CopyAgentKey("SHA256:gone".into())).is_empty());
    assert_eq!(st.toasts.len(), toasts, "an unknown key does nothing");
}

/// A key outside the .ssh folder that ~/.ssh/config names (not added with Use existing).
fn config_key() -> SshKeyPair {
    let dir = home().join("work");
    SshKeyPair { private: dir.join("id_work"), public: dir.join("id_work.pub"), ..pair("id_work", "work") }
}

fn with_config(st: &mut crate::state::AppState) {
    let key = config_key();
    let l = page_mut(st).listing.as_mut().unwrap();
    l.config = vec![ConfigIdentity {
        raw: "~/work/id_work".into(),
        file: home().join(".ssh").join("config"),
        line: 2,
        section: "Host work".into(),
        status: ConfigStatus::Key(key.private),
    }];
}

#[test]
fn a_config_key_can_be_chosen_but_not_forgotten() {
    let mut st = loaded(vec![pair("id_ed25519", "ada"), config_key()], true);
    with_config(&mut st);
    page_mut(&mut st).choice = Some(Choices { global: Ok(KeyChoice::Unset), repo: None });
    let fx = ssh(&mut st, SshMsg::UseKey { key: config_key().private, repo: false, confirmed: false });
    assert!(
        fx.iter().any(|e| matches!(e, Effect::Ssh(SshEffect::SetKeyChoice { repo: None, key: Some(k) }) if *k == config_key().private))
    );

    let before = page(&st).listing.clone();
    let fx = ssh(&mut st, SshMsg::ForgetKey(config_key().private));
    assert!(fx.is_empty(), "not remembered: nothing to save");
    assert!(st.settings.ssh_extra_keys.is_empty());
    assert_eq!(page(&st).listing, before);

    // Remembered too: Remove from list forgets it, but the config still lists it.
    st.settings.ssh_extra_keys = vec![config_key().private];
    let fx = ssh(&mut st, SshMsg::ForgetKey(config_key().private));
    assert!(fx.iter().any(|e| matches!(e, Effect::SaveSettings(_))));
    assert!(st.settings.ssh_extra_keys.is_empty());
    assert_eq!(page(&st).listing.as_ref().unwrap().keys.len(), 2, "the config names it");
}

#[test]
fn loaded_keeps_the_config_entries() {
    let mut st = loaded(vec![], true);
    let mut l = listing(vec![config_key()], true);
    l.config = vec![ConfigIdentity {
        raw: "~/.ssh/%h".into(),
        file: home().join(".ssh").join("config"),
        line: 7,
        section: String::new(),
        status: ConfigStatus::Unresolved(glowfly_core::ssh_keys::Unresolved::HostToken('h')),
    }];
    l.config_error = Some("boom".into());
    let generation = page(&st).generation;
    ssh(&mut st, SshMsg::Loaded { generation, result: Ok(l.clone()) });
    assert_eq!(page(&st).listing.as_ref(), Some(&l));
}
