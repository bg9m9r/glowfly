//! Snapshots of the first-run welcome (each step and its states) and the tour of the main window
//! (a panel outlined, the centred last step, a hidden panel), plus the clicks they turn into
//! messages. Built from state only: no git config is read. The welcome in other themes and
//! window sizes is in `snapshot_tests_welcome_looks.rs`.

use super::snapshot_tests::{harness, remove_fixture, settings, with_repo};
use crate::msg::{LayoutMsg, Msg, PrefsMsg};
use crate::onboarding::{OnboardingMsg, StartChoice};
use crate::state::layout::Panel;
use crate::state::onboarding::{Welcome, WelcomeStep};
use crate::state::onboarding_tour::TourStep;
use crate::state::{AppState, Modal, PrefPage};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::accesskit::Role;
use eframe::egui::{Key, Modifiers};
use egui_kittest::kittest::Queryable;
use glowfly_core::forge::{Account, Connection, Provider};
use glowfly_core::git_version::GitInfo;
use glowfly_core::i18n::Lang;
use glowfly_core::identity::Identity;
use glowfly_core::settings::ThemeChoice;
use std::time::Instant;

pub(in crate::ui) fn ada() -> Identity {
    Identity { name: "Ada Lovelace".into(), email: "ada@example.com".into(), signing_key: None }
}

/// No repository open, the welcome on `step` with git's identity read as `git` (None: still
/// reading).
pub(in crate::ui) fn on(theme: ThemeChoice, step: WelcomeStep, git: Option<Identity>) -> AppState {
    let mut st = AppState::new(settings(theme), Vec::new(), Instant::now());
    let mut w = Welcome::new();
    w.step = step;
    if let Some(id) = git {
        w.identity.seed(id);
    }
    st.onboarding.welcome = Some(w);
    st
}

pub(in crate::ui) fn snapshot(name: &str, st: AppState, dark: bool) -> Vec<Msg> {
    let mut h = harness(st, dark);
    h.run();
    h.check_snapshot(name);
    std::mem::take(&mut h.state_mut().1)
}

/// The identity problems a blank or bad form can show.
const PROBLEMS: [&str; 3] = ["welcome.identity.email_no_at", "welcome.identity.email_blank", "welcome.identity.name_blank"];
/// The help lines under the identity fields while they have nothing to complain about.
const HELP: [&str; 2] = ["welcome.identity.name.help", "welcome.identity.email.help"];

/// The welcome's messages a run pushed, taken from the harness.
fn sent(h: &mut H) -> Vec<OnboardingMsg> {
    let msgs = std::mem::take(&mut h.state_mut().1);
    msgs.into_iter().filter_map(|m| if let Msg::Onboarding(o) = m { Some(o) } else { None }).collect()
}

type H = egui_kittest::Harness<'static, (AppState, Vec<Msg>)>;

#[test]
fn welcome_identity() {
    let mut h = harness(on(ThemeChoice::Dark, WelcomeStep::Identity, Some(ada())), true);
    h.run();
    h.check_snapshot("welcome_identity_seeded");
    assert!(h.query_by_label(Lang::En.tr("welcome.subtitle")).is_some(), "the generic subtitle on the first step");
    assert!(h.query_by_label("Nice to meet you, Ada Lovelace.").is_none(), "no greeting yet");
    assert!(h.query_by_label("Step 1 of 4: You").is_some());
    drop(h);
    let mut h = harness(on(ThemeChoice::Dark, WelcomeStep::Identity, Some(Identity::default())), true);
    h.run();
    h.check_snapshot("welcome_identity_missing");
    for key in PROBLEMS {
        assert!(h.query_by_label(Lang::En.tr(key)).is_none(), "{key}: nothing complains before the user types");
    }
    for key in HELP {
        assert!(h.query_by_label(Lang::En.tr(key)).is_some(), "{key} under its field on the untouched form");
    }
    drop(h);
    // The spinner keeps repainting: a fixed number of frames instead of running until idle.
    let mut h = harness(on(ThemeChoice::Dark, WelcomeStep::Identity, None), true);
    h.run_steps(4);
    h.check_snapshot("welcome_identity_reading");
}

#[test]
fn welcome_identity_attempted() {
    let mut st = on(ThemeChoice::Dark, WelcomeStep::Identity, Some(Identity::default()));
    st.onboarding.welcome.as_mut().unwrap().identity.touched.attempt();
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("welcome_identity_attempted");
    for key in ["welcome.identity.name_blank", "welcome.identity.email_blank"] {
        assert!(h.query_by_label(Lang::En.tr(key)).is_some(), "{key} shows after Continue");
    }
    for key in HELP {
        assert!(h.query_by_label(Lang::En.tr(key)).is_none(), "{key}: the problem takes the help's place");
    }
}

