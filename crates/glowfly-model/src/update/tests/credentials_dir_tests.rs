//! Where the account calls that ask git's credential helpers run (`AppState::credentials_dir`):
//! signing in through the credential manager, signing it out of hosts and listing a Trello
//! account's boards, and the fork dialogs' service calls, run in the active repository only while
//! git runs there unasked, and in a folder outside any repository otherwise, so its own credential
//! helper never runs untrusted.

use super::file_trust_tests::{clean, held, refused, trusted};
use super::tests::state;
use super::update;
use crate::effect::{AccountEffect, Effect};
use crate::msg::{ForgeMsg, Msg};
use crate::state::prefs::PrefsState;
use crate::state::{AppState, PrefPage};
use glowfly_core::forge::{Account, Connection, Provider};
use std::path::PathBuf;
use std::time::Instant;

/// The folder each account effect among `fx` runs in.
fn dirs(fx: &[Effect]) -> Vec<PathBuf> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Account(AccountEffect::Connect { cwd, .. })
            | Effect::Account(AccountEffect::ForgetCredentialManager { cwd, .. })
            | Effect::Account(AccountEffect::TrelloBoards { cwd, .. }) => Some(cwd.clone()),
            _ => None,
        })
        .collect()
}

/// The folders Connect, forgetting the sign-ins and loading Trello boards run in, in `st`.
fn asked_in(mut st: AppState) -> Vec<PathBuf> {
    let github = Account { provider: Provider::GitHub, host: "github.com".into(), org: String::new() };
    let trello = Account { provider: Provider::Trello, host: "trello.com".into(), org: String::new() };
    st.settings.connections.push(Connection { account: trello.clone(), user: "ada".into(), method: "token".into() });
    st.prefs = Some(PrefsState::new(PrefPage::RepoTracker));
    let mut out = Vec::new();
    for msg in [
        ForgeMsg::Connect { account: github, user: None, token: None },
        ForgeMsg::ForgetTokens { gcm: true },
        ForgeMsg::LoadTrelloBoards(trello.clone()),
    ] {
        if matches!(msg, ForgeMsg::LoadTrelloBoards(_)) {
            // Forgetting the tokens dropped the connection: the boards load for a connected one.
            st.settings.connections.push(Connection { account: trello.clone(), user: "ada".into(), method: "token".into() });
        }
        let fx = update(&mut st, Msg::Forge(msg), Instant::now());
        out.extend(dirs(&fx));
    }
    assert_eq!(out.len(), 3, "each one asks once: {out:?}");
    out
}

#[test]
fn account_calls_run_in_the_active_repository_only_while_git_runs_there_unasked() {
    for st in [clean("cred-clean"), trusted("cred-trusted")] {
        let workdir = st.workdir().unwrap();
        assert_eq!(asked_in(st), vec![workdir; 3]);
    }
    for st in [held("cred-held"), refused("cred-refused")] {
        assert_eq!(asked_in(st), vec![AppState::outside_dir(); 3], "its own credential helper never runs");
    }
    // With no repository open there is none to read.
    assert_eq!(asked_in(state()), vec![AppState::outside_dir(); 3]);
}

/// The folder the fork dialogs' service calls sign in from, opened on a GitHub `origin` in `st`:
/// Add a fork lists the forks, Fork on GitHub looks for the user's own.
fn fork_calls_in(mut st: AppState) -> Vec<PathBuf> {
    use crate::remote::{RemoteEffect, RemoteMsg};
    use glowfly_core::repo::RemoteInfo;
    let origin = "https://github.com/team/app.git";
    st.snap.as_mut().unwrap().remote_info.push(RemoteInfo {
        name: "origin".into(),
        urls: vec![origin.into()],
        push_urls: Vec::new(),
        skip_fetch_all: false,
    });
    let mut out = Vec::new();
    for msg in [RemoteMsg::ForkDialog("origin".into()), RemoteMsg::ForkOnService("origin".into())] {
        st.modal = None;
        for e in update(&mut st, Msg::Remote(msg), Instant::now()) {
            match e {
                Effect::Remote(RemoteEffect::ListForks(r) | RemoteEffect::FindMyFork(r)) => out.push(r.cwd),
                e => panic!("unexpected {e:?}"),
            }
        }
    }
    assert_eq!(out.len(), 2, "each dialog asks once: {out:?}");
    out
}

#[test]
fn fork_calls_sign_in_from_the_repository_only_while_git_runs_there_unasked() {
    for st in [clean("fork-cred-clean"), trusted("fork-cred-trusted")] {
        let workdir = st.workdir().unwrap();
        assert_eq!(fork_calls_in(st), vec![workdir; 2]);
    }
    for st in [held("fork-cred-held"), refused("fork-cred-refused")] {
        assert_eq!(fork_calls_in(st), vec![AppState::outside_dir(); 2], "its own credential helper never runs");
    }
}

#[test]
fn a_call_for_another_repository_signs_in_outside_any_repository() {
    let st = trusted("cred-other");
    let workdir = st.workdir().unwrap();
    assert_eq!(st.credentials_dir_for(&workdir), workdir);
    assert_eq!(st.credentials_dir_for(&workdir.join("elsewhere")), AppState::outside_dir());
    assert_eq!(state().credentials_dir_for(&workdir), AppState::outside_dir());
}
