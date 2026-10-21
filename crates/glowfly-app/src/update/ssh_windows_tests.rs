//! Headless tests for Use Windows' OpenSSH and its undo (Preferences > SSH, Agent), and how
//! Which key and Use local SSH agent treat the value it writes.

use super::ssh_tests::{loaded, page, page_mut, pair, ssh};
use crate::effect::Effect;
use crate::ssh::{SshEffect, SshMsg};
use crate::state::AppState;
use crate::state::ssh_use::Choices;
use glowfly_core::ssh_keys::{AgentReach, AgentStatus, KeyChoice, WinService, WinSshFacts};
use std::path::PathBuf;

const WIN: &str = "C:/Windows/System32/OpenSSH/ssh.exe";

/// The SSH page with one key listed, the agent service running with Git's bundled ssh in use,
/// and the global core.sshCommand read as `global`.
fn windows_page(global: KeyChoice) -> AppState {
    let mut st = loaded(vec![pair("id_ed25519", "ada")], true);
    let p = page_mut(&mut st);
    let facts =
        WinSshFacts { windows: Some(WIN.into()), bundled: Some("C:/Program Files/Git/usr/bin/ssh.exe".into()), ..Default::default() };
    p.agent = Some(AgentStatus {
        ssh_add: Some("ssh-add".into()),
        reach: Some(AgentReach::Keys(vec![])),
        service: Some(WinService::Running),
        win_ssh: Some(facts),
        ..Default::default()
    });
    p.choice = Some(Choices { global: Ok(global), repo: None });
    st
}

fn windows_sets(fx: &[Effect]) -> Vec<Option<PathBuf>> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Ssh(SshEffect::SetWindowsSsh { program }) => Some(program.clone()),
            _ => None,
        })
        .collect()
}

fn key_sets(fx: &[Effect]) -> Vec<(Option<PathBuf>, Option<PathBuf>)> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Ssh(SshEffect::SetKeyChoice { repo, key }) => Some((repo.clone(), key.clone())),
            _ => None,
        })
        .collect()
}

#[test]
fn switching_writes_once_and_reads_the_choice_again() {
    let mut st = windows_page(KeyChoice::Unset);
    let fx = ssh(&mut st, SshMsg::UseWindowsSsh(true));
    assert_eq!(windows_sets(&fx), [Some(PathBuf::from(WIN))]);
    assert!(page(&st).choice_busy && !page(&st).choice_repo);

    let fx = ssh(&mut st, SshMsg::UseWindowsSsh(true));
    assert!(fx.is_empty(), "one write at a time");
    assert_eq!(page(&st).windows_error.as_deref(), Some("core.sshCommand is being changed."));

    let fx = ssh(&mut st, SshMsg::WindowsSshSet { on: true, result: Ok(()) });
    assert!(!page(&st).choice_busy);
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("git now runs Windows' OpenSSH for SSH remotes"));
    assert!(fx.iter().any(|e| matches!(e, Effect::Ssh(SshEffect::ReadChoice { .. }))));
    assert!(page(&st).choice.is_none(), "read again");
}

#[test]
fn a_failed_write_is_shown_and_logged() {
    let mut st = windows_page(KeyChoice::Unset);
    ssh(&mut st, SshMsg::UseWindowsSsh(true));
    let fx = ssh(&mut st, SshMsg::WindowsSshSet { on: true, result: Err("could not lock config file".into()) });
    assert!(fx.iter().all(|e| !matches!(e, Effect::Ssh(SshEffect::ReadChoice { .. }))));
    assert!(!page(&st).choice_busy);
    assert_eq!(page(&st).windows_error.as_deref(), Some("could not lock config file"));
    assert!(st.log.iter().any(|l| l.text.contains("Could not change core.sshCommand: could not lock config file")));
}

#[test]
fn refused_unless_offered() {
    let mut blocked = windows_page(KeyChoice::Key("C:/h/.ssh/id_ed25519".into()));
    assert!(ssh(&mut blocked, SshMsg::UseWindowsSsh(true)).is_empty());
    assert!(page(&blocked).windows_error.as_deref().is_some_and(|e| e.starts_with("A key is chosen")));
    let mut hidden = windows_page(KeyChoice::Unset);
    page_mut(&mut hidden).agent.as_mut().unwrap().win_ssh = None;
    assert!(ssh(&mut hidden, SshMsg::UseWindowsSsh(true)).is_empty());
    assert!(!page(&hidden).choice_busy);
    let mut unset = windows_page(KeyChoice::Unset);
    assert!(ssh(&mut unset, SshMsg::UseWindowsSsh(false)).is_empty(), "nothing to undo");
}

#[test]
fn undo_unsets_the_global_value() {
    let mut st = windows_page(KeyChoice::WindowsSsh(WIN.into()));
    let fx = ssh(&mut st, SshMsg::UseWindowsSsh(false));
    assert_eq!(windows_sets(&fx), [None]);
    ssh(&mut st, SshMsg::WindowsSshSet { on: false, result: Ok(()) });
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("git runs Git for Windows' own ssh again"));
}

#[test]
fn which_key_and_the_agent_setting_treat_it_as_trellis_wrote_it() {
    let mut st = windows_page(KeyChoice::WindowsSsh(WIN.into()));
    let fx = ssh(&mut st, SshMsg::SetUseAgent(true));
    assert!(key_sets(&fx).is_empty(), "the agent setting leaves Windows' OpenSSH alone");

    let mut st = windows_page(KeyChoice::WindowsSsh(WIN.into()));
    let fx = ssh(&mut st, SshMsg::StopUsing { repo: false, confirmed: false });
    assert_eq!(key_sets(&fx), [(None, None)]);
    assert!(st.modal.is_none(), "no confirmation");

    let mut st = windows_page(KeyChoice::WindowsSsh(WIN.into()));
    let key = pair("id_ed25519", "ada").private;
    let fx = ssh(&mut st, SshMsg::UseKey { key: key.clone(), repo: false, confirmed: false });
    assert_eq!(key_sets(&fx), [(None, Some(key))]);
    assert!(st.modal.is_none(), "no confirmation");
}
