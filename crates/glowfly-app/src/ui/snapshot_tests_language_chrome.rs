//! Language snapshots of the window's chrome: the German menu bar, tab bar and labelled toolbar
//! (long labels must not collide), the View and Help menus open in German, and UI Customization
//! in Spanish (its labels must fit their column); and the tab menu, release notes and About
//! follow the language.

use crate::msg::{Msg, UpdateMsg};
use crate::state::menu_bar::VERSION;
use crate::state::release_notes::NotesDialog;
use crate::state::{AppState, Modal, PrefPage, PrefsState};
use crate::ui::snapshot_check::CheckSnapshot;
use crate::ui::snapshot_tests::{harness, harness_sized, remove_fixture, settings, with_repo};
use eframe::egui::{self, Event, Modifiers, PointerButton};
use egui_kittest::kittest::Queryable;
use glowfly_core::app_update::Version;
use glowfly_core::i18n::Lang;
use glowfly_core::settings::ThemeChoice;
use std::path::PathBuf;
use std::time::Instant;

/// The fixture in German, with the menu bar and the toolbar's labels shown.
fn german(name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, name);
    st.settings.language = "de".into();
    st.settings.show_menu_bar = true;
    st.settings.toolbar_labels = true;
    (st, dir)
}

#[test]
fn chrome_german() {
    let (st, dir) = german("chrome-de");
    let mut h = harness(st, true);
    h.run();
    for label in ["Datei", "Bearbeiten", "Ansicht", "Fenster", "Hilfe"] {
        assert!(h.query_by_label(label).is_some(), "{label} is in the menu bar");
    }
    h.check_snapshot("chrome_german");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn menu_bar_view_german() {
    let (st, dir) = german("menu-view-de");
    let mut h = harness(st, true);
    h.run();
    h.get_by_label(Lang::De.tr("menu.view.title")).click();
    h.run();
    assert!(h.query_by_label(Lang::De.tr("menu.view.theme")).is_some());
    h.check_snapshot("menu_bar_view_german");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn menu_bar_help_german() {
    let (st, dir) = german("menu-help-de");
    let mut h = harness(st, true);
    h.run();
    h.get_by_label(Lang::De.tr("menu.help.title")).click();
    h.run();
    h.check_snapshot("menu_bar_help_german");
    h.get_by_label(Lang::De.tr("menu.help.check_updates")).click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Updates(UpdateMsg::CheckNow))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn prefs_ui_spanish() {
    let mut s = settings(ThemeChoice::Dark);
    s.language = "es".into();
    let mut st = AppState::new(s, Vec::new(), Instant::now());
    st.prefs = Some(PrefsState::new(PrefPage::Ui));
    let mut h = harness_sized(st, true, egui::vec2(1400.0, 2400.0));
    h.run();
    assert!(h.query_by_label(Lang::Es.tr("prefs.ui.columns.compact.button")).is_some(), "the last row is drawn");
    h.check_snapshot("prefs_ui_spanish");
}

#[test]
fn tab_menu_and_release_notes_in_german() {
    let (mut st, dir) = german("chrome-queries-de");
    // The tab bar at the top, where the menu bar would be.
    st.settings.show_menu_bar = false;
    let mut h = harness(st, true);
    h.run();
    // Right-click the second tab.
    let at = crate::ui::tabs_tests::tab_centre(&h, 1);
    for pressed in [true, false] {
        let events = &mut h.input_mut().events;
        events.push(Event::PointerMoved(at));
        events.push(Event::PointerButton { pos: at, button: PointerButton::Secondary, pressed, modifiers: Modifiers::NONE });
        h.step();
    }
    h.run();
    assert!(h.query_by_label(Lang::De.tr("tabs.menu.close")).is_some(), "the tab menu is German");
    assert!(h.query_by_label("Close tab").is_none());
    drop(h);

    let (mut st, notes_dir) = german("chrome-notes-de");
    let d = NotesDialog::new("", &st.updates.current, Version::parse("0.1.0"), None);
    st.modal = Some(Modal::ReleaseNotes(Box::new(d)));
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label(Lang::De.tr("release_notes.title")).is_some(), "the release notes are German");
    assert!(h.query_by_label(Lang::De.tr("release_notes.none")).is_some());
    drop(h);

    let (mut st, other) = german("chrome-about-de");
    st.modal = Some(Modal::About);
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label(&Lang::De.trf("about.version", &[("version", &VERSION)])).is_some(), "About is German");
    assert!(h.query_by_label(Lang::De.tr("about.copy_info.button")).is_some());
    drop(h);
    remove_fixture(&dir);
    remove_fixture(&other);
    remove_fixture(&notes_dir);
}
