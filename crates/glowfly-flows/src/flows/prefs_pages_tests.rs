//! Every Preferences page opens the same way whichever way leads to it: opening Preferences on
//! the page, its navigation row, or its area's `ShowPage` message (update/prefs_nav.rs
//! `enter_page`), before and after git config is read.

use super::tests::{drive, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::git_env::GitEnvMsg;
use crate::gitflow::GitFlowMsg;
use crate::lfs::LfsMsg;
use crate::msg::{ForgeMsg, Msg, PrefsMsg, RepoMsg, SparseMsg};
use crate::profile::ProfileMsg;
use crate::ssh::SshMsg;
use crate::state::AppState;
use crate::state::PrefPage::{self, *};
use crate::state::prefs::GitConfigValues;
use crate::tools::ToolsMsg;
use glowfly_core::forge::{Account, Connection, Provider};
use glowfly_core::settings::{RepoTrust, Settings, TrackerChoice};
use std::time::Instant;

const PAGES: [PrefPage; 18] = [
    General,
    Git,
    Profile,
    Integrations,
    Ui,
    Language,
    DiffViewer,
    Keyboard,
    Tools,
    Terminal,
    Signing,
    Ssh,
    RepoIdentity,
    RepoHooks,
    RepoTracker,
    RepoLfs,
    RepoGitFlow,
    RepoSparse,
];

/// The area message that shows `page` in open Preferences, for the pages that have one.
fn show_page(page: PrefPage) -> Option<Msg> {
    Some(match page {
        Git => Msg::GitEnv(GitEnvMsg::ShowPage),
        Profile => Msg::Profile(ProfileMsg::ShowPage),
        Ssh => Msg::Ssh(SshMsg::ShowPage),
        RepoTracker => Msg::Forge(ForgeMsg::ShowTrackerPage),
        RepoLfs => Msg::Lfs(LfsMsg::ShowPage),
        RepoGitFlow => Msg::GitFlow(GitFlowMsg::ShowPage),
        RepoSparse => Msg::Sparse(SparseMsg::ShowPage),
        Tools => Msg::Tools(ToolsMsg::ShowPage),
        General | Integrations | Ui | Language | DiffViewer | Keyboard | Terminal | Signing | RepoIdentity | RepoHooks => return None,
    })
}

/// A trusted repository whose tracker is a connected Trello board, so every repository page
/// has something to read.
fn opened(name: &str) -> AppState {
    let mut st = state();
    let trello = Account { provider: Provider::Trello, host: "trello.com".into(), org: String::new() };
    st.settings.connections.push(Connection { account: trello.clone(), user: "Ada".into(), method: "access token".into() });
    drive(&mut st, Msg::Repo(RepoMsg::Open(scratch_repo(name))));
    let real = st.snap.as_ref().expect("opened").real_workdir.clone();
    crate::state::trust::record(&mut st.settings, &real, RepoTrust::Trusted);
    st.settings.set_tracker(&real, TrackerChoice::Trello { account: trello, board: String::new(), board_name: String::new() });
    st
}

fn config() -> GitConfigValues {
    GitConfigValues { user_name: "Ada".into(), user_email: "ada@example.com".into(), ..Default::default() }
}

/// Runs `msgs` from `start` with Preferences closed and returns what they left: the page's
/// state and every effect, in a stable order (the ways differ only in when the reads every
/// page shares go out).
fn outcome(st: &mut AppState, start: &Settings, msgs: Vec<Msg>) -> String {
    st.prefs = None;
    st.git.page = Default::default();
    st.settings = start.clone();
    st.settings_file.saved = serde_json::to_string(start).unwrap_or_default();
    let mut fx: Vec<String> = Vec::new();
    for msg in msgs {
        fx.extend(update(st, msg, Instant::now()).iter().map(|e: &Effect| format!("{e:?}")));
    }
    fx.sort();
    let p = st.prefs.as_ref().expect("Preferences open");
    format!(
        "page {:?}\nprofiles {:?} {:?} {:?}\nssh {} {:?} {:?}\nlfs {} {:?}\nsparse {} {:?}\nboards {:?} {}\ngitflow {:?} {:?}\ngit scan {} {}\neffects\n{}",
        p.page,
        p.profiles.selected,
        p.profiles.author_name,
        p.profiles.author_email,
        p.ssh.generation,
        p.ssh.comment,
        p.ssh.error,
        p.lfs.generation,
        p.lfs.error,
        p.sparse.generation,
        p.sparse.error,
        p.trello_boards.account,
        p.trello_boards.loading,
        p.gitflow.workdir,
        p.gitflow.form,
        st.git.page.generation,
        st.git.page.candidates.is_none(),
        fx.join("\n"),
    )
}

fn open(page: PrefPage) -> Msg {
    Msg::Prefs(PrefsMsg::OpenPrefs(page))
}

fn loaded() -> Msg {
    Msg::Prefs(PrefsMsg::GitConfigLoaded(config()))
}

#[test]
fn every_page_opens_alike_directly_by_its_row_and_by_its_show_page_message() {
    let mut st = opened("prefs-pages-direct");
    let start = st.settings.clone();
    for page in PAGES {
        let direct = outcome(&mut st, &start, vec![open(page)]);
        let row = outcome(&mut st, &start, vec![open(General), Msg::Prefs(PrefsMsg::Navigate(page))]);
        assert_eq!(direct, row, "{page:?}: opened directly, then by its navigation row");
        if let Some(msg) = show_page(page) {
            assert_eq!(direct, outcome(&mut st, &start, vec![open(General), msg]), "{page:?}: by its ShowPage message");
        }
    }
}

#[test]
fn every_page_opens_alike_once_git_config_is_read() {
    let mut st = opened("prefs-pages-config");
    let start = st.settings.clone();
    for page in PAGES {
        let direct = outcome(&mut st, &start, vec![open(page), loaded()]);
        let row = outcome(&mut st, &start, vec![open(General), loaded(), Msg::Prefs(PrefsMsg::Navigate(page))]);
        assert_eq!(direct, row, "{page:?}: opened directly, then by its navigation row");
        if let Some(msg) = show_page(page) {
            assert_eq!(direct, outcome(&mut st, &start, vec![open(General), loaded(), msg]), "{page:?}: by its ShowPage message");
        }
    }
}
