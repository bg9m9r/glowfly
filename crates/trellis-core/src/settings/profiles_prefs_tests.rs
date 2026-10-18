//! What switches with a profile: copying the preferences, classifying every setting, swapping
//! the setups in and out, old files, and which accounts are still in use.

use super::*;
use crate::forge::{Connection, Provider};
use crate::settings::{DiffLayout, DiffMode, Profile, RepoTrust, ResizableColumn};

const A: ProfileId = ProfileId(1);
const B: ProfileId = ProfileId(2);

fn conn(provider: Provider, host: &str, user: &str) -> Connection {
    Connection { account: Account { provider, host: host.into(), org: String::new() }, user: user.into(), method: "access token".into() }
}

fn json(s: &Settings) -> String {
    serde_json::to_string(s).unwrap()
}

/// A (current, Dark, GitHub) and B (saved Light, GitLab).
fn two() -> Settings {
    let mut light = ProfilePrefs { theme: ThemeChoice::Light, ..ProfilePrefs::default() };
    light.tab_size = 2;
    Settings {
        connections: vec![conn(Provider::GitHub, "github.com", "ada")],
        profiles: vec![
            Profile { id: A, name: "A".into(), ..Profile::default() },
            Profile {
                id: B,
                name: "B".into(),
                connections: Some(vec![conn(Provider::GitLab, "gitlab.com", "ada-l")]),
                prefs: Some(light),
                ..Profile::default()
            },
        ],
        active_profile: Some(A),
        ..Settings::default()
    }
}

#[test]
fn round_trip_copies_profile_fields() {
    let mut src = Settings {
        theme: ThemeChoice::Light,
        date_time_format: "YYYY-MM-DD HH:mm".into(),
        relative_dates: true,
        columns: GraphColumns::compact(),
        diff_font_size: 16.0,
        tab_size: 8,
        auto_fetch_minutes: 0,
        watch_files: false,
        initial_commits: 900,
        ..Settings::default()
    };
    src.column_widths.set(ResizableColumn::Author, 222.0);
    let mut dst = Settings::default();
    ProfilePrefs::from_settings(&src).apply_to(&mut dst);
    assert_eq!(ProfilePrefs::from_settings(&dst), ProfilePrefs::from_settings(&src));
    assert_eq!(dst.theme, ThemeChoice::Light);
    assert_eq!(dst.date_time_format, "YYYY-MM-DD HH:mm");
    assert!(dst.relative_dates && !dst.watch_files);
    assert_eq!(dst.columns, GraphColumns::compact());
    assert_eq!(dst.column_widths, src.column_widths);
    assert_eq!((dst.diff_font_size, dst.tab_size, dst.auto_fetch_minutes, dst.initial_commits), (16.0, 8, 0, 900));
}

#[test]
fn apply_leaves_global_fields() {
    let mut s = two();
    s.repo_trust.insert("C:/r".into(), RepoTrust::Trusted);
    s.favorite_repos.push("C:/r".into());
    s.graph_filters.insert("C:/r".into(), Default::default());
    s.repo_trackers.insert("C:/r".into(), Default::default());
    s.external_tools.editor = "code".into();
    s.use_ssh_agent = true;
    s.diff_layout = DiffLayout::Split;
    s.diff_mode = DiffMode::WholeFile;
    s.diff_ignore_whitespace = true;
    s.diff_word_wrap = true;
    s.push_after_commit = true;
    let before = json(&s);
    let mut prefs = ProfilePrefs::from_settings(&s);
    prefs.theme = ThemeChoice::Light;
    prefs.apply_to(&mut s);
    assert_eq!(s.theme, ThemeChoice::Light);
    s.theme = ThemeChoice::Dark;
    assert_eq!(json(&s), before, "only the profile's fields changed");
}

