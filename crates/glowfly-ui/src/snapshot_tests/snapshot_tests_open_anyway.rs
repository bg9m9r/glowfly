//! Opening a folder whose Git data points to network places: the question the failed open asks
//! (Open anyway accepts; Cancel closes and is the default, so Enter never opens), and
//! Preferences > Trusted repositories listing the folders opened anyway with their places, where
//! Forget only asks update (`TrustMsg`).

use super::snapshot_check::CheckSnapshot;
use super::snapshot_tests::{harness, settings};
use eframe::egui::Key;
use egui_kittest::kittest::Queryable;
use glowfly_core::error::{NetworkPlaces, ReadFailed};
use glowfly_core::settings::{SavedTrust, ThemeChoice};
use glowfly_model::effect::LoadKind;
use glowfly_model::msg::{Msg, RepoMsg, TrustMsg};
use glowfly_model::state::{AppState, Modal, PrefPage, PrefsState};
use std::path::PathBuf;
use std::time::Instant;

/// A work folder that is absolute on every system.
fn at(name: &str) -> PathBuf {
    PathBuf::from(if cfg!(windows) { r"C:\work" } else { "/home/me/work" }).join(name)
}

/// The places used here, as the check names them on this system.
fn places() -> Vec<String> {
    if cfg!(windows) { vec![r"\\nas\team".to_owned(), "Z:\\".to_owned()] } else { vec!["/net/nas".to_owned()] }
}

/// Opening `app` refused for `places()`: the state the question is drawn over.
fn asked() -> AppState {
    let mut st = AppState::new(settings(ThemeChoice::Light), Vec::new(), Instant::now());
    let path = at("app");
    glowfly_model::update::update(&mut st, Msg::Repo(RepoMsg::Open(path.clone())), Instant::now());
    let n = NetworkPlaces { folder: path.clone(), places: places() };
    let why = Box::new(ReadFailed::NetworkPointer(n));
    let result = Err(ReadFailed::OpenFailed { path: glowfly_core::native_path::shown(&path), why });
    let generation = st.load_gen;
    glowfly_model::update::update(&mut st, Msg::Repo(RepoMsg::RepoLoaded { generation, kind: LoadKind::Open, result }), Instant::now());
    st
}

#[test]
fn open_folder_pointing_to_network_place() {
    let st = asked();
    assert!(matches!(&st.modal, Some(Modal::Confirm { .. })) && st.toasts.is_empty(), "the question, no toast");
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("open_folder_pointing_to_network_place");
    assert!(h.query_by_label("Open a folder that points to a network place?").is_some());
    assert!(h.query_by_label_contains(&places()[0]).is_some(), "names the place");
    h.get_by_label("Open anyway").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::ConfirmAccepted)), "the button accepts");
    let mut h = harness(asked(), false);
    h.run();
    h.get_by_label("Cancel").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::CloseModal)), "Cancel closes");
}

#[test]
fn enter_on_the_network_question_cancels() {
    let mut h = harness(asked(), false);
    h.run();
    assert!(h.get_by_label("Cancel").is_focused(), "Cancel is the default");
    h.press_key(Key::Enter);
    h.run();
    let msgs = std::mem::take(&mut h.state_mut().1);
    assert!(msgs.iter().any(|m| matches!(m, Msg::CloseModal)), "Enter cancels");
    assert!(!msgs.iter().any(|m| matches!(m, Msg::ConfirmAccepted)), "Enter never opens anyway");
    // The same question without the flag: Enter accepts, as in every other confirmation.
    let mut st = asked();
    if let Some(Modal::Confirm { cancel_default, .. }) = &mut st.modal {
        *cancel_default = false;
    }
    let mut h = harness(st, false);
    h.run();
    h.press_key(Key::Enter);
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::ConfirmAccepted)), "another confirmation accepts on Enter");
}

/// Preferences on Trusted repositories: one decision and two folders opened anyway.
fn page() -> AppState {
    let mut st = AppState::new(settings(ThemeChoice::Dark), Vec::new(), Instant::now());
    st.settings.set_trust(&at("tools"), Some(SavedTrust::trusted(None)));
    st.settings.approve_network_places(&at("app"), &places());
    st.settings.approve_network_places(&at("shared-build"), &places()[..1]);
    let mut prefs = PrefsState::new(PrefPage::Trust);
    prefs.trust.missing = Some(Vec::new());
    st.prefs = Some(prefs);
    st
}

#[test]
fn prefs_trust_network_places() {
    let mut h = harness(page(), true);
    h.run();
    h.check_snapshot("prefs_trust_network_places");
    assert!(h.query_by_label("Network places allowed").is_some());
    h.get_by_label("Forget the network places allowed for shared-build").click();
    h.run();
    let msgs = std::mem::take(&mut h.state_mut().1);
    assert!(msgs.iter().any(|m| matches!(m, Msg::Trust(TrustMsg::ForgetNetworkPlaces(p)) if *p == at("shared-build"))), "{}", msgs.len());
}
