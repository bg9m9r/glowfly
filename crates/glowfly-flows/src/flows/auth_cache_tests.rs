//! Headless tests for the shared credential cache: which answers other tabs and the dashboard
//! reuse (an environment or saved token anywhere, a credential helper's answer only in its own
//! folder), what keeps a background refresh of an unconnected account going, and what drops
//! an answer (a rejected sign-in, a connection change).

use super::tests::{drive, git, scratch_repo, state};
use super::update;
use glowfly_core::forge::{Account, Auth, Connection, HELPER_SOURCE, Provider};
use glowfly_model::effect::{Effect, FilterResult, ForgeError, ForgeReply, ForgeResult, FoundAuth, NetworkEffect};
use glowfly_model::msg::{ForgeMsg, Msg, RepoMsg};
use glowfly_model::remote::{RemoteEffect, RemoteMsg};
use glowfly_model::state::AppState;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn auth(source: &str) -> Auth {
    Auth::from_token(Provider::GitHub, None, "tok", source)
}

fn github() -> Account {
    Account { provider: Provider::GitHub, host: "github.com".into(), org: String::new() }
}

/// Opens a scratch repository whose origin is on GitHub, in a new tab. Its credentials folder
/// is the one `st.any_dir()` names while it is the active tab.
fn open_repo(st: &mut AppState, name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    git(&dir, &["remote", "add", "origin", "https://github.com/o/r.git"]);
    drive(st, Msg::Repo(RepoMsg::Open(dir.clone())));
    assert!(st.session.forge.is_some(), "the service is detected");
    dir
}

fn opened(name: &str) -> (AppState, PathBuf) {
    let mut st = state();
    (st.settings.auto_fetch_minutes, st.settings.avatar_lookups) = (0, false);
    let a = open_repo(&mut st, name);
    (st, a)
}

