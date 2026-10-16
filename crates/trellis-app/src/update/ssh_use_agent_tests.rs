//! Headless tests for Add to agent (Preferences > SSH): when it may run, what it starts, and
//! what its result does (toast and re-check, log, leftover prompts cancelled).

use super::ssh_tests::{loaded, page, page_mut, pair, ssh};
use crate::effect::Effect;
use crate::msg::{CredentialMsg, Msg};
use crate::ssh::{SshEffect, SshMsg};
use crate::state::{AppState, RunningJob};
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::askpass::{Prompt, Reply};
use trellis_core::ssh_keys::{AgentReach, AgentStatus, SshKeyPair};

fn key() -> SshKeyPair {
    pair("id_ed25519", "ada")
}

fn ssh_add() -> PathBuf {
    PathBuf::from("C:/Windows/System32/OpenSSH/ssh-add.exe")
}

/// The page with `key()` listed and the agent check's result.
fn with_agent(status: Option<AgentStatus>) -> AppState {
    let mut st = loaded(vec![key()], true);
    page_mut(&mut st).agent = status;
    st
}

fn running() -> AgentStatus {
    AgentStatus { ssh_add: Some(ssh_add()), reach: Some(AgentReach::Keys(vec![])), ..Default::default() }
}

fn adds(fx: &[Effect]) -> Vec<(PathBuf, PathBuf)> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Ssh(SshEffect::AddToAgent { ssh_add, key }) => Some((ssh_add.clone(), key.clone())),
            _ => None,
        })
        .collect()
}

#[test]
fn add_to_agent_is_refused_with_a_reason() {
    let refused = |mut st: AppState, why: &str| {
        let fx = ssh(&mut st, SshMsg::AddToAgent(key().private));
        assert!(adds(&fx).is_empty(), "{why}");
        assert!(page(&st).agent_error.as_deref().is_some_and(|e| e.contains(why)), "{why}: {:?}", page(&st).agent_error);
    };
    refused(with_agent(None), "Wait until the agent");
    refused(with_agent(Some(AgentStatus { ssh_add: None, ..running() })), "ssh-add wasn't found");
    refused(with_agent(Some(AgentStatus { reach: Some(AgentReach::NoAgent), ..running() })), "No SSH agent");

    let mut st = with_agent(Some(running()));
    st.job = Some(RunningJob {
        label: "Pull".into(),
        workdir: None,
        quiet: false,
        clear_commit: false,
        then_open: None,
        then_push: false,
        keeps_undo: false,
    });
    refused(st, "Wait until Pull finishes");

    let mut st = with_agent(Some(running()));
    page_mut(&mut st).agent_adding = Some(key().private);
    refused(st, "being added");
}

#[test]
fn add_to_agent_runs_ssh_add() {
    let mut st = with_agent(Some(running()));
    let fx = ssh(&mut st, SshMsg::AddToAgent(key().private));
    assert_eq!(adds(&fx), [(ssh_add(), key().private)]);
    assert_eq!(page(&st).agent_adding, Some(key().private));
    assert!(ssh(&mut st, SshMsg::AddToAgent(PathBuf::from("C:/elsewhere/id"))).is_empty(), "not listed");

    let fx = ssh(&mut st, SshMsg::AgentAdded { key: key().private, result: Ok(()) });
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("Added id_ed25519 to the SSH agent"));
    assert!(page(&st).agent_adding.is_none() && page(&st).agent.is_none(), "checked again");
    assert!(fx.iter().any(|e| matches!(e, Effect::Ssh(SshEffect::Agent { .. }))));
}

#[test]
fn a_failed_add_is_logged_and_leftover_prompts_are_cancelled() {
    let mut st = with_agent(Some(running()));
    ssh(&mut st, SshMsg::AddToAgent(key().private));
    let prompt = Prompt::new("Enter passphrase for C:/h/.ssh/id_ed25519: ", None);
    crate::update::update(&mut st, Msg::Credentials(CredentialMsg::CredentialPrompt { id: 9, prompt }), Instant::now());
    assert_eq!(st.askpass.queue.len(), 1, "the passphrase dialog is up");

    let fx = ssh(&mut st, SshMsg::AgentAdded { key: key().private, result: Err("Error loading key: incorrect passphrase".into()) });
    assert!(fx.iter().any(|e| matches!(e, Effect::AnswerPrompt { id: 9, reply: Reply::Cancel })), "{fx:?}");
    assert!(st.askpass.queue.is_empty());
    let entry = st.log.last().expect("logged");
    assert_eq!(entry.text, "Could not add id_ed25519 to the SSH agent: Error loading key: incorrect passphrase");
    assert!(page(&st).agent_adding.is_none());
}
