//! The six themes added beside Dusk and Dawn (Midnight, Mist, Ember, Pine, Graphite and
//! Linen), each as the main window over a graph of four pinned branches with a commit's details
//! open (`theme_<id>`), and Preferences' theme picker (`prefs_theme_picker*`).

use super::snapshot_tests::{harness_palette_sized, remove_fixture, settings};
use super::snapshot_tests_graph_bands::with_bands;
use super::theme::{self, builtin};
use crate::state::{AppState, PrefPage, PrefsState, Selection};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::vec2;
use glowfly_core::repo;
use glowfly_core::settings::{BuiltinTheme, ThemeChoice};
use std::sync::Arc;
use std::time::Instant;

fn main_in(t: BuiltinTheme) {
    let name = format!("theme_{}", t.id());
    let (mut st, dir) = with_bands(&format!("theme-{}", t.id()));
    let Selection::Commit(oid) = st.session.selection else { panic!("a commit is selected") };
    st.session.detail = Some(repo::commit_detail(&dir, oid).unwrap());
    st.session.reads.line_counts.start_commit(oid);
    st.session.reads.line_counts.finish_commit(oid, Arc::new(repo::commit_line_counts(&dir, oid).unwrap()));
    let mut h = harness_palette_sized(st, *builtin(t), vec2(1600.0, 820.0));
    h.run_steps(4);
    h.check_snapshot(&name);
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn theme_midnight() {
    main_in(BuiltinTheme::Midnight);
}

#[test]
fn theme_mist() {
    main_in(BuiltinTheme::Mist);
}

#[test]
fn theme_ember() {
    main_in(BuiltinTheme::Ember);
}

#[test]
fn theme_pine() {
    main_in(BuiltinTheme::Pine);
}

#[test]
fn theme_graphite() {
    main_in(BuiltinTheme::Graphite);
}

#[test]
fn theme_linen() {
    main_in(BuiltinTheme::Linen);
}

/// Preferences > Appearance with `choice` picked, drawn in its own palette.
fn picker(name: &str, choice: ThemeChoice) {
    let mut st = AppState::new(settings(choice.clone()), Vec::new(), Instant::now());
    st.prefs = Some(PrefsState::new(PrefPage::Ui));
    let p = *builtin(choice.builtin(true).unwrap_or(BuiltinTheme::DEFAULT_DARK));
    let mut h = harness_palette_sized(st, p, vec2(1400.0, 1040.0));
    h.run();
    h.check_snapshot(name);
}

#[test]
fn prefs_theme_picker() {
    // The default: Graphite.
    picker("prefs_theme_picker", ThemeChoice::Graphite);
    picker("prefs_theme_picker_linen", ThemeChoice::Linen);
}

#[test]
fn prefs_theme_picker_midnight() {
    let _ = theme::MIDNIGHT;
    picker("prefs_theme_picker_midnight", ThemeChoice::Midnight);
}