fn cleanup(dirs: &[PathBuf]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

/// The runner's report of a credential its lookup found, as the active tab's request had it.
fn found(st: &mut AppState, asked: bool, connected: bool, a: Auth) {
    let found = FoundAuth { account: github(), dir: st.any_dir(), connected, asked, auth: a };
    update(st, Msg::Forge(ForgeMsg::AuthFound(Box::new(found))), Instant::now());
}

fn cached(st: &AppState, dir: &Path, env_only: bool) -> Option<String> {
    st.auth_cache.get(&github().key(), dir, env_only).map(|a| a.source.clone())
}

/// The credential (its source) each forge call a background refresh starts carries.
fn refresh_auths(st: &mut AppState) -> Vec<Option<String>> {
    let fx = update(st, Msg::Forge(ForgeMsg::ForgeRefresh { interactive: false }), Instant::now());
    fx.iter()
        .filter_map(|e| match e {
            Effect::Network(NetworkEffect::Forge { auth, .. }) => Some(auth.as_ref().map(|a| a.source.clone())),
            _ => None,
        })
        .collect()
}

fn reply(st: &mut AppState, result: ForgeResult) {
    let target = st.session.forge.as_ref().unwrap().host.clone().unwrap();
    update(st, Msg::Forge(ForgeMsg::ForgeReplied(ForgeReply { target, results: vec![result] })), Instant::now());
}

fn rejected() -> ForgeError {
    ForgeError { message: "authentication failed: bad credentials".into(), auth: true }
}

#[test]
fn two_tabs_share_an_environment_token() {
    let (mut st, a) = opened("auth-cache-env-a");
    found(&mut st, false, false, auth("GITHUB_TOKEN"));
    let b = open_repo(&mut st, "auth-cache-env-b");
    let sent = refresh_auths(&mut st);
    assert!(!sent.is_empty() && sent.iter().all(|s| s.as_deref() == Some("GITHUB_TOKEN")), "{sent:?}");
    assert_eq!(cached(&st, &AppState::outside_dir(), false).as_deref(), Some("GITHUB_TOKEN"), "the dashboard reads it too");
    cleanup(&[a, b]);
}

#[test]
fn different_credential_folders_dont_share_a_helper_answer() {
    let (mut st, a) = opened("auth-cache-helper-a");
    found(&mut st, true, false, auth(HELPER_SOURCE));
    let b = open_repo(&mut st, "auth-cache-helper-b");
    let sent = refresh_auths(&mut st);
    assert!(!sent.is_empty() && sent.iter().all(Option::is_none), "another folder asks its own helper: {sent:?}");
    drive(&mut st, Msg::Repo(RepoMsg::Open(a.clone())));
    let sent = refresh_auths(&mut st);
    assert!(!sent.is_empty() && sent.iter().all(|s| s.as_deref() == Some(HELPER_SOURCE)), "{sent:?}");
    cleanup(&[a, b]);
}

#[test]
fn a_rejected_call_clears_what_it_could_have_carried() {
    let rejections: [fn(&mut AppState); 4] = [
        |st| reply(st, ForgeResult::Prs(Err(rejected()))),
        |st| reply(st, ForgeResult::PrDetails(Err(rejected()))),
        |st| reply(st, ForgeResult::Filter(FilterResult::Me { epoch: 0, result: Err(rejected()) })),
        |st| reply(st, ForgeResult::AuthFailed("authentication failed: bad credentials".into())),
    ];
    for (i, reject) in rejections.into_iter().enumerate() {
        let (mut st, a) = opened(&format!("auth-cache-401-{i}"));
        let dir = st.any_dir();
        found(&mut st, true, false, auth(HELPER_SOURCE));
        found(&mut st, true, false, auth("GITHUB_TOKEN"));
        st.auth_cache.store(github().key(), &dir.join("other"), auth(HELPER_SOURCE), true);
        assert!(cached(&st, &dir, false).is_some());
        reject(&mut st);
        assert!(cached(&st, &dir, false).is_none(), "case {i}: its folder's answer and the shared ones go");
        assert!(cached(&st, &dir.join("other"), false).is_some(), "case {i}: another folder's answer stays");
        let sent = refresh_auths(&mut st);
        assert!(!sent.is_empty() && sent.iter().all(Option::is_none), "case {i}: {sent:?}");
        cleanup(&[a]);
    }
}

#[test]
fn a_failed_lookup_in_one_tab_keeps_the_sign_in_made_in_another() {
    let (mut st, a) = opened("auth-cache-failed-a");
    // The user signed in in tab A: a background refresh there carries the helper's answer.
    found(&mut st, true, false, auth(HELPER_SOURCE));
    let b = open_repo(&mut st, "auth-cache-failed-b");
    // Tab B's own quiet lookup finds nothing.
    reply(&mut st, ForgeResult::AuthFailed("not signed in to GitHub".into()));
    drive(&mut st, Msg::Repo(RepoMsg::Open(a.clone())));
    let sent = refresh_auths(&mut st);
    assert!(!sent.is_empty() && sent.iter().all(|s| s.as_deref() == Some(HELPER_SOURCE)), "{sent:?}");
    cleanup(&[a, b]);
}

#[test]
fn a_quiet_helper_answer_does_not_keep_an_unconnected_background_refresh_going() {
    let (mut st, a) = opened("auth-cache-quiet");
    let dir = st.any_dir();
    found(&mut st, false, false, auth(HELPER_SOURCE));
    let sent = refresh_auths(&mut st);
    assert!(sent.iter().all(Option::is_none), "{sent:?}");
    // The user's own calls in that folder do reuse it.
    let fx = update(&mut st, Msg::Remote(RemoteMsg::ForkDialog("origin".into())), Instant::now());
    let carried = fx.iter().any(|e| matches!(e, Effect::Remote(RemoteEffect::ListForks(r)) if r.auth.is_some()));
    assert!(carried, "{fx:?}");
    assert_eq!(cached(&st, &dir, true), None);
    // Asked for in that folder (Sign in): background refreshes carry it too.
    found(&mut st, true, false, auth(HELPER_SOURCE));
    let sent = refresh_auths(&mut st);
    assert!(!sent.is_empty() && sent.iter().all(|s| s.as_deref() == Some(HELPER_SOURCE)), "{sent:?}");
    cleanup(&[a]);
}

#[test]
fn an_answer_from_before_a_connection_change_is_ignored() {
    let (mut st, a) = opened("auth-cache-changed");
    st.settings.connections.push(Connection { account: github(), user: "ada".into(), method: "access token".into() });
    found(&mut st, false, false, auth("GITHUB_TOKEN"));
    assert!(st.auth_cache.is_empty(), "the lookup started before the account was connected");
    found(&mut st, false, true, auth("GITHUB_TOKEN"));
    assert!(!st.auth_cache.is_empty());
    cleanup(&[a]);
}

#[test]
fn the_dashboard_sign_in_and_failures_forget_the_account() {
    use crate::flows::dashboard_tests::{deliver, gh, reply as fetched, run, scanned, token};
    use glowfly_model::effect::{AccountReply, TargetReply};
    use glowfly_model::msg::DashboardMsg;
    let mut st = state();
    st.dashboard.generation = 1;
    deliver(&mut st, vec![scanned("widgets", &[("origin", "https://github.com/acme/widgets")], &[])]);
    let out = AppState::outside_dir();
    let held = |st: &AppState| cached(st, &out, false).is_some();
    let runner_found = |st: &mut AppState| {
        let found =
            FoundAuth { account: github(), dir: AppState::outside_dir(), connected: false, asked: true, auth: token(Provider::GitHub) };
        update(st, Msg::Forge(ForgeMsg::AuthFound(Box::new(found))), Instant::now());
    };
    let account = |auth: Result<Auth, String>, targets| AccountReply { account: github(), auth, me: None, targets };

    // A reply that signed in adds nothing by itself: the runner's AuthFound did.
    run(&mut st, fetched(1, false, vec![account(Ok(token(Provider::GitHub)), Vec::new())]));
    assert!(!held(&st));
    runner_found(&mut st);
    assert!(held(&st));

    run(&mut st, DashboardMsg::SignIn(github()));
    assert!(!held(&st), "Sign in looks it up again");

    runner_found(&mut st);
    run(&mut st, fetched(1, true, vec![account(Err("not signed in to GitHub".into()), Vec::new())]));
    assert!(!held(&st), "a failed sign-in");

    runner_found(&mut st);
    let target = TargetReply { target: gh("acme", "widgets"), prs: Some(Err(rejected())), issues: None };
    run(&mut st, fetched(1, true, vec![account(Ok(token(Provider::GitHub)), vec![target])]));
    assert!(!held(&st), "a call the service rejected");

    // A failed dashboard lookup says nothing about a repository folder's own credential helper.
    let folder = AppState::outside_dir().join("a-repository");
    st.auth_cache.store(github().key(), &folder, auth(HELPER_SOURCE), true);
    run(&mut st, fetched(1, true, vec![account(Err("not signed in to GitHub".into()), Vec::new())]));
    assert!(cached(&st, &folder, false).is_some(), "a tab's sign-in stays");

    runner_found(&mut st);
    let target = TargetReply { target: gh("acme", "widgets"), prs: Some(Ok(Vec::new())), issues: None };
    run(&mut st, fetched(1, true, vec![account(Ok(token(Provider::GitHub)), vec![target])]));
    assert!(held(&st), "a good reply keeps it");
}
