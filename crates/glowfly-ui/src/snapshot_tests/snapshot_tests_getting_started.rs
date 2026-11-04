//! Help > Getting started: the page over an open repository in dark and light (two features in
//! use, so left out), without a repository with the finished cards listed (Show me disabled
//! where it needs one), German for clipping, and the messages its buttons, the switch and Esc
//! push.

use super::snapshot_tests::{harness, remove_fixture, settings, with_repo};
use super::snapshot_tests_esc::{app_harness, esc};
use crate::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use glowfly_core::forge::{Account, Connection, Provider};
use glowfly_core::settings::ThemeChoice;
use glowfly_model::msg::Msg;
use glowfly_model::onboarding::OnboardingMsg;
use glowfly_model::state::AppState;
use glowfly_model::state::onboarding::getting_started::{Card, GettingStarted};
use std::time::Instant;

fn connect(st: &mut AppState) {
    let account = Account { provider: Provider::GitHub, host: "github.com".into(), org: String::new() };
    st.settings.connections.push(Connection { account, user: "ada".into(), method: "token".into() });
}

/// The page over the fixture repository, with a connection and a theme picked (both in use).
fn over_repo(theme: ThemeChoice, name: &str) -> (AppState, std::path::PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    connect(&mut st);
    st.onboarding.getting_started = Some(GettingStarted::default());
    (st, dir)
}

fn onboarding(msgs: Vec<Msg>) -> Vec<OnboardingMsg> {
    msgs.into_iter().filter_map(|m| if let Msg::Onboarding(o) = m { Some(o) } else { None }).collect()
}

#[test]
fn getting_started_dark() {
    let (st, dir) = over_repo(ThemeChoice::Dark, "getting-started-dark");
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("Connect a hosting service").is_none(), "a feature in use is left out");
    h.check_snapshot("getting_started_dark");
    remove_fixture(&dir);
}

#[test]
fn getting_started_light() {
    let (st, dir) = over_repo(ThemeChoice::Light, "getting-started-light");
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("getting_started_light");
    remove_fixture(&dir);
}

#[test]
fn getting_started_no_repo_finished_shown() {
    let mut st = AppState::new(settings(ThemeChoice::Dark), Vec::new(), Instant::now());
    connect(&mut st);
    st.onboarding.getting_started = Some(GettingStarted { show_finished: true });
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("getting_started_no_repo_finished_shown");
}

#[test]
fn getting_started_german() {
    let (mut st, dir) = over_repo(ThemeChoice::Dark, "getting-started-de");
    st.settings.language = "de".into();
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("getting_started_german");
    remove_fixture(&dir);
}

#[test]
fn getting_started_buttons_and_esc() {
    let (st, dir) = over_repo(ThemeChoice::Dark, "getting-started-clicks");
    let mut h = app_harness(st);
    h.run();
    h.get_all_by_label("Show me").next().unwrap().click();
    h.run();
    assert_eq!(onboarding(std::mem::take(&mut h.state_mut().1)), [OnboardingMsg::ShowMe(Card::Tour)], "the tour's card is first");
    h.get_by_label("Show the cards already in use (2)").click();
    h.run();
    assert_eq!(onboarding(std::mem::take(&mut h.state_mut().1)), [OnboardingMsg::ShowFinished(true)]);
    h.get_by_label("Hide from the menu").click();
    h.run();
    assert_eq!(onboarding(std::mem::take(&mut h.state_mut().1)), [OnboardingMsg::HideGettingStarted(true)]);
    assert_eq!(onboarding(esc(&mut h)), [OnboardingMsg::CloseGettingStarted]);
    remove_fixture(&dir);
}
