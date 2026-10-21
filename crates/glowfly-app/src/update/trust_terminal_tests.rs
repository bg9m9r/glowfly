//! Repository trust and the built-in terminal: a shell started while git is held in the
//! repository waits (`SessionStatus::Held`), Trust and run… asks on its behalf, and trusting
//! the repository by any route starts the held shells. Shells already running are never held.

use super::terminal_tests::{run as term, terminal, workdir};
use super::tests::{drive, scratch_repo};
use super::trust_held_tests::held_repo;
use super::trust_print_tests::{add_hook, trusted_repo};
use super::trust_tests::{asking, real, run};
use crate::effect::Effect;
use crate::msg::{Msg, RepoMsg, TabsMsg, TrustMsg};
use crate::state::terminal::SessionStatus;
use crate::state::trust::{Standing, TrustPrompt, record};
use crate::state::{AppState, Modal};
use crate::terminal::{TerminalEffect as E, TerminalMsg as T};
use glowfly_core::settings::{RepoTrust, ShellChoice};
use glowfly_core::terminal::{KeyMods, TermInput, TermKey};

fn statuses(st: &AppState) -> Vec<SessionStatus> {
    st.terminal.sessions.iter().map(|s| s.status.clone()).collect()
}

fn spawns(fx: &[E]) -> usize {
    fx.iter().filter(|e| matches!(e, E::Spawn { .. })).count()
}

fn prompt(st: &AppState) -> &TrustPrompt {
    let Some(Modal::Trust(p)) = &st.modal else { panic!("expected the trust prompt") };
    p
}

fn ask_for_terminal(st: &mut AppState) {
    let real = real(st);
    run(st, Msg::Trust(TrustMsg::AskForTerminal { real }));
}

