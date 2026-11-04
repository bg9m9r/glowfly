//! Preferences after design pass D18: the search in the navigation (filtered pages with their
//! counts, Enter opening the first match, the no-match line), General's Notifications and
//! Commit messages groups in light and dark, help collapsed behind "Learn more", the content
//! left-anchored and at most 760 px wide in a 1900 px window, and the renamed Commit identity
//! and Profiles pages.

use super::snapshot_tests::{harness_sized, render, settings, with_repo};
use crate::snapshot_check::CheckSnapshot;
use eframe::egui::{self, Event, Key, Modifiers};
use egui_kittest::kittest::Queryable;
use glowfly_core::git_version::GitInfo;
use glowfly_core::settings::ThemeChoice;
use glowfly_model::msg::{Msg, PrefsMsg};
use glowfly_model::state::prefs::{Autocrlf, GitConfigValues};
use glowfly_model::state::{AppState, PrefPage, PrefsState};
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
    assert!(h.query_by_label("Nothing matches “zzqx”.").is_some());
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

type H = egui_kittest::Harness<'static, (AppState, Vec<Msg>)>;

/// Types Enter in the focused search field (one frame).
fn enter(h: &mut H) {
    h.state_mut().1.clear();
    let modifiers = Modifiers::NONE;
    h.input_mut().events.push(Event::Key { key: Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers });
    h.step();
}

/// `enter`, then applies the messages that frame sent.
fn enter_and_apply(h: &mut H) {
    enter(h);
    let (st, msgs) = h.state_mut();
    for m in std::mem::take(msgs) {
        glowfly_model::update::update(st, m, Instant::now());
    }
}

/// Preferences on `page` with `query` typed in the focused search, motion reduced so the tint
/// stays solid.
fn searching(page: PrefPage, query: &str) -> H {
    let mut st = general(ThemeChoice::Dark, query);
    st.settings.reduce_motion = true;
    let p = st.prefs.as_mut().unwrap();
    (p.page, p.focus_search) = (page, true);
    let mut h = harness_sized(st, true, egui::vec2(1400.0, 820.0));
    h.run();
    h
}

/// The row labelled `label` is tinted with the selected fill: at the start of the label column,
/// level with the row's first node (its label, or the control it names).
fn tinted(h: &H, label: &str) -> bool {
    let n = h.query_all_by_label(label).next().expect("the row");
    let at = egui::pos2(250.0 + 24.0 + 8.0, super::snapshot_tests_painted::bounds(&n).center().y);
    super::snapshot_tests_painted::fills_at(h, at).contains(&crate::theme::color::selected())
}

/// Enter on a row far down a long page scrolls it into view and tints it.
#[test]
fn enter_reveals_a_row_far_down_the_page() {
    let mut h = searching(PrefPage::General, "Glowfly links");
    let below = |h: &H| h.query_all_by_label("Glowfly links").next().and_then(|n| n.bounding_box()).is_none_or(|b| b.y0 > 790.0);
    assert!(below(&h), "the row starts out of view");
    enter_and_apply(&mut h);
    h.run_steps(4);
    let b = h.query_all_by_label("Glowfly links").next().and_then(|n| n.bounding_box()).expect("drawn");
    assert!(b.y0 > 60.0 && b.y1 < 790.0, "scrolled into view: {b:?}");
    assert!(tinted(&h, "Glowfly links"));
}

/// "Interface size" labels a row on Appearance and on Accessibility: from Accessibility, Enter
/// opens Appearance's (first in the nav) and tints that one, never the row on the page left.
#[test]
fn a_shared_label_is_revealed_on_the_page_opened() {
    let mut h = searching(PrefPage::Accessibility, "interface size");
    enter(&mut h);
    assert!(!tinted(&h, "Interface size"), "not the row on Accessibility, the page being left");
    let (st, msgs) = h.state_mut();
    for m in std::mem::take(msgs) {
        glowfly_model::update::update(st, m, Instant::now());
    }
    assert_eq!(h.state().0.prefs.as_ref().unwrap().page, PrefPage::Ui, "Appearance comes first");
    h.run_steps(4);
    assert!(tinted(&h, "Interface size"), "the row on Appearance");
}
