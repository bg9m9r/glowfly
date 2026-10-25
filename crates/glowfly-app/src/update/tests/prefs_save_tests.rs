//! Headless tests for the Preferences save model: the Save / Revert bar on each tracked page,
//! the "Saved" tick and its clock, the navigation column's rows, the Integrations form toggle,
//! and the Configure terminal palette entry. Leaving an edited page is in
//! `prefs_leave_tests.rs`.

use super::profiles_tests::{WORK, two_profiles};
use super::tests::state;
use super::update;
use crate::effect::Effect;
use crate::effect::PrefsEffect;
use crate::keymap::{BindingId, Slot};
use crate::msg::{FolderPurpose, ForgeMsg, Msg, PaletteMsg, PrefsMsg};
use crate::state::keys_page::Recording;
use crate::state::palette::{PaletteCommand as C, entries};
use crate::state::prefs::{Autocrlf, GitConfigValues};
use crate::state::prefs_save::unsaved;
use crate::state::{AppState, PrefPage, PrefsState};
use glowfly_core::forge::{Account, Provider};
use std::time::{Duration, Instant};

pub(super) fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

pub(super) fn prefs(st: &mut AppState, m: PrefsMsg) -> Vec<Effect> {
    run(st, Msg::Prefs(m))
}

/// Preferences open on `page` with this git config read.
pub(super) fn on_page(page: PrefPage, git: GitConfigValues) -> AppState {
    let mut st = state();
    st.prefs = Some(PrefsState::new(page));
    prefs(&mut st, PrefsMsg::GitConfigLoaded(git));
    st
}

/// The repository's Identity page, its local name "Ada".
pub(super) fn identity() -> AppState {
    on_page(PrefPage::RepoIdentity, GitConfigValues { repo_name: "Ada".into(), ..Default::default() })
}

pub(super) fn git_mut(st: &mut AppState) -> &mut GitConfigValues {
    st.prefs.as_mut().unwrap().git.as_mut().unwrap()
}

pub(super) fn edited(st: &AppState) -> bool {
    unsaved(st.prefs.as_ref().unwrap(), &st.settings, st.snap.as_ref())
}

pub(super) fn page(st: &AppState) -> PrefPage {
    st.prefs.as_ref().unwrap().page
}

/// The git config writes among `fx`: (global, key, value).
pub(super) fn writes(fx: &[Effect]) -> Vec<(bool, &str, Option<&str>)> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Prefs(PrefsEffect::SetGitConfig { global, key, value, .. }) => Some((*global, key.as_str(), value.as_deref())),
            _ => None,
        })
        .collect()
}

#[test]
fn identity_saves_only_the_changed_keys_and_shows_saved() {
    let mut st = identity();
    assert!(!edited(&st));
    git_mut(&mut st).repo_email = " ada@example.com ".into();
    assert!(edited(&st));
    let fx = prefs(&mut st, PrefsMsg::SavePage);
    assert_eq!(writes(&fx), [(false, "user.email", Some("ada@example.com"))], "the unchanged name isn't written");
    assert!(!edited(&st), "the loaded copy now holds what was written");
    assert_eq!(git_mut(&mut st).repo_email, "ada@example.com", "the field is trimmed");
    assert!(st.prefs.as_ref().unwrap().saved.pending, "the tick waits for the next frame");

    // Clearing the name unsets it.
    git_mut(&mut st).repo_name.clear();
    let fx = prefs(&mut st, PrefsMsg::SavePage);
    assert_eq!(writes(&fx), [(false, "user.name", None)]);
}

#[test]
fn revert_puts_the_loaded_values_back() {
    let mut st = identity();
    git_mut(&mut st).repo_name = "Grace".into();
    assert!(prefs(&mut st, PrefsMsg::RevertPage).is_empty());
    assert_eq!(git_mut(&mut st).repo_name, "Ada");
    assert!(!edited(&st));
}

