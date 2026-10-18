//! Preferences > In-App Terminal snapshots: the page with its defaults, and with a custom shell
//! whose arguments have an unmatched quote; its navigation entry and the shell choices this
//! system offers.

use super::prefs::terminal::choice_label;
use super::snapshot_tests::{harness_sized, settings};
use crate::state::{AppState, PrefPage, PrefsState};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use std::time::Instant;
use trellis_core::i18n::Lang;
use trellis_core::settings::{ShellChoice, ShellSetting, TerminalPrefs, ThemeChoice};

fn page(terminal: TerminalPrefs) -> AppState {
    let mut s = settings(ThemeChoice::Dark);
    s.terminal = terminal;
    let mut st = AppState::new(s, Vec::new(), Instant::now());
    st.prefs = Some(PrefsState::new(PrefPage::Terminal));
    st
}

#[test]
fn prefs_terminal_dark() {
    let mut h = harness_sized(page(TerminalPrefs::default()), true, eframe::egui::vec2(1400.0, 900.0));
    h.run();
    h.check_snapshot("prefs_terminal");
    assert!(h.query_all_by_label("In-App Terminal").next().is_some(), "the nav entry and title");
    assert!(h.query_by_label_contains("Separated by spaces").is_none(), "arguments only for a custom program");
    // The menu lists exactly this system's choices.
    h.get_by_role(Role::ComboBox).click();
    h.run();
    for c in
        [ShellChoice::Auto, ShellChoice::Pwsh, ShellChoice::WindowsPowerShell, ShellChoice::Cmd, ShellChoice::Bash, ShellChoice::Custom]
    {
        let offered = ShellChoice::offered(cfg!(windows)).contains(&c);
        let label = choice_label(Lang::En, c, cfg!(windows));
        assert_eq!(h.query_all_by_label(label).next().is_some(), offered, "{label}");
    }
}

#[test]
fn prefs_terminal_custom_dark() {
    let shell = ShellSetting { choice: ShellChoice::Custom, program: "nu".into(), args: "--login \"--config x".into() };
    let mut h = harness_sized(page(TerminalPrefs { shell, ..TerminalPrefs::default() }), true, eframe::egui::vec2(1400.0, 1000.0));
    h.run();
    h.check_snapshot("prefs_terminal_custom");
    assert!(h.query_by_label_contains("Separated by spaces").is_some());
    assert!(h.query_by_label_contains("unmatched quote").is_some());
}