#[test]
fn welcome_identity_keys() {
    let mut h = harness(on(ThemeChoice::Dark, WelcomeStep::Identity, Some(Identity::default())), true);
    h.run();
    h.get_by_role_and_label(Role::TextInput, "Name").type_text("Ada");
    h.run();
    h.get_by_role_and_label(Role::TextInput, "Email").type_text("ada@example.com");
    h.run();
    sent(&mut h);
    // A single-line field gives up the focus on Enter: the frame runs its primary, Continue.
    h.press_key(Key::Enter);
    h.run();
    assert_eq!(sent(&mut h), [OnboardingMsg::SaveIdentity]);
    let f = &h.state().0.onboarding.welcome.as_ref().unwrap().identity;
    assert_eq!((f.name.as_str(), f.email.as_str()), ("Ada", "ada@example.com"));
}

#[test]
fn welcome_default_has_focus() {
    let mut st = on(ThemeChoice::Dark, WelcomeStep::Connect, Some(ada()));
    let account = Account { provider: Provider::GitHub, host: "github.com".into(), org: String::new() };
    st.settings.connections = vec![Connection { account, user: "ada-l".into(), method: "access token".into() }];
    let mut h = harness(st, true);
    h.run();
    assert!(h.get_by_role_and_label(Role::Button, "Continue").is_focused(), "the default button takes the focus");
    sent(&mut h);
    h.press_key_modifiers(Modifiers::COMMAND, Key::Enter);
    h.run();
    assert_eq!(sent(&mut h), [OnboardingMsg::Next]);
}

/// A dialog stacked over the welcome gets the keys: Enter and Ctrl/Cmd+Enter answer it, not the
/// welcome underneath.
#[test]
fn a_dialog_over_the_welcome_takes_the_keys() {
    let mut st = on(ThemeChoice::Dark, WelcomeStep::Start, Some(ada()));
    st.modal = Some(Modal::Confirm { title: "t".into(), body: "b".into(), button: "OK".into(), then: Box::new(Msg::ClearLog) });
    let mut h = harness(st, true);
    h.run();
    std::mem::take(&mut h.state_mut().1);
    h.press_key_modifiers(Modifiers::COMMAND, Key::Enter);
    h.run();
    let msgs = std::mem::take(&mut h.state_mut().1);
    assert!(msgs.iter().any(|m| matches!(m, Msg::ConfirmAccepted)), "the top dialog runs its default");
    assert!(!msgs.iter().any(|m| matches!(m, Msg::Onboarding(_))), "the welcome sends nothing");
    h.press_key(Key::Enter);
    h.run();
    assert!(sent(&mut h).is_empty(), "plain Enter doesn't reach the welcome either");
}

#[test]
fn welcome_close_button() {
    let mut st = on(ThemeChoice::Dark, WelcomeStep::Git, Some(ada()));
    st.git.info = Some(GitInfo { path: None, version: Some((2, 45, 1)), chosen: false });
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label(Lang::En.tr("dialog.close.tip")).is_none(), "the welcome draws no standard header");
    sent(&mut h);
    h.get_by_label("Close the welcome").click();
    h.run();
    assert_eq!(sent(&mut h), [OnboardingMsg::CloseWelcome]);
    h.run();
    assert!(sent(&mut h).is_empty(), "the click is taken once");
}

#[test]
fn welcome_identity_typing() {
    let mut h = harness(on(ThemeChoice::Dark, WelcomeStep::Identity, Some(Identity::default())), true);
    h.run();
    h.get_by_role_and_label(Role::TextInput, "Email").type_text("ada");
    h.run();
    assert_eq!(h.state().0.onboarding.welcome.as_ref().unwrap().identity.email, "ada");
    assert!(h.query_by_label(Lang::En.tr("welcome.identity.email_no_at")).is_some(), "the edited field complains");
    assert!(h.query_by_label(Lang::En.tr("welcome.identity.name_blank")).is_none(), "the other one doesn't");
}

#[test]
fn welcome_identity_buttons() {
    let mut h = harness(on(ThemeChoice::Dark, WelcomeStep::Identity, Some(ada())), true);
    h.run();
    h.get_by_label("Skip for now").click();
    h.run();
    h.get_by_label("Continue").click();
    h.run();
    h.get_by_label("Close the welcome").click();
    h.run();
    assert_eq!(sent(&mut h), [OnboardingMsg::Skip, OnboardingMsg::SaveIdentity, OnboardingMsg::CloseWelcome]);
    drop(h);

    // While the identity is written, Continue says why it waits in the footer.
    let mut st = on(ThemeChoice::Dark, WelcomeStep::Identity, Some(ada()));
    st.onboarding.welcome.as_mut().unwrap().identity.saving = true;
    let mut h = harness(st, true);
    h.run();
    assert!(h.get_by_role_and_label(Role::Button, "Continue").is_disabled());
    assert!(h.query_by_label(Lang::En.tr("welcome.identity.saving")).is_some(), "the saving reason");
    drop(h);
    // The spinner keeps repainting: a fixed number of frames.
    let mut h = harness(on(ThemeChoice::Dark, WelcomeStep::Identity, None), true);
    h.run_steps(4);
    assert!(h.get_by_role_and_label(Role::Button, "Continue").is_disabled());
    let reading = h.get_all_by_label(Lang::En.tr("welcome.identity.reading")).count();
    assert!(reading >= 2, "the body's line and the footer's reason: {reading}");
}