#[test]
fn hooks_browse_only_fills_the_field() {
    let mut st = on_page(PrefPage::RepoHooks, GitConfigValues::default());
    let path = glowfly_core::paths::temp_dir().join("hooks");
    let fx = prefs(&mut st, PrefsMsg::FolderPicked { purpose: FolderPurpose::HooksPath, path: Some(path.clone()) });
    assert!(fx.is_empty(), "{fx:?}");
    assert_eq!(git_mut(&mut st).hooks_path, path.display().to_string());
    assert!(edited(&st));
    let fx = prefs(&mut st, PrefsMsg::SavePage);
    assert_eq!(writes(&fx), [(false, "core.hooksPath", Some(path.display().to_string().as_str()))]);
}

#[test]
fn general_saves_long_paths_and_line_endings_globally() {
    let mut st = on_page(PrefPage::General, GitConfigValues::default());
    git_mut(&mut st).longpaths = true;
    git_mut(&mut st).autocrlf = Autocrlf::Input;
    let fx = prefs(&mut st, PrefsMsg::SavePage);
    assert_eq!(writes(&fx), [(true, "core.longpaths", Some("true")), (true, "core.autocrlf", Some("input"))]);
    let p = st.prefs.as_ref().unwrap();
    assert_eq!(p.git_saved.as_ref().map(|g| (g.longpaths, g.autocrlf.clone())), Some((true, Autocrlf::Input)));
    assert!(!edited(&st) && p.saved.pending);

    git_mut(&mut st).longpaths = false;
    git_mut(&mut st).autocrlf = Autocrlf::False;
    prefs(&mut st, PrefsMsg::RevertPage);
    assert_eq!((git_mut(&mut st).longpaths, git_mut(&mut st).autocrlf.clone()), (true, Autocrlf::Input));
}

#[test]
fn profile_fields_save_through_the_bar() {
    let mut st = two_profiles();
    let p = st.prefs.as_mut().unwrap();
    assert_eq!(p.profiles.selected, Some(WORK));
    p.profiles.author_name = "Ada Lovelace".into();
    assert!(edited(&st));
    let fx = prefs(&mut st, PrefsMsg::SavePage);
    assert_eq!(writes(&fx), [(true, "user.name", Some("Ada Lovelace"))], "the current profile is git's identity");
    assert_eq!(st.settings.profile(WORK).unwrap().author_name, "Ada Lovelace");
    assert!(!edited(&st));
}

#[test]
fn the_saved_tick_runs_on_the_frame_clock() {
    let mut st = state();
    st.prefs = Some(PrefsState::new(PrefPage::Ui));
    st.settings.show_toolbar = !st.settings.show_toolbar;
    prefs(&mut st, PrefsMsg::SettingsEdited);
    assert!(st.prefs.as_ref().unwrap().saved.pending, "an app setting that saves shows it too");
    let t0 = Instant::now();
    let flash = |st: &AppState| st.prefs.as_ref().unwrap().saved.clone();
    run(&mut st, Msg::Tick(t0));
    assert!(flash(&st).showing() && !flash(&st).pending);
    assert_eq!(flash(&st).alpha, 1.0);
    run(&mut st, Msg::Tick(t0 + Duration::from_millis(1800)));
    assert!((flash(&st).alpha - 0.5).abs() < 0.01, "half way through the fade: {}", flash(&st).alpha);
    run(&mut st, Msg::Tick(t0 + Duration::from_millis(2100)));
    assert!(!flash(&st).showing());

    st.settings.reduce_motion = true;
    prefs(&mut st, PrefsMsg::SettingsEdited);
    run(&mut st, Msg::Tick(t0));
    run(&mut st, Msg::Tick(t0 + Duration::from_millis(1800)));
    assert_eq!(flash(&st).alpha, 1.0, "no fade with Reduce motion");
    run(&mut st, Msg::Tick(t0 + Duration::from_millis(2100)));
    assert!(!flash(&st).showing());
}