#[test]
fn every_setting_is_classified() {
    let keys = |v: serde_json::Value| -> Vec<String> { v.as_object().unwrap().keys().cloned().collect() };
    let all = keys(serde_json::to_value(Settings::default()).unwrap());
    let prefs = keys(serde_json::to_value(ProfilePrefs::default()).unwrap());
    for g in GLOBAL_FIELDS {
        assert!(!prefs.iter().any(|p| p == g), "{g} is both a profile preference and global");
    }
    let mut classified: Vec<String> = prefs.into_iter().chain(GLOBAL_FIELDS.iter().map(|g| g.to_string())).collect();
    classified.sort();
    let mut all = all;
    all.sort();
    assert_eq!(all, classified, "add a new setting to ProfilePrefs or GLOBAL_FIELDS");
}

#[test]
fn swap_saves_outgoing_and_loads_incoming() {
    let mut s = two();
    let dark = ProfilePrefs::from_settings(&s);
    assert_eq!(s.swap_profile_setup(B), SetupSwap { connections: true, prefs: true });
    assert_eq!((s.theme.clone(), s.tab_size), (ThemeChoice::Light, 2));
    assert_eq!(s.connections, vec![conn(Provider::GitLab, "gitlab.com", "ada-l")]);
    let a = s.profile(A).unwrap();
    assert_eq!(a.prefs.as_ref(), Some(&dark));
    assert_eq!(a.connections, Some(vec![conn(Provider::GitHub, "github.com", "ada")]));
    let b = s.profile(B).unwrap();
    assert!(b.prefs.is_none() && b.connections.is_none(), "the incoming profile's are live now");
    assert_eq!(s.active_profile, Some(A), "the caller makes B current");
}

#[test]
fn swap_keeps_the_diff_header_toggles() {
    let mut s = two();
    (s.diff_word_wrap, s.diff_ignore_whitespace, s.diff_layout) = (true, true, DiffLayout::Split);
    s.swap_profile_setup(B);
    assert!(s.diff_word_wrap && s.diff_ignore_whitespace);
    assert_eq!(s.diff_layout, DiffLayout::Split);
    s.active_profile = Some(B);
    s.diff_word_wrap = false;
    s.swap_profile_setup(A);
    assert!(!s.diff_word_wrap, "word wrap is the header toggle's, not the profile's");
}

#[test]
fn swap_back_restores_exactly() {
    let original = two();
    let mut s = two();
    s.swap_profile_setup(B);
    s.active_profile = Some(B);
    assert_eq!(s.swap_profile_setup(A), SetupSwap { connections: true, prefs: true });
    s.active_profile = Some(A);
    assert_eq!(ProfilePrefs::from_settings(&s), ProfilePrefs::from_settings(&original));
    assert_eq!(s.connections, original.connections);
    let (a, b) = (s.profile(A).unwrap(), s.profile(B).unwrap());
    assert!(a.prefs.is_none() && a.connections.is_none());
    assert_eq!(b.prefs, original.profile(B).unwrap().prefs);
    assert_eq!(b.connections, original.profile(B).unwrap().connections);
}

#[test]
fn swap_to_unsaved_profile_keeps_live() {
    let mut s = two();
    let b = s.profile_mut(B).unwrap();
    (b.prefs, b.connections) = (None, None);
    let mut expected = s.clone();
    assert_eq!(s.swap_profile_setup(B), SetupSwap::default());
    assert_eq!((s.theme.clone(), s.connections.clone()), (expected.theme.clone(), expected.connections.clone()));
    let a = s.profile(A).unwrap();
    assert_eq!(a.prefs, Some(ProfilePrefs::from_settings(&expected)));
    assert_eq!(a.connections.as_ref(), Some(&expected.connections));
    // Nothing else changed.
    let ea = expected.profile_mut(A).unwrap();
    (ea.prefs, ea.connections) = (a.prefs.clone(), a.connections.clone());
    assert_eq!(json(&s), json(&expected));
}

#[test]
fn swap_to_self_or_missing_is_noop() {
    let mut s = two();
    let before = json(&s);
    assert_eq!(s.swap_profile_setup(A), SetupSwap::default());
    assert_eq!(s.swap_profile_setup(ProfileId(9)), SetupSwap::default());
    assert_eq!(json(&s), before);
}

