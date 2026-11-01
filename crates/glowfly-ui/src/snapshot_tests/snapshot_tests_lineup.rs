//! The theme lineup: the main window with a word diff open in every built-in theme, to compare
//! the palettes side by side (`snapshot_tests_vivid.rs` has each one over a merge conflict), and
//! the welcome screen's icon and wordmark in Dusk and Dawn.

use super::snapshot_tests::{harness_palette, remove_fixture, settings};
use super::snapshot_tests_diff::with_word_diff_in;
use super::theme::{self, builtin};
use crate::state::AppState;
use crate::ui::snapshot_check::CheckSnapshot;
use glowfly_core::settings::{BuiltinTheme, DiffLayout, ThemeChoice};
use std::time::Instant;

fn diff_in(t: BuiltinTheme) {
    let theme = match t {
        BuiltinTheme::Dark => "dusk".to_owned(),
        BuiltinTheme::Light => "dawn".to_owned(),
        other => other.id().replace('-', "_"),
    };
    let name = format!("lineup_{theme}");
    let (st, dir) = with_word_diff_in(DiffLayout::Inline, ThemeChoice::Dark, &name);
    let mut h = harness_palette(st, *builtin(t));
    h.run();
    h.check_snapshot(&name);
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn lineup_dusk() {
    diff_in(BuiltinTheme::Dark);
}

#[test]
fn lineup_dawn() {
    diff_in(BuiltinTheme::Light);
}

#[test]
fn lineup_porcelain() {
    diff_in(BuiltinTheme::Porcelain);
}

#[test]
fn lineup_high_contrast() {
    diff_in(BuiltinTheme::HighContrastDark);
    diff_in(BuiltinTheme::HighContrastLight);
}

#[test]
fn lineup_welcome() {
    for (name, p) in [("lineup_welcome_dusk", theme::DUSK), ("lineup_welcome_dawn", theme::DAWN)] {
        let st = AppState::new(settings(ThemeChoice::Dark), Vec::new(), Instant::now());
        let mut h = harness_palette(st, p);
        h.run();
        h.check_snapshot(name);
    }
}