#[test]
fn a_signing_write_shows_saved() {
    let mut st = on_page(PrefPage::Signing, GitConfigValues::default());
    let fx = prefs(&mut st, PrefsMsg::GitConfig { global: true, key: "commit.gpgsign".into(), value: Some("true".into()) });
    assert_eq!(writes(&fx), [(true, "commit.gpgsign", Some("true"))]);
    assert!(st.prefs.as_ref().unwrap().saved.pending);
}

#[test]
fn nav_rows_run_what_their_page_needs() {
    let mut st = on_page(PrefPage::General, GitConfigValues::default());
    let fx = prefs(&mut st, PrefsMsg::Navigate(PrefPage::Git));
    assert_eq!(page(&st), PrefPage::Git);
    assert!(fx.iter().any(|e| matches!(e, Effect::GitEnv(_))), "finds the installed gits: {fx:?}");

    st.prefs.as_mut().unwrap().page = PrefPage::Keyboard;
    st.prefs.as_mut().unwrap().keys.recording = Some(Recording { id: BindingId::NewTab, slot: Slot::Add });
    prefs(&mut st, PrefsMsg::Navigate(PrefPage::General));
    assert_eq!(page(&st), PrefPage::General);
    assert!(st.prefs.as_ref().unwrap().keys.recording.is_none(), "a key being recorded stays with its page");
}

#[test]
fn the_sparse_row_loads_its_status() {
    let dir = super::tests::scratch_repo("prefs-nav-sparse");
    let mut st = state();
    super::tests::drive(&mut st, Msg::Repo(crate::msg::RepoMsg::Open(dir.clone())));
    st.prefs = Some(PrefsState::new(PrefPage::General));
    let fx = prefs(&mut st, PrefsMsg::Navigate(PrefPage::RepoSparse));
    assert_eq!(page(&st), PrefPage::RepoSparse);
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadSparse { .. })), "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_account_form_opens_and_closes() {
    let mut st = state();
    st.prefs = Some(PrefsState::new(PrefPage::Integrations));
    st.prefs.as_mut().unwrap().int_token = "secret".into();
    prefs(&mut st, PrefsMsg::ShowAccountForm(true));
    let p = st.prefs.as_ref().unwrap();
    assert!(p.int_adding);
    assert!(p.int_token.is_empty(), "a fresh form");
    prefs(&mut st, PrefsMsg::ShowAccountForm(false));
    assert!(!st.prefs.as_ref().unwrap().int_adding);

    // Another service, or a connection made, puts the form away.
    prefs(&mut st, PrefsMsg::ShowAccountForm(true));
    prefs(&mut st, PrefsMsg::SelectIntegration(Provider::GitLab));
    assert!(!st.prefs.as_ref().unwrap().int_adding);
    prefs(&mut st, PrefsMsg::ShowAccountForm(true));
    let account = Account { provider: Provider::GitLab, host: "gitlab.com".into(), org: String::new() };
    run(&mut st, Msg::Forge(ForgeMsg::Connected { account, method: "access token".into(), token: None, result: Ok("ada".into()) }));
    assert!(!st.prefs.as_ref().unwrap().int_adding);
}

#[test]
fn configure_terminal_opens_the_terminal_page() {
    let mut st = state();
    let all = entries(&st);
    let terminal = all.iter().find(|e| e.label == "Configure terminal").expect("offered");
    assert!(terminal.keywords.contains("terminal"));
    let tools = all.iter().find(|e| e.label == "Configure external tools").unwrap();
    assert!(!tools.keywords.split(' ').any(|w| w == "terminal"), "{}", tools.keywords);
    run(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::ConfigureTerminal)));
    assert_eq!(st.prefs.as_ref().map(|p| p.page), Some(PrefPage::Terminal));
    assert_eq!(st.palette_recent.position("configure-terminal"), Some(0));
}
