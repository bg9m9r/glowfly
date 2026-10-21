//! Preferences > Terminal snapshots: the page with its defaults (the built-in and external
//! terminal sections), and with a custom shell
//! whose arguments have an unmatched quote; its navigation entry and the shell choices this
//! system offers; the page as Linux draws it, with Middle-click paste.

use super::prefs::terminal::{choice_label, page_for};
use super::snapshot_tests::{harness_sized, settings};
use super::theme::{self, color};
use crate::state::{AppState, PrefPage, PrefsState};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{self, accesskit::Role};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glowfly_core::i18n::Lang;
use glowfly_core::settings::{ShellChoice, ShellSetting, TerminalPrefs, ThemeChoice};
use std::time::Instant;

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
    assert!(h.query_all_by_label("Terminal").next().is_some(), "the nav entry and title");
    assert!(h.query_by_label("Open a tab in the built-in terminal").is_some());
    assert!(h.query_by_label_contains("Open in terminal doesn't open a built-in tab").is_some(), "the external terminal moved here");
    assert!(h.query_by_label_contains("Separated by spaces").is_none(), "arguments only for a custom program");
    assert!(h.query_by_label_contains("Selecting text with the mouse").is_some());
    assert_eq!(h.query_by_label_contains("A middle click on the terminal").is_some(), cfg!(target_os = "linux"), "only where it acts");
    // The menu lists exactly this system's choices.
    h.query_all_by_role(Role::ComboBox).next().expect("the shell menu").click();
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

#[test]
fn prefs_terminal_linux() {
    let mut s = settings(ThemeChoice::Dark);
    s.terminal = TerminalPrefs::default();
    let mut h = Harness::builder().with_size(egui::vec2(900.0, 640.0)).build_state(
        |ctx, s: &mut glowfly_core::settings::Settings| {
            color::set_palette(&theme::DARK);
            theme::apply_style(ctx, &theme::DARK);
            ctx.set_theme(theme::egui_theme(&theme::DARK));
            egui::CentralPanel::default().show(ctx, |ui| page_for(ui, s, true, &mut Vec::new()));
        },
        s,
    );
    h.run();
    h.check_snapshot("prefs_terminal_linux");
    assert!(h.query_by_label_contains("Selecting text with the mouse").is_some());
    assert!(h.query_by_label_contains("A middle click on the terminal").is_some());
}