#[test]
fn welcome_connect() {
    let st = on(ThemeChoice::Dark, WelcomeStep::Connect, Some(ada()));
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("welcome_connect_none");
    assert!(h.query_by_label("Nice to meet you, Ada Lovelace.").is_some(), "greets with git's name");
    assert!(h.query_by_label("Step 2 of 4: Services").is_some(), "the progress bar's name");
    h.get_by_label("Connect a service…").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Integrations)))));
    drop(h);

    let mut st = on(ThemeChoice::Light, WelcomeStep::Connect, Some(ada()));
    let account = Account { provider: Provider::GitHub, host: "github.com".into(), org: String::new() };
    st.settings.connections = vec![Connection { account, user: "ada-l".into(), method: "access token".into() }];
    snapshot("welcome_connect_connected", st, false);
}

#[test]
fn welcome_git() {
    let mut st = on(ThemeChoice::Dark, WelcomeStep::Git, Some(ada()));
    st.git.info = Some(GitInfo { path: None, version: Some((2, 45, 1)), chosen: false });
    snapshot("welcome_git_ready", st, true);
    let mut st = on(ThemeChoice::Dark, WelcomeStep::Git, Some(ada()));
    st.git.info = Some(GitInfo::default());
    // Dismissing the banner doesn't hide the problem here.
    st.git.banner_dismissed = true;
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("welcome_git_missing");
    h.get_by_label("Download git").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::OpenUrl(u) if u.contains("git-scm.com"))));
}

#[test]
fn welcome_start() {
    let mut h = harness(on(ThemeChoice::Dark, WelcomeStep::Start, Some(ada())), true);
    h.run();
    h.check_snapshot("welcome_start");
    for key in ["welcome.start.open.hint", "welcome.start.clone.hint", "welcome.start.init.hint"] {
        assert!(h.query_by_label(Lang::En.tr(key)).is_some(), "{key} is shown");
    }
    // The welcome screen's own buttons are behind the dialog; the dialog's cards are drawn last.
    for label in ["Open repository…", "Clone repository…", "New repository…"] {
        h.get_all_by_label(label).last().unwrap().click();
        h.run();
    }
    h.get_by_label("Done").click();
    h.run();
    let finished: Vec<_> =
        h.state().1.iter().filter_map(|m| if let Msg::Onboarding(OnboardingMsg::Finish(c)) = m { Some(*c) } else { None }).collect();
    assert_eq!(finished, [Some(StartChoice::Open), Some(StartChoice::Clone), Some(StartChoice::Init), None]);
    drop(h);

    let (mut st, dir) = with_repo(ThemeChoice::Dark, "welcome-start-repo");
    let mut w = Welcome::new();
    w.step = WelcomeStep::Start;
    w.identity.seed(ada());
    st.onboarding.welcome = Some(w);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("welcome_start_with_repo");
    assert!(h.query_by_label(Lang::En.tr("welcome.start.tour_after.check")).is_none(), "the tour runs now instead");
    h.get_by_label("Take the tour now").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Onboarding(OnboardingMsg::StartTour))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn tour_cards() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "tour-left");
    st.onboarding.tour = Some(TourStep::LeftPanel);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("tour_left_panel");
    h.get_by_label("Next").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Onboarding(OnboardingMsg::TourNext))));
    drop(h);
    remove_fixture(&dir);

    let (mut st, dir) = with_repo(ThemeChoice::Dark, "tour-palette");
    st.onboarding.tour = Some(TourStep::Palette);
    snapshot("tour_palette", st, true);
    remove_fixture(&dir);

    let (mut st, dir) = with_repo(ThemeChoice::Dark, "tour-hidden");
    st.settings.show_details_panel = false;
    st.onboarding.tour = Some(TourStep::Details);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("tour_hidden_panel");
    h.get_by_label("Show it").click();
    h.run();
    let shown = LayoutMsg::SetPanel { panel: Panel::Details, shown: true };
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Layout(l) if *l == shown)));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn the_tour_hides_under_a_dialog() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "tour-under-welcome");
    st.onboarding.tour = Some(TourStep::Graph);
    let mut w = Welcome::new();
    w.identity.seed(ada());
    st.onboarding.welcome = Some(w);
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("Close the tour").is_none(), "not drawn under the welcome");
    h.state_mut().0.onboarding.welcome = None;
    h.run();
    assert!(h.query_by_label("Close the tour").is_some(), "back once it closes");
    drop(h);
    remove_fixture(&dir);
}