#[test]
fn old_profiles_load_without_setup() {
    let p: Profile = serde_json::from_str(r#"{"id": 2, "name": "Home", "author_name": "Ada", "author_email": "a@b"}"#).unwrap();
    assert!(p.connections.is_none() && p.prefs.is_none());
    let v = serde_json::to_value(&p).unwrap();
    assert!(v.get("connections").is_none() && v.get("prefs").is_none(), "{v}");

    let s = two();
    let b = s.profile(B).unwrap();
    let back: Profile = serde_json::from_str(&serde_json::to_string(b).unwrap()).unwrap();
    assert_eq!(&back, b);
}

#[test]
fn account_in_use() {
    let s = two();
    let github = conn(Provider::GitHub, "github.com", "ada").account;
    let gitlab = conn(Provider::GitLab, "gitlab.com", "ada-l").account;
    let other = conn(Provider::Bitbucket, "bitbucket.org", "ada").account;
    assert!(s.account_in_use(&github), "live");
    assert!(s.account_in_use(&gitlab), "another profile's saved list");
    assert!(!s.account_in_use(&other));
}

#[test]
fn swap_carries_a_theme_file() {
    let mut s = two();
    s.theme = ThemeChoice::Custom("ocean".into());
    if let Some(prefs) = s.profiles[1].prefs.as_mut() {
        prefs.theme = ThemeChoice::Custom("forest".into());
    }
    s.swap_profile_setup(B);
    assert_eq!(s.theme, ThemeChoice::Custom("forest".into()), "in from the incoming profile");
    let a = s.profile(A).and_then(|p| p.prefs.as_ref()).map(|p| p.theme.clone());
    assert_eq!(a, Some(ThemeChoice::Custom("ocean".into())), "out to the outgoing profile");
}

#[test]
fn swap_carries_zoom_and_panels() {
    let mut s = two();
    s.zoom_percent = 150;
    s.profile_mut(B).unwrap().prefs.as_mut().unwrap().show_left_panel = false;
    s.swap_profile_setup(B);
    assert_eq!(s.zoom_percent, 100);
    assert!(!s.show_left_panel);
    s.active_profile = Some(B);
    s.swap_profile_setup(A);
    assert_eq!(s.zoom_percent, 150);
    assert!(s.show_left_panel);
}

#[test]
fn group_worktree_tabs_defaults_on_and_switches_with_the_profile() {
    assert!(Settings::default().group_worktree_tabs);
    // Files written before the setting existed group the tabs.
    let old: Settings = serde_json::from_str(r#"{"auto_fetch_minutes": 5}"#).unwrap();
    assert!(old.group_worktree_tabs);
    let mut s = two();
    s.group_worktree_tabs = false;
    s.swap_profile_setup(B);
    assert!(s.group_worktree_tabs, "B's saved preference");
    s.active_profile = Some(B);
    s.swap_profile_setup(A);
    assert!(!s.group_worktree_tabs, "A's, saved on the way out");
}

#[test]
fn swap_carries_the_menu_bar() {
    let mut s = two();
    s.show_menu_bar = false;
    s.swap_profile_setup(B);
    assert!(s.show_menu_bar, "in from the incoming profile");
    s.active_profile = Some(B);
    s.swap_profile_setup(A);
    assert!(!s.show_menu_bar, "back from the outgoing profile");
}

#[test]
fn swap_carries_keeping_submodules_updated() {
    let mut s = two();
    s.submodules.keep_updated = true;
    s.repo_submodule_updates.insert("/w/r".into(), false);
    s.swap_profile_setup(B);
    assert!(!s.submodules.keep_updated, "B's saved preference");
    assert_eq!(s.repo_submodule_updates.get("/w/r"), Some(&false), "the per-repository choice stays");
    s.active_profile = Some(B);
    s.swap_profile_setup(A);
    assert!(s.submodules.keep_updated, "A's, saved on the way out");
}
