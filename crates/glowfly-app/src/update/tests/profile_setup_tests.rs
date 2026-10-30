//! Headless tests for what switches with a profile besides its identity: preferences and
//! connected accounts swap only once the switch worked, the app follows them (watcher, repo
//! refresh, diff row, pull requests and issues), new profiles start with or without the
//! accounts, and shared sign-ins stay while any profile lists their account.

use super::profiles_tests::{PERSONAL, WORK, profile, run, saves, two_profiles, values};
use super::tests::{drive, git, scratch_repo};
use crate::effect::{AccountEffect, Effect};
use crate::msg::{ForgeMsg, Msg, RepoMsg};
use crate::profile::ProfileMsg;
use crate::state::{AppState, ForgeState, PrefPage};
use glowfly_core::forge::{Account, Connection, Provider, PullRequest, Target};
use glowfly_core::repo::DiffTarget;
use glowfly_core::settings::{ProfilePrefs, ThemeChoice};
use std::path::PathBuf;

fn conn(provider: Provider, host: &str, user: &str) -> Connection {
    Connection { account: Account { provider, host: host.into(), org: String::new() }, user: user.into(), method: "access token".into() }
}

fn github() -> Connection {
    conn(Provider::GitHub, "github.com", "ada")
}

fn gitlab() -> Connection {
    conn(Provider::GitLab, "gitlab.com", "ada-l")
}

/// Work (current, Dark, GitHub) and Personal (saved Light without file watching, GitLab).
fn with_setups() -> AppState {
    let mut st = two_profiles();
    st.settings.connections = vec![github()];
    let prefs = ProfilePrefs { theme: ThemeChoice::Light, watch_files: false, ..ProfilePrefs::from_settings(&st.settings) };
    let p = st.settings.profile_mut(PERSONAL).unwrap();
    p.prefs = Some(prefs);
    p.connections = Some(vec![gitlab()]);
    st
}

/// Switches to `id` and reports the switch's writes as having worked.
fn switch(st: &mut AppState, id: glowfly_core::settings::ProfileId) -> Vec<Effect> {
    profile(st, ProfileMsg::Switch(id));
    profile(st, ProfileMsg::Applied { id, result: Ok(values("Ada", "ada@example.com")) })
}

fn token_deletes(fx: &[Effect]) -> Vec<Account> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Account(AccountEffect::DeleteToken(a)) => Some(a.clone()),
            _ => None,
        })
        .collect()
}

fn forge_calls(fx: &[Effect]) -> usize {
    fx.iter().filter(|e| matches!(e, Effect::Forge { .. })).count()
}

fn pr(id: u64) -> PullRequest {
    PullRequest {
        id,
        title: "Add search".into(),
        description: String::new(),
        source: "feature".into(),
        target: "main".into(),
        author: "ada".into(),
        draft: false,
        reviewers: Vec::new(),
        head: None,
        head_repo: Default::default(),
        merge_status: None,
        url: "u".into(),
        checks: None,
        facets: Default::default(),
    }
}

#[test]
fn switch_applies_incoming_setup() {
    let mut st = with_setups();
    assert!(st.watch.enabled);
    profile(&mut st, ProfileMsg::Switch(PERSONAL));
    assert_eq!(
        (st.settings.theme.clone(), st.settings.connections.clone()),
        (ThemeChoice::Graphite, vec![github()]),
        "unchanged while pending"
    );

    let fx = profile(&mut st, ProfileMsg::Applied { id: PERSONAL, result: Ok(values("Ada L", "ada@home.example")) });
    assert_eq!(st.settings.theme, ThemeChoice::Light);
    assert_eq!(st.settings.connections, vec![gitlab()]);
    assert!(!st.watch.enabled, "the watcher follows the new General settings");
    let work = st.settings.profile(WORK).unwrap();
    assert_eq!(work.prefs.as_ref().map(|p| p.theme.clone()), Some(ThemeChoice::Graphite));
    assert_eq!(work.connections, Some(vec![github()]));
    assert_eq!(saves(&fx), 1);
    let saved = fx.iter().find_map(|e| if let Effect::SaveSettings(s) = e { Some(s) } else { None }).unwrap();
    assert_eq!((saved.theme.clone(), saved.active_profile), (ThemeChoice::Light, Some(PERSONAL)), "saved after the swap");
    assert!(st.toasts.iter().any(|t| t.ok && t.text == "Switched to profile Personal"));
}

