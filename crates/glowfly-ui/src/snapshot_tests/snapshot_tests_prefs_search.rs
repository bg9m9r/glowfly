//! Preferences after design pass D18: the search in the navigation (filtered pages with their
//! counts, Enter opening the first match, the no-match line), General's Notifications and
//! Commit messages groups in light and dark, help collapsed behind "Learn more", the content
//! left-anchored and at most 760 px wide in a 1900 px window, and the renamed Commit identity
//! and Profiles pages.

use super::snapshot_tests::{harness_sized, render, settings, with_repo};
use crate::msg::{Msg, PrefsMsg};
use crate::state::prefs::{Autocrlf, GitConfigValues};
use crate::state::{AppState, PrefPage, PrefsState};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{self, Event, Key, Modifiers};
use egui_kittest::kittest::Queryable;
use glowfly_core::git_version::GitInfo;
use glowfly_core::settings::ThemeChoice;
use std::time::Instant;

/// General with a git found and its config read, searched for `query`.
fn general(theme: ThemeChoice, query: &str) -> AppState {
    let mut st = AppState::new(settings(theme), Vec::new(), Instant::now());
    let mut prefs = PrefsState::new(PrefPage::General);
    prefs.git = Some(GitConfigValues { autocrlf: Autocrlf::Input, ..Default::default() });
    prefs.search = query.into();
    st.prefs = Some(prefs);
    let program = if cfg!(windows) { "C:/Program Files/Git/cmd/git.exe" } else { "/opt/homebrew/bin/git" };
    st.git.info = Some(GitInfo { path: Some(program.into()), version: Some((2, 45, 1)), chosen: false });
    st
}

#[test]
fn prefs_general_light() {
    render("prefs_general_light", general(ThemeChoice::Light, ""), false);
}

#[test]
fn prefs_search_dark() {
    let mut h = harness_sized(general(ThemeChoice::Dark, "notif"), true, egui::vec2(1400.0, 820.0));
    h.run();
    // General holds the three notification rows; pages without a match are gone.
    assert!(h.query_all_by_label("General").count() == 2, "the nav entry and the title");
    assert!(h.query_by_label("Keyboard").is_none());
    h.check_snapshot("prefs_search_dark");
}

#[test]
fn a_search_with_no_match_says_so() {
    let mut h = harness_sized(general(ThemeChoice::Dark, "zzqx"), true, egui::vec2(1400.0, 820.0));
    h.run();
    assert!(h.query_by_label("No settings match “zzqx”.").is_some());
    assert!(h.query_by_label("General").is_none_or(|n| n.bounding_box().is_some_and(|b| b.x0 > 250.0)), "no page rows in the nav");
}

#[test]
fn enter_in_the_search_opens_the_first_match() {
    let mut st = general(ThemeChoice::Dark, "file lists");
    st.prefs.as_mut().unwrap().focus_search = true;
    let mut h = harness_sized(st, true, egui::vec2(1400.0, 820.0));
    h.run();
    h.state_mut().1.clear();
    let modifiers = Modifiers::NONE;
    h.input_mut().events.push(Event::Key { key: Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers });
    h.step();
    let msgs = &h.state().1;
    assert!(msgs.iter().any(|m| matches!(m, Msg::Prefs(PrefsMsg::Navigate(PrefPage::Ui)))), "Enter opens Appearance");
}

#[test]
fn long_help_opens_behind_learn_more() {
    let mut h = harness_sized(general(ThemeChoice::Dark, ""), true, egui::vec2(1400.0, 820.0));
    h.run();
    let full = "Glowfly asks GitHub's releases API for the newest release of its repository once a day.";
    assert!(h.query_by_label_contains(full).is_none(), "collapsed at first");
    h.query_all_by_label("Learn more").next().expect("a Learn more link").click();
    h.run();
    assert!(h.query_by_label_contains(full).is_some(), "opened in place");
    assert!(h.query_by_label("Show less").is_some());
}

/// At 1900 px the page stays left-anchored beside the navigation, at most 760 px wide.
#[test]
fn the_page_keeps_its_width_in_a_wide_window() {
    let mut h = harness_sized(general(ThemeChoice::Dark, ""), true, egui::vec2(1900.0, 1200.0));
    h.run();
    let left = 250.0 + 24.0;
    let section = h.query_by_label("Notifications").expect("the Notifications group").bounding_box().unwrap();
    assert!((section.x0 - left).abs() < 1.0, "left-anchored: {section:?}");
    let status_top = 1200.0 - 30.0;
    let past: Vec<String> = h
        .query_all_by(|_| true)
        .filter_map(|n| {
            let b = n.bounding_box()?;
            let label = n.label().or_else(|| n.value())?;
            (b.x0 >= left - 1.0 && b.y1 < status_top && b.x1 > left + 760.0 + 1.0).then(|| format!("{label:?} ends at {}", b.x1))
        })
        .collect();
    assert!(past.is_empty(), "{past:#?}");
}

#[test]
fn prefs_ui_light() {
    let mut st = AppState::new(settings(ThemeChoice::Light), Vec::new(), Instant::now());
    st.prefs = Some(PrefsState::new(PrefPage::Ui));
    render("prefs_ui_light", st, false);
}

#[test]
fn prefs_identity_light() {
    let (mut st, dir) = with_repo(ThemeChoice::Light, "prefs_identity_light");
    let mut prefs = PrefsState::new(PrefPage::RepoIdentity);
    prefs.git = Some(GitConfigValues { user_name: "Ada Lovelace".into(), user_email: "ada@example.com".into(), ..Default::default() });
    prefs.git_saved = prefs.git.clone();
    st.prefs = Some(prefs);
    render("prefs_identity_light", st, false);
    super::snapshot_tests::remove_fixture(&dir);
}
