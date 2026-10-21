//! Preferences > Profile snapshots: two profiles with the current one selected, the New
//! profile form with a duplicate name, the inline rename field, a switch running, another
//! profile's saved (or not yet saved) accounts and preferences, the Integrations page naming
//! the profile, and the palette's profile sub-list; plus the clicks the page turns into
//! messages. Built from state only: no git config is read or written.

use super::snapshot_tests::{harness, render, settings};
use crate::msg::Msg;
use crate::profile::ProfileMsg;
use crate::state::palette::{Mode, Palette, Pick};
use crate::state::prefs::GitConfigValues;
use crate::state::profiles::{AddForm, COPY_ACCOUNTS, RenameForm, SWITCH_BUTTON};
use crate::state::{AppState, PrefPage, PrefsState};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use glowfly_core::forge::{Account, Connection, Provider};
use glowfly_core::settings::{Profile, ProfileId, ProfilePrefs, Settings, ThemeChoice};
use std::time::Instant;

const WORK: ProfileId = ProfileId(1);
const PERSONAL: ProfileId = ProfileId(2);
const OPEN_SOURCE: ProfileId = ProfileId(3);

fn conn(provider: Provider, host: &str, user: &str) -> Connection {
    Connection { account: Account { provider, host: host.into(), org: String::new() }, user: user.into(), method: "access token".into() }
}

fn with_profiles(theme: ThemeChoice) -> Settings {
    let mut s = settings(theme);
    s.profiles = vec![
        Profile {
            id: WORK,
            name: "Work".into(),
            author_name: "Ada Lovelace".into(),
            author_email: "ada@analytical.example".into(),
            signing_key: Some("4AE0 9C1B".into()),
            ..Profile::default()
        },
        Profile {
            id: PERSONAL,
            name: "Personal".into(),
            author_name: "Ada".into(),
            author_email: "ada@home.example".into(),
            signing_key: None,
            ..Profile::default()
        },
        Profile { id: OPEN_SOURCE, name: "Open source".into(), ..Profile::default() },
    ];
    s.active_profile = Some(WORK);
    s.connections = vec![conn(Provider::GitHub, "github.com", "ada-work")];
    s
}

/// Preferences open on the Profile page with Work's identity in git config and selected.
fn profile_page(theme: ThemeChoice) -> AppState {
    let mut st = AppState::new(with_profiles(theme), Vec::new(), Instant::now());
    let mut prefs = PrefsState::new(PrefPage::Profile);
    prefs.git = Some(GitConfigValues {
        user_name: "Ada Lovelace".into(),
        user_email: "ada@analytical.example".into(),
        signing_key: "4AE0 9C1B".into(),
        signing_format: "openpgp".into(),
        ..Default::default()
    });
    prefs.profiles.fill(&st.settings, None);
    st.prefs = Some(prefs);
    st
}

fn page(st: &mut AppState) -> &mut crate::state::profiles::ProfilesPage {
    &mut st.prefs.as_mut().unwrap().profiles
}

#[test]
fn prefs_profiles() {
    render("prefs_profiles", profile_page(ThemeChoice::Dark), true);
}

#[test]
fn prefs_profiles_add_light() {
    let mut st = profile_page(ThemeChoice::Light);
    page(&mut st).adding =
        Some(AddForm { name: "personal".into(), error: Some("Another profile already has this name.".into()), ..AddForm::new() });
    render("prefs_profiles_add_light", st, false);
}

#[test]
fn prefs_profiles_rename() {
    let mut st = profile_page(ThemeChoice::Dark);
    let settings = st.settings.clone();
    page(&mut st).fill(&settings, Some(PERSONAL));
    page(&mut st).renaming = Some(RenameForm { name: "Home".into(), ..RenameForm::new(PERSONAL, "Personal") });
    render("prefs_profiles_rename", st, true);
}

#[test]
fn prefs_profiles_switching() {
    let mut st = profile_page(ThemeChoice::Dark);
    st.profile_switch = Some(PERSONAL);
    // The spinner keeps repainting; a fixed number of frames keeps the image reproducible.
    let mut h = harness(st, true);
    h.run_steps(4);
    let switch = h.get_by_label(SWITCH_BUTTON);
    assert!(switch.is_disabled(), "the other Switch button waits");
    h.check_snapshot("prefs_profiles_switching");
}

#[test]
fn prefs_profiles_setup() {
    let mut st = profile_page(ThemeChoice::Dark);
    let saved = vec![conn(Provider::GitHub, "github.com", "ada"), conn(Provider::GitLabSelfManaged, "gitlab.example", "ada")];
    let prefs = ProfilePrefs { theme: ThemeChoice::Light, ..ProfilePrefs::from_settings(&st.settings) };
    let p = st.settings.profile_mut(PERSONAL).unwrap();
    (p.connections, p.prefs) = (Some(saved), Some(prefs));
    let settings = st.settings.clone();
    page(&mut st).fill(&settings, Some(PERSONAL));
    render("prefs_profiles_setup", st, true);
}

#[test]
fn prefs_profiles_setup_inherit_light() {
    let mut st = profile_page(ThemeChoice::Light);
    let settings = st.settings.clone();
    page(&mut st).fill(&settings, Some(OPEN_SOURCE));
    render("prefs_profiles_setup_inherit_light", st, false);
}

#[test]
fn prefs_integrations_profile() {
    let mut st = AppState::new(with_profiles(ThemeChoice::Dark), Vec::new(), Instant::now());
    let mut prefs = PrefsState::new(PrefPage::Integrations);
    (prefs.int_provider, prefs.int_host) = (Provider::GitHub, "github.com".into());
    st.prefs = Some(prefs);
    render("prefs_integrations_profile", st, true);
}

#[test]
fn add_form_sends_the_accounts_choice() {
    let mut st = profile_page(ThemeChoice::Dark);
    page(&mut st).adding = Some(AddForm { name: "Client".into(), ..AddForm::new() });
    let mut h = harness(st, true);
    h.run();
    h.get_by_label(COPY_ACCOUNTS).click();
    h.run();
    h.get_by_label("Add").click();
    h.run();
    let sent = h
        .state()
        .1
        .iter()
        .any(|m| matches!(m, Msg::Profile(ProfileMsg::Add { name, copy_current: true, copy_accounts: true }) if name == "Client"));
    assert!(sent, "Add with copy_accounts was not sent");
}

#[test]
fn palette_profiles() {
    let mut st = AppState::new(with_profiles(ThemeChoice::Dark), Vec::new(), Instant::now());
    st.palette = Some(Palette { mode: Mode::Pick(Pick::profiles(&st.settings)), ..Default::default() });
    render("palette_profiles", st, true);
}

#[test]
fn page_clicks_turn_into_messages() {
    let mut st = profile_page(ThemeChoice::Dark);
    // Only Personal and Open source can be switched to; keep one Switch button.
    st.settings.profiles.truncate(2);
    let mut h = harness(st, true);
    h.run();
    h.get_by_label(SWITCH_BUTTON).click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Profile(ProfileMsg::Switch(PERSONAL)))));
    h.get_by_label("Personal").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Profile(ProfileMsg::Select(PERSONAL)))));
    let deletes: Vec<bool> = h.get_all_by_label("Delete").map(|n| n.is_disabled()).collect();
    assert_eq!(deletes, vec![true, false], "the current profile's Delete is disabled");
}