#[test]
fn switching_back_restores_work() {
    let mut st = with_setups();
    switch(&mut st, PERSONAL);
    switch(&mut st, WORK);
    assert_eq!(st.settings.active_profile, Some(WORK));
    assert_eq!((st.settings.theme.clone(), st.settings.connections.clone()), (ThemeChoice::Graphite, vec![github()]));
    assert!(st.watch.enabled);
    let personal = st.settings.profile(PERSONAL).unwrap();
    assert_eq!(personal.prefs.as_ref().map(|p| (p.theme.clone(), p.watch_files)), Some((ThemeChoice::Light, false)));
    assert_eq!(personal.connections, Some(vec![gitlab()]));
    let work = st.settings.profile(WORK).unwrap();
    assert!(work.prefs.is_none() && work.connections.is_none(), "the current profile's are the live ones");
}

#[test]
fn failed_switch_changes_nothing() {
    let mut st = with_setups();
    let before = serde_json::to_string(&st.settings).unwrap();
    profile(&mut st, ProfileMsg::Switch(PERSONAL));
    profile(&mut st, ProfileMsg::Applied { id: PERSONAL, result: Err("could not lock config file".into()) });
    assert_eq!(serde_json::to_string(&st.settings).unwrap(), before);
}

#[test]
fn unsaved_profile_keeps_current_prefs_and_accounts() {
    let mut st = two_profiles();
    st.settings.connections = vec![github()];
    let t = Target { provider: Provider::GitHub, host: "github.com".into(), owner: "o".into(), project: String::new(), repo: "r".into() };
    st.session.forge = Some(ForgeState::new(Some(t.clone()), "origin".into(), Some(t)));
    st.session.forge.as_mut().unwrap().prs = vec![pr(1)];
    let fx = switch(&mut st, PERSONAL);
    assert_eq!(st.settings.active_profile, Some(PERSONAL));
    assert_eq!((st.settings.theme.clone(), st.settings.connections.clone()), (ThemeChoice::Graphite, vec![github()]));
    assert_eq!(forge_calls(&fx), 0, "{fx:?}");
    assert_eq!(st.session.forge.as_ref().unwrap().prs.len(), 1, "pull requests stay");
    assert_eq!(st.settings.profile(WORK).unwrap().connections, Some(vec![github()]), "Work keeps its own");
}

/// A scratch repository whose origin is on GitHub, opened in a new tab.
fn open_github_repo(st: &mut AppState, name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    git(&dir, &["remote", "add", "origin", "https://github.com/o/r.git"]);
    drive(st, Msg::Repo(RepoMsg::Open(dir.clone())));
    assert!(st.snap.is_some() && st.session.forge.is_some(), "the service is detected");
    dir
}

#[test]
fn account_change_reloads_forge() {
    let mut st = with_setups();
    st.settings.profile_mut(PERSONAL).unwrap().prefs = None;
    (st.settings.auto_fetch_minutes, st.settings.avatar_lookups) = (0, false);
    let a = open_github_repo(&mut st, "profile-forge-a");
    st.session.forge.as_mut().unwrap().prs = vec![pr(1)];
    let b = open_github_repo(&mut st, "profile-forge-b");
    assert!(st.tabs[0].parked.as_ref().and_then(|p| p.session.forge.as_ref()).is_some_and(|f| f.prs.len() == 1));
    st.session.forge.as_mut().unwrap().prs = vec![pr(2)];

    let fx = switch(&mut st, PERSONAL);
    let fs = st.session.forge.as_ref().expect("still on GitHub");
    assert!(fs.prs.is_empty() && fs.auth.is_empty(), "rebuilt");
    assert!(fs.last_refresh.is_some(), "refreshed at once");
    assert_eq!(forge_calls(&fx), 1, "{fx:?}");
    // Personal doesn't list github.com, so the sign-in Work saved for it isn't used.
    assert!(fx.iter().any(|e| matches!(e, Effect::Forge { auth: None, connected: false, .. })), "{fx:?}");
    assert!(st.tabs[0].parked.as_ref().unwrap().session.forge.is_none(), "the parked tab detects again when selected");
    for d in [a, b] {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn prefs_change_refreshes_repo_and_keeps_diff_row() {
    let mut st = with_setups();
    st.settings.profile_mut(PERSONAL).unwrap().connections = None;
    let dir = scratch_repo("profile-prefs-refresh");
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let file =
        glowfly_core::repo::FileDiff { binary: false, hunks: Vec::new(), highlight: None, intraline: Default::default(), whole: None };
    st.session.diff.view = Some((DiffTarget::Unstaged("a.txt".into()), file.into()));
    st.session.tab_view.diff_top_row = 6;
    st.view.diff_scroll_to_row = None;
    let fx = switch(&mut st, PERSONAL);
    assert_eq!(st.view.diff_scroll_to_row, Some(6));
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadRepo { .. })), "{fx:?}");
    assert_eq!(forge_calls(&fx), 0);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn integrations_page_follows_switch() {
    let mut st = with_setups();
    let self_managed = conn(Provider::GitLabSelfManaged, "gitlab.example", "ada");
    st.settings.profile_mut(PERSONAL).unwrap().connections = Some(vec![self_managed]);
    let p = st.prefs.as_mut().unwrap();
    (p.page, p.int_provider, p.int_host) = (PrefPage::Integrations, Provider::GitLabSelfManaged, String::new());
    switch(&mut st, PERSONAL);
    assert_eq!(st.prefs.as_ref().unwrap().int_host, "gitlab.example");
}

