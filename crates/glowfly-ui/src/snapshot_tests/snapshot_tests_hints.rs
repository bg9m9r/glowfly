//! The tip row (widgets/hint_row.rs): the Undo tip under the graph's header in Graphite and
//! Porcelain, the Connect tip in a 240 px left panel (its button under the text) above a
//! collapsed PULL REQUESTS section, the Tidy tip in German, its × menu, and Preferences >
//! General's Help and tips.

use super::snapshot_tests::{harness, harness_sized, remove_fixture, settings, with_repo};
use super::snapshot_tests_forge_filters::target;
use super::snapshot_tests_worktrees::pushed;
use crate::msg::Msg;
use crate::onboarding::OnboardingMsg;
use crate::state::hints::{HintId, ShownHint};
use crate::state::{AppState, ForgeState, PrefPage, PrefsState, Status};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::vec2;
use egui_kittest::kittest::Queryable;
use glowfly_core::forge::Provider;
use glowfly_core::repo::RemoteInfo;
use glowfly_core::settings::ThemeChoice;
use std::time::Instant;

/// Shows tip `id` in the active tab.
fn show(st: &mut AppState, id: HintId) {
    let tab = st.tabs[st.active_tab].id;
    st.onboarding.hint = Some(ShownHint { id, tab });
}

fn undo_in(theme: ThemeChoice, dark: bool, name: &str, golden: &str) {
    let (mut st, dir) = with_repo(theme, name);
    show(&mut st, HintId::Undo);
    let mut h = harness(st, dark);
    h.run();
    h.check_snapshot(golden);
    let key = h.state().0.keymap.hint(crate::keymap::BindingId::Undo).unwrap().to_owned();
    let text = format!("{key} undoes that, and the operation journal keeps every earlier state of your branches.");
    assert!(h.query_by_label(&text).is_some(), "the row is a group named by its sentence");
    h.get_by_label("Open journal").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Onboarding(OnboardingMsg::HintAct(HintId::Undo)))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn hint_row_undo() {
    undo_in(ThemeChoice::Graphite, true, "hint-row-undo", "hint_row_undo");
}

#[test]
fn hint_row_undo_light() {
    undo_in(ThemeChoice::Porcelain, false, "hint-row-undo-light", "hint_row_undo_light");
}

#[test]
fn hint_row_connect() {
    let (mut st, dir) = with_repo(ThemeChoice::Graphite, "hint-row-connect");
    st.settings.panel_sizes.sidebar = Some(240.0);
    // As the tip really shows: PULL REQUESTS collapsed (an open one shows its own Connect…), and
    // the repository's `origin` on GitHub.
    st.view.sections.prs = false;
    let snap = st.snap.as_mut().unwrap();
    let main = snap.locals.iter().find(|b| b.name == "main").and_then(|b| b.oid).unwrap();
    snap.remotes = vec![("origin".into(), vec![("main".into(), main)])];
    let url = "https://github.com/o/r.git".to_owned();
    snap.remote_info = vec![RemoteInfo { name: "origin".into(), urls: vec![url], push_urls: Vec::new(), skip_fetch_all: false }];
    let mut f = ForgeState::new(Some(target(Provider::GitHub, "github.com")), "origin".into(), None);
    f.host_status = Status::Ok;
    st.session.forge = Some(f);
    show(&mut st, HintId::Connect);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("hint_row_connect");
    assert!(h.query_by_label("Connect your GitHub account to see this repository's pull requests here.").is_some());
    h.get_by_label("Connect…").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Onboarding(OnboardingMsg::HintAct(HintId::Connect)))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn hint_row_tidy_de() {
    let (mut st, dir) = with_repo(ThemeChoice::Graphite, "hint-row-tidy-de");
    st.settings.language = "de".into();
    show(&mut st, HintId::TidyCommits);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("hint_row_tidy_de");
    assert!(h.query_by_label("Commits aufräumen…").is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn hint_row_menu() {
    let (mut st, dir) = with_repo(ThemeChoice::Graphite, "hint-row-menu");
    show(&mut st, HintId::Undo);
    let mut h = harness(st, true);
    h.run();
    h.get_by_label("Tip options").click();
    h.run();
    h.check_snapshot("hint_row_menu");
    h.get_by_label("Don't show this again").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Onboarding(OnboardingMsg::DismissHint(HintId::Undo)))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn prefs_general_tips() {
    let mut st = AppState::new(settings(ThemeChoice::Graphite), Vec::new(), Instant::now());
    st.prefs = Some(PrefsState::new(PrefPage::General));
    let mut h = harness_sized(st, true, vec2(1400.0, 1500.0));
    h.run();
    h.check_snapshot("prefs_general_tips");
    h.get_by_label("Show tips again").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Onboarding(OnboardingMsg::ResetHints))));
    h.get_by_label("Show tips while I work").click();
    h.run();
    assert!(!h.state().0.settings.hints, "the checkbox edits the setting");
}
