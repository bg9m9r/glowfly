//! When the Windows' OpenSSH row offers the switch, blocks it, offers the undo, or hides.

use super::*;
use crate::state::ssh_use::Choices;
use glowfly_core::ssh_keys::{AgentReach, AgentStatus, WinSshFacts};

const WIN: &str = "C:/Windows/System32/OpenSSH/ssh.exe";
const BUNDLED: &str = "C:/Program Files/Git/usr/bin/ssh.exe";
const T: Lang = Lang::En;

fn facts() -> WinSshFacts {
    WinSshFacts { windows: Some(WIN.into()), windows_version: Some((9, 5)), bundled: Some(BUNDLED.into()), ..Default::default() }
}

/// A page whose agent check found `facts` with the service `service`, and `global` read.
fn page(facts: Option<WinSshFacts>, service: WinService, global: Result<KeyChoice, String>) -> SshPage {
    let agent = AgentStatus { reach: Some(AgentReach::Keys(vec![])), service: Some(service), win_ssh: facts, ..Default::default() };
    SshPage { agent: Some(agent), choice: Some(Choices { global, repo: None }), ..Default::default() }
}

fn running(global: KeyChoice) -> SshPage {
    page(Some(facts()), WinService::Running, Ok(global))
}

#[test]
fn hidden_unless_git_runs_its_own_ssh_with_the_service_running() {
    let hidden = |p: &SshPage| assert_eq!(windows_offer(p, T), WindowsOffer::Hidden);
    hidden(&SshPage::default());
    hidden(&page(None, WinService::Running, Ok(KeyChoice::Unset)));
    hidden(&page(Some(WinSshFacts { windows: None, ..facts() }), WinService::Running, Ok(KeyChoice::Unset)));
    hidden(&page(Some(facts()), WinService::Stopped, Ok(KeyChoice::Unset)));
    hidden(&page(Some(facts()), WinService::Running, Err("boom".into())));
    let mut env = running(KeyChoice::Unset);
    env.agent.as_mut().unwrap().git_ssh_command_set = true;
    hidden(&env);
    let mut unread = running(KeyChoice::Unset);
    unread.choice = None;
    hidden(&unread);
    hidden(&page(
        Some(WinSshFacts { git_ssh: Some("plink.exe".into()), bundled: None, ..facts() }),
        WinService::Running,
        Ok(KeyChoice::Unset),
    ));
    assert_eq!(windows_blocked(&SshPage::default(), T), Some(T.tr("prefs.ssh.windows.unavailable.tip")));
}

#[test]
fn offers_the_switch_or_says_why_not() {
    let offer = WindowsOffer::Offer { windows: WIN.into(), bundled: BUNDLED.into(), old: None };
    assert_eq!(windows_offer(&running(KeyChoice::Unset), T), offer);
    assert_eq!(windows_blocked(&running(KeyChoice::Unset), T), None);
    let old = page(Some(WinSshFacts { windows_version: Some((7, 7)), ..facts() }), WinService::Running, Ok(KeyChoice::Unset));
    assert!(matches!(windows_offer(&old, T), WindowsOffer::Offer { old: Some((7, 7)), .. }));

    let key = running(KeyChoice::Key("C:/Users/ada/.ssh/id".into()));
    let why = T.tr("prefs.ssh.windows.blocked.key.tip");
    assert_eq!(windows_offer(&key, T), WindowsOffer::Blocked { windows: WIN.into(), bundled: BUNDLED.into(), why });
    assert_eq!(windows_blocked(&key, T), Some(why));
    let other = running(KeyChoice::Other { value: "ssh -v".into(), key: None });
    let why = T.tr("prefs.ssh.windows.blocked.other.tip");
    assert!(matches!(windows_offer(&other, T), WindowsOffer::Blocked { why: w, .. } if w == why));

    let mut busy = running(KeyChoice::Unset);
    busy.choice_busy = true;
    assert_eq!(windows_blocked(&busy, T), Some(T.tr("prefs.ssh.which.busy.tip")));
}

#[test]
fn already_and_on() {
    let on_path = WinSshFacts { bundled: None, on_path: Some(WIN.into()), ..facts() };
    assert_eq!(windows_offer(&page(Some(on_path), WinService::Running, Ok(KeyChoice::Unset)), T), WindowsOffer::Already);

    let on = WindowsOffer::On { windows: WIN.into(), service_running: true, old: None };
    assert_eq!(windows_offer(&running(KeyChoice::WindowsSsh(WIN.into())), T), on);
    let stopped =
        page(Some(WinSshFacts { windows_version: Some((7, 7)), ..facts() }), WinService::Stopped, Ok(KeyChoice::WindowsSsh(WIN.into())));
    assert_eq!(windows_offer(&stopped, T), WindowsOffer::On { windows: WIN.into(), service_running: false, old: Some((7, 7)) });
    assert_eq!(windows_blocked(&stopped, T), None, "the undo works with the service stopped");
}
