//! Preferences > Profile's folder rules and Open repository row, and the commit box's identity
//! line: a profile with a folder rule and a repository rule in a repository its rule applies
//! to, a repository whose own identity comes before the rule, a profile without rules (with a
//! refusal shown), plus the clicks the rows turn into messages. Rule paths are fixed display
//! strings; nothing is read from git.

use super::snapshot_tests::{harness, harness_sized, remove_fixture, with_repo};
use crate::msg::Msg;
use crate::profile::ProfileMsg;
use crate::state::prefs::GitConfigValues;
use crate::state::profile_rules::{ADD_FOLDER_BUTTON, REMOVE_BUTTON, USE_REPO_BUTTON};
use crate::state::{AppState, PrefPage, PrefsState};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui;
use egui_kittest::kittest::Queryable;
use std::path::PathBuf;
use trellis_core::paths;
use trellis_core::repo_identity::{IdentitySource, RepoIdentity};
use trellis_core::settings::{FolderRule, Profile, ProfileId, ThemeChoice};

const WORK: ProfileId = ProfileId(1);
const PERSONAL: ProfileId = ProfileId(2);
const OPEN_SOURCE: ProfileId = ProfileId(3);

/// Tall enough to show the Folders row under the fields.
const SIZE: egui::Vec2 = egui::vec2(1400.0, 1250.0);

/// A path absolute on every system, never on disk.
fn abs(rest: &str) -> PathBuf {
    PathBuf::from(if cfg!(windows) { format!("C:/work/{rest}") } else { format!("/work/{rest}") })
}

fn folder(rest: &str) -> FolderRule {
    FolderRule { path: paths::encode(&abs(rest)), ..FolderRule::default() }
}

/// The fixture repository open as `acme` at `/work/acme`, Preferences on the Profile page with
/// Work current and selected; Work has `rules`.
fn page(theme: ThemeChoice, name: &str, rules: Vec<FolderRule>) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    let p = |id, name: &str, author: &str, email: &str| Profile {
        id,
        name: name.into(),
        author_name: author.into(),
        author_email: email.into(),
        ..Profile::default()
    };
    st.settings.profiles = vec![
        Profile { folders: rules, ..p(WORK, "Work", "Ada Lovelace", "ada@analytical.example") },
        p(PERSONAL, "Personal", "Ada", "ada@home.example"),
        p(OPEN_SOURCE, "Open source", "", ""),
    ];
    st.settings.active_profile = Some(WORK);
    let snap = st.snap.as_mut().unwrap();
    snap.name = "acme".into();
    snap.git_dir = abs("acme/.git");
    snap.common.dir = abs("acme/.git");
    let mut prefs = PrefsState::new(PrefPage::Profile);
    prefs.git = Some(GitConfigValues {
        user_name: "Ada Lovelace".into(),
        user_email: "ada@analytical.example".into(),
        signing_format: "openpgp".into(),
        ..Default::default()
    });
    prefs.profiles.fill(&st.settings, None);
    st.prefs = Some(prefs);
    (st, dir)
}

fn ruled() -> Vec<FolderRule> {
    vec![folder(""), FolderRule { repository: true, ..folder("acme/.git") }]
}

fn read_as(st: &mut AppState, id: RepoIdentity) {
    st.identities.insert(abs("acme/.git"), id);
}

fn from_work() -> RepoIdentity {
    RepoIdentity {
        name: "Ada Lovelace".into(),
        email: "ada@analytical.example".into(),
        source: IdentitySource::Profile(WORK),
        expected: Some(WORK),
        ..RepoIdentity::default()
    }
}

fn own() -> RepoIdentity {
    RepoIdentity {
        name: "Ada".into(),
        email: "ada@acme.example".into(),
        source: IdentitySource::Repository,
        local_name: Some("Ada".into()),
        local_email: Some("ada@acme.example".into()),
        expected: Some(WORK),
    }
}

fn sent(h: &egui_kittest::Harness<'_, (AppState, Vec<Msg>)>, want: impl Fn(&Msg) -> bool) -> bool {
    h.state().1.iter().any(want)
}

#[test]
fn prefs_profile_rules() {
    let (mut st, dir) = page(ThemeChoice::Dark, "profile-rules", ruled());
    read_as(&mut st, from_work());
    let mut h = harness_sized(st, true, SIZE);
    h.run();
    h.check_snapshot("prefs_profile_rules");
    assert!(h.query_by_label_contains("from the profile Work (folder rule").is_some(), "the Open repository row names the rule");
    assert!(h.get_by_label(USE_REPO_BUTTON).is_disabled(), "this repository already uses Work");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn prefs_profile_rules_shadowed_light() {
    let (mut st, dir) = page(ThemeChoice::Light, "profile-rules-shadowed", vec![folder("")]);
    read_as(&mut st, own());
    let mut h = harness_sized(st, false, SIZE);
    h.run();
    h.check_snapshot("prefs_profile_rules_shadowed_light");
    assert!(h.query_by_label_contains("This comes before the Work folder rule").is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn prefs_profile_rules_empty() {
    let (mut st, dir) = page(ThemeChoice::Dark, "profile-rules-empty", Vec::new());
    read_as(&mut st, RepoIdentity { source: IdentitySource::Elsewhere, expected: None, ..from_work() });
    st.prefs.as_mut().unwrap().profiles.rules_error = Some("The profile Personal already has this folder.".into());
    let mut h = harness_sized(st, true, SIZE);
    h.run();
    h.check_snapshot("prefs_profile_rules_empty");
    assert!(!h.get_by_label(ADD_FOLDER_BUTTON).is_disabled() && !h.get_by_label(USE_REPO_BUTTON).is_disabled());
    // A profile without an email can't have rules yet.
    h.state_mut().0.prefs.as_mut().unwrap().profiles.selected = Some(OPEN_SOURCE);
    h.run();
    assert!(h.get_by_label(ADD_FOLDER_BUTTON).is_disabled() && h.get_by_label(USE_REPO_BUTTON).is_disabled());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn commit_box_identity() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "commit-identity");
    st.settings.profiles = vec![Profile { id: WORK, name: "Work".into(), ..Profile::default() }];
    let git_dir = st.snap.as_ref().unwrap().git_dir.clone();
    st.identities.insert(git_dir, from_work());
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("commit_box_identity");
    assert!(h.query_by_label("as Ada Lovelace <ada@analytical.example> · Work").is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn rule_clicks_turn_into_messages() {
    let (mut st, dir) = page(ThemeChoice::Dark, "profile-rules-clicks", vec![folder("")]);
    read_as(&mut st, own());
    let mut h = harness_sized(st, true, SIZE);
    h.run();
    h.get_by_label(ADD_FOLDER_BUTTON).click();
    h.run();
    assert!(sent(&h, |m| matches!(m, Msg::Profile(ProfileMsg::AddFolder(WORK)))));
    h.get_by_label(USE_REPO_BUTTON).click();
    h.run();
    assert!(sent(&h, |m| matches!(m, Msg::Profile(ProfileMsg::AddThisRepo(WORK)))));
    h.get_by_label(REMOVE_BUTTON).click();
    h.run();
    let path = paths::encode(&abs(""));
    assert!(sent(&h, |m| matches!(m, Msg::Profile(ProfileMsg::RemoveFolder { id: WORK, path: p }) if *p == path)));
    h.get_by_label("Use Work here").click();
    h.run();
    assert!(sent(&h, |m| matches!(m, Msg::Profile(ProfileMsg::AskUseHere(WORK)))));
    drop(h);
    remove_fixture(&dir);
}