#[test]
fn a_held_repository_makes_tabs_without_shells_until_it_is_trusted() {
    let (mut st, dir) = held_repo("term-held");
    let fx = terminal(term(&mut st, T::Toggle));
    assert_eq!(spawns(&fx), 0, "{fx:?}");
    assert_eq!(statuses(&st), [SessionStatus::Held]);
    assert!(st.terminal.open);

    let fx = terminal(term(&mut st, T::NewTab(Some(ShellChoice::Bash))));
    assert_eq!(spawns(&fx), 0, "{fx:?}");
    assert_eq!(statuses(&st), [SessionStatus::Held, SessionStatus::Held]);
    assert_eq!(st.terminal.sessions[1].shell.choice, ShellChoice::Bash, "the chosen shell is kept");
    assert_eq!(st.terminal.sessions[1].number, 2);

    ask_for_terminal(&mut st);
    assert!(prompt(&st).terminal && prompt(&st).held.is_none());
    st.terminal.grid = Some((30, 100));
    let real = real(&st);
    let fx = terminal(run(&mut st, Msg::Trust(TrustMsg::Decide { real: real.clone(), trusted: true })));
    assert_eq!(statuses(&st), [SessionStatus::Starting, SessionStatus::Starting]);
    let ids: Vec<u64> = st.terminal.sessions.iter().map(|s| s.id).collect();
    let w = workdir(&st);
    for id in ids {
        let started = fx.iter().filter(|e| matches!(e, E::Spawn { id: i, cwd, rows: 30, cols: 100, .. } if *i == id && *cwd == w));
        assert_eq!(started.count(), 1, "{fx:?}");
    }
    assert_eq!(st.terminal.sessions[1].shell.choice, ShellChoice::Bash);
    assert_eq!(st.terminal.sessions[0].screen.size(), (30, 100));
    assert!(st.terminal.focus_requested);
    assert_eq!(st.trust_standing(), Some(Standing::Trusted));
    assert_eq!(crate::state::trust::recorded(&st.settings, &real), Some(RepoTrust::Trusted));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cancel_leaves_the_shells_held() {
    let (mut st, dir) = held_repo("term-held-cancel");
    term(&mut st, T::Toggle);
    ask_for_terminal(&mut st);
    let fx = terminal(run(&mut st, Msg::CloseModal));
    assert_eq!(spawns(&fx), 0);
    assert!(st.modal.is_none());
    assert_eq!(statuses(&st), [SessionStatus::Held]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_repository_you_chose_not_to_trust_holds_its_shells_too() {
    let (mut st, dir) = held_repo("term-held-refused");
    let real = real(&st);
    record(&mut st.settings, &real, RepoTrust::NotTrusted);
    let fx = terminal(term(&mut st, T::Toggle));
    assert_eq!(spawns(&fx), 0);
    assert_eq!(statuses(&st), [SessionStatus::Held]);
    ask_for_terminal(&mut st);
    assert!(prompt(&st).refused && prompt(&st).terminal);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_trusted_repository_that_gained_a_hook_holds_new_shells() {
    let (mut st, dir) = trusted_repo("term-held-changed");
    add_hook(&dir, "post-merge");
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert_eq!(st.trust_standing(), Some(Standing::Changed));
    let fx = terminal(term(&mut st, T::Toggle));
    assert_eq!(spawns(&fx), 0);
    assert_eq!(statuses(&st), [SessionStatus::Held]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_held_tab_has_nothing_to_kill_restart_or_type_into_and_closes_at_once() {
    let (mut st, dir) = held_repo("term-held-actions");
    term(&mut st, T::Toggle);
    let id = st.terminal.sessions[0].id;
    let typed = T::Input(vec![TermInput::Key { key: TermKey::Enter, mods: KeyMods::default() }]);
    for m in [T::Kill, T::Restart, typed, T::Paste] {
        let fx = term(&mut st, m);
        assert!(fx.is_empty(), "{fx:?}");
    }
    assert_eq!(statuses(&st), [SessionStatus::Held], "Restart leaves it waiting");
    let fx = terminal(term(&mut st, T::CloseTab(id)));
    assert!(matches!(&fx[..], [E::Close(c)] if *c == id), "{fx:?}");
    assert!(st.terminal.sessions.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn switching_tabs_holds_only_the_held_repository_s_shell() {
    let (mut st, a) = held_repo("term-held-tab-a");
    let b = scratch_repo("term-held-tab-b");
    drive(&mut st, Msg::Repo(RepoMsg::Open(b.clone())));
    drive(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)));
    term(&mut st, T::Toggle);
    let wa = workdir(&st);
    assert_eq!(statuses(&st), [SessionStatus::Held]);

    let fx = terminal(super::update(&mut st, Msg::Tabs(TabsMsg::SelectTab(1)), std::time::Instant::now()));
    let wb = workdir(&st);
    assert!(matches!(&fx[..], [E::Spawn { cwd, .. }] if *cwd == wb), "the clean repository's shell starts: {fx:?}");
    let held = |st: &AppState| st.terminal.sessions_for(&wa).map(|s| s.status.clone()).collect::<Vec<_>>();
    assert_eq!(held(&st), [SessionStatus::Held]);

    let fx = terminal(super::update(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)), std::time::Instant::now()));
    assert_eq!(spawns(&fx), 0, "still held: {fx:?}");
    let real = real(&st);
    let fx = terminal(run(&mut st, Msg::Trust(TrustMsg::Decide { real, trusted: true })));
    assert!(matches!(&fx[..], [E::Spawn { cwd, .. }] if *cwd == wa), "{fx:?}");
    assert_eq!(held(&st), [SessionStatus::Starting]);
    let _ = std::fs::remove_dir_all(&a);
    let _ = std::fs::remove_dir_all(&b);
}

#[test]
fn a_repository_that_no_longer_runs_anything_starts_its_held_shells_without_asking() {
    let (mut st, dir) = held_repo("term-held-cleaned");
    term(&mut st, T::Toggle);
    assert_eq!(statuses(&st), [SessionStatus::Held]);
    super::tests::git(&dir, &["config", "--unset", "core.sshCommand"]);
    std::fs::remove_file(dir.join(".git").join("hooks").join("pre-commit")).unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert_eq!(st.trust_standing(), Some(Standing::Clean));
    assert!(!asking(&st));
    assert_eq!(statuses(&st), [SessionStatus::Starting]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn not_trusting_a_repository_never_stops_its_running_shell() {
    let (mut st, dir) = trusted_repo("term-running-refused");
    term(&mut st, T::Toggle);
    let id = st.terminal.sessions[0].id;
    term(&mut st, T::Started { id, program: "sh".into() });
    assert_eq!(statuses(&st), [SessionStatus::Running]);
    let real = real(&st);
    let fx: Vec<Effect> = run(&mut st, Msg::Trust(TrustMsg::Decide { real, trusted: false }));
    assert!(terminal(fx).is_empty());
    assert_eq!(statuses(&st), [SessionStatus::Running]);
    let _ = std::fs::remove_dir_all(&dir);
}