#[test]
fn add_with_and_without_accounts() {
    let mut st = with_setups();
    profile(&mut st, ProfileMsg::Add { name: "Blank".into(), copy_current: false, copy_accounts: false });
    profile(&mut st, ProfileMsg::Add { name: "Copied".into(), copy_current: false, copy_accounts: true });
    let by_name = |n: &str| st.settings.profiles.iter().find(|p| p.name == n).unwrap().connections.clone();
    assert_eq!(by_name("Blank"), Some(Vec::new()));
    assert_eq!(by_name("Copied"), Some(vec![github()]));
}

#[test]
fn disconnect_keeps_shared_token() {
    let mut st = with_setups();
    st.settings.profile_mut(PERSONAL).unwrap().connections = Some(vec![gitlab(), github()]);
    let fx = run(&mut st, Msg::Forge(ForgeMsg::Disconnect(github().account)));
    assert!(token_deletes(&fx).is_empty(), "Personal still uses it: {fx:?}");
    assert!(st.settings.connections.is_empty());

    let mut st = with_setups();
    let fx = run(&mut st, Msg::Forge(ForgeMsg::Disconnect(github().account)));
    assert_eq!(token_deletes(&fx), vec![github().account]);

    let mut st = with_setups();
    let bb = conn(Provider::Bitbucket, "bitbucket.org", "ada");
    st.settings.connections = vec![github(), bb.clone()];
    st.settings.profile_mut(PERSONAL).unwrap().connections = Some(vec![github()]);
    let fx = run(&mut st, Msg::Forge(ForgeMsg::ForgetTokens { gcm: true }));
    assert_eq!(token_deletes(&fx), vec![bb.account.clone()]);
    let gcm = fx.iter().find_map(|e| match e {
        Effect::Account(AccountEffect::ForgetCredentialManager { accounts, .. }) => Some(accounts.clone()),
        _ => None,
    });
    assert_eq!(gcm, Some(vec![bb.account]), "the shared GitHub sign-in stays");
}

#[test]
fn deleting_profile_forgets_only_its_accounts() {
    let mut st = with_setups();
    st.settings.profile_mut(PERSONAL).unwrap().connections = Some(vec![gitlab(), github()]);
    let fx = profile(&mut st, ProfileMsg::Delete(PERSONAL));
    assert!(st.settings.profile(PERSONAL).is_none());
    assert_eq!(token_deletes(&fx), vec![gitlab().account], "GitHub is still connected here");
}

#[test]
fn the_dashboard_forgets_the_previous_profiles_sign_ins() {
    use crate::state::dashboard::{AccountRow, AccountState};
    let mut st = with_setups();
    let account = github().account;
    let d = &mut st.dashboard;
    d.auth.insert(account.key(), glowfly_core::forge::Auth::from_token(Provider::GitHub, None, "t", "test"));
    let me = Some(glowfly_core::forge::Me { id: "work-user".into(), name: "Work User".into(), ..Default::default() });
    d.accounts = vec![AccountRow { me, ..AccountRow::new(account.clone(), AccountState::Ok) }];
    d.fetched = Some(std::time::Instant::now());
    switch(&mut st, PERSONAL);
    let d = &st.dashboard;
    assert!(d.auth.is_empty(), "no credential carries over");
    assert!(d.account(&account).is_some_and(|a| a.me.is_none()), "nor the signed-in user");
    assert!(d.fetched.is_none(), "the results are stale");
}
