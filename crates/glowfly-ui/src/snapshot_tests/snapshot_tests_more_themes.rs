//! Midnight, Mist, Graphite and Porcelain, each as the main window over a graph of four pinned
//! branches with a commit's details open (`theme_<id>`), and Preferences' theme picker
//! (`prefs_theme_picker*`, `prefs_theme_custom_open`): Match the system's pair and the Custom
//! theme files disclosure.

use super::snapshot_tests::{harness_palette_sized, remove_fixture, settings};
use super::snapshot_tests_graph_bands::with_bands;
use super::snapshot_tests_themes::ocean_prefs;
use super::theme::{self, builtin, color::Palette};
use crate::snapshot_check::CheckSnapshot;
use eframe::egui::accesskit::{Role, Toggled};
use eframe::egui::vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glowfly_core::i18n::Lang;
use glowfly_core::repo;
use glowfly_core::settings::{BuiltinTheme, Settings, SystemPair, ThemeChoice};
use glowfly_model::state::{AppState, PrefPage, PrefsState, Selection};
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
fn theme_graphite() {
    main_in(BuiltinTheme::Graphite);
}

#[test]
fn theme_porcelain() {
    main_in(BuiltinTheme::Porcelain);
}

/// Preferences > Appearance with `choice` picked, drawn in its own palette.
fn picker(name: &str, choice: ThemeChoice) {
    let p = *builtin(choice.builtin(true, SystemPair::default()).unwrap_or(BuiltinTheme::DEFAULT_DARK));
    let mut h = prefs(settings(choice), p);
    h.check_snapshot(name);
}

/// Preferences > Appearance over `s`, drawn in `p` at the picker goldens' size.
fn prefs(s: Settings, p: Palette) -> Harness<'static, (AppState, Vec<glowfly_model::msg::Msg>)> {
    let mut st = AppState::new(s, Vec::new(), Instant::now());
    st.prefs = Some(PrefsState::new(PrefPage::Ui));
    let mut h = harness_palette_sized(st, p, vec2(1400.0, 1040.0));
    h.run();
    h
}

const MIDNIGHT_PORCELAIN: SystemPair = SystemPair { dark: BuiltinTheme::Midnight, light: BuiltinTheme::Porcelain };

#[test]
fn prefs_theme_picker() {
    // The default: Graphite.
    picker("prefs_theme_picker", ThemeChoice::Graphite);
    picker("prefs_theme_picker_porcelain", ThemeChoice::Porcelain);
}

#[test]
fn prefs_theme_picker_midnight() {
    let _ = theme::MIDNIGHT;
    picker("prefs_theme_picker_midnight", ThemeChoice::Midnight);
}

#[test]
fn prefs_theme_picker_system() {
    // The test desktop is dark, so Match the system shows the pair's dark member.
    let s = Settings { system_pair: MIDNIGHT_PORCELAIN, ..settings(ThemeChoice::System) };
    prefs(s, theme::MIDNIGHT).check_snapshot("prefs_theme_picker_system");
}

#[test]
fn prefs_theme_picker_de() {
    let s = Settings { language: "de".into(), ..settings(ThemeChoice::System) };
    prefs(s, theme::GRAPHITE).check_snapshot("prefs_theme_picker_de");
}

#[test]
fn prefs_theme_custom_open() {
    let (st, p) = ocean_prefs();
    let mut h = harness_palette_sized(st, p, vec2(1400.0, 1040.0));
    h.run();
    h.check_snapshot("prefs_theme_custom_open");
}

#[test]
fn theme_files_disclosure_open_only_with_a_file() {
    let (section, file) = (Lang::En.tr("prefs.ui.theme.files.section"), Lang::En.tr("prefs.ui.theme.files.label"));
    let mut h = prefs(settings(ThemeChoice::Graphite), theme::GRAPHITE);
    assert!(h.query_by_label(file).is_none(), "closed with a built-in theme");
    // It reports its expanded state (`theme_files`' tests read the value).
    assert!(h.get_by_label(section).supports_expand_collapse());
    h.get_by_label(section).click();
    h.run();
    assert!(h.query_by_label(file).is_some(), "a click opens it");
    drop(h);

    let (st, p) = ocean_prefs();
    let mut h = harness_palette_sized(st, p, vec2(1400.0, 1040.0));
    h.run();
    assert!(h.query_by_label(file).is_some(), "open while a theme file is in use");
    h.get_by_label(section).click();
    h.run();
    assert!(h.query_by_label(file).is_some(), "and a click leaves it open");
}

#[test]
fn system_mode_shows_two_selected_cards() {
    let h = prefs(Settings { system_pair: MIDNIGHT_PORCELAIN, ..settings(ThemeChoice::System) }, theme::MIDNIGHT);
    // The theme cards among the page's radio buttons.
    let selected: Vec<String> = h
        .query_all_by_role(Role::RadioButton)
        .filter(|n| n.toggled() == Some(Toggled::True))
        .filter_map(|n| n.label())
        .filter(|l| BuiltinTheme::ALL.iter().any(|b| b.label() == l))
        .collect();
    assert_eq!(selected, ["Midnight", "Porcelain"]);
}
