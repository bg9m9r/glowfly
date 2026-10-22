//! Headless tests for terminal color schemes: Import scheme… (the file dialog, the import, the
//! scheme it switches to), what a failed import logs, and a selected scheme that is gone.

use super::tests::state;
use super::update;
use crate::effect::Effect;
use crate::msg::Msg;
use crate::state::AppState;
use crate::themes::{ThemeEffect, ThemeMsg};
use glowfly_core::settings::terminal_schemes::{ImportReport, SchemeCatalog, parse_stored};
use glowfly_core::settings::{TerminalColours, ThemeCatalog};
use std::path::{Path, PathBuf};
use std::time::Instant;

fn run(st: &mut AppState, m: ThemeMsg) -> Vec<Effect> {
    update(st, Msg::Theme(m), Instant::now())
}

fn imported(saved: &[(&str, &str)], skipped: &[&str]) -> ThemeMsg {
    let report = ImportReport {
        saved: saved.iter().map(|(i, n)| (i.to_string(), n.to_string())).collect(),
        skipped: skipped.iter().map(|s| s.to_string()).collect(),
    };
    ThemeMsg::SchemesImported { file: "settings.json".into(), result: Ok(report) }
}

#[test]
fn import_asks_for_a_file_then_reads_it() {
    let mut st = state();
    assert!(matches!(&run(&mut st, ThemeMsg::PickScheme)[..], [Effect::Theme(ThemeEffect::PickScheme)]));
    assert!(run(&mut st, ThemeMsg::SchemePicked(None)).is_empty(), "cancelled");
    let p = PathBuf::from("Downloads").join("night.itermcolors");
    let fx = run(&mut st, ThemeMsg::SchemePicked(Some(p.clone())));
    assert!(matches!(&fx[..], [Effect::Theme(ThemeEffect::ImportScheme { path })] if *path == p), "{fx:?}");
    assert_eq!(st.settings.terminal.colours, TerminalColours::MatchTheme, "nothing changes until it is imported");
}

#[test]
fn an_import_selects_the_first_scheme_saves_and_reloads() {
    let mut st = state();
    let fx = run(&mut st, imported(&[("paper", "Paper"), ("night", "Night")], &["scheme \"Broken\": bad"]));
    assert_eq!(st.settings.terminal.colours, TerminalColours::Scheme("paper".into()));
    assert!(
        matches!(&fx[..], [Effect::SaveSettings(s), Effect::Theme(ThemeEffect::Load)] if s.terminal.colours == TerminalColours::Scheme("paper".into())),
        "{fx:?}"
    );
    let toast = &st.toasts.last().unwrap();
    assert!(
        toast.ok
            && toast.text
                == "Imported 2 terminal color schemes from settings.json; the terminal now uses Paper. Skipped 1; the activity log says why.",
        "{}",
        toast.text
    );
    assert!(st.log.iter().any(|l| !l.ok && l.text == "Skipped in settings.json: scheme \"Broken\": bad"));

    run(&mut st, imported(&[("solo", "Solo")], &[]));
    assert_eq!(st.settings.terminal.colours, TerminalColours::Scheme("solo".into()));
    assert_eq!(st.toasts.last().unwrap().text, "Imported the terminal color scheme Solo and switched the terminal to it.");
}

#[test]
fn a_failed_import_changes_nothing_and_says_why() {
    let mut st = state();
    st.settings.terminal.colours = TerminalColours::Theme("sand".into());
    let fx = run(&mut st, ThemeMsg::SchemesImported { file: "x.plist".into(), result: Err("it is not valid JSON: eof".into()) });
    assert!(fx.is_empty());
    assert_eq!(st.settings.terminal.colours, TerminalColours::Theme("sand".into()));
    assert!(st.log.last().is_some_and(|l| !l.ok && l.text == "Could not import x.plist: it is not valid JSON: eof"));
}

#[test]
fn match_theme_and_a_missing_scheme() {
    let mut st = state();
    run(&mut st, ThemeMsg::Loaded(ThemeCatalog::default()));
    assert!(st.log.is_empty(), "matching the app theme needs no scheme");
    st.settings.terminal.colours = TerminalColours::Scheme("gone".into());
    run(&mut st, ThemeMsg::Loaded(ThemeCatalog::default()));
    let last = &st.log.last().unwrap().text;
    assert!(last.contains("gone.json") && last.contains("app theme's colors"), "{last}");
    assert!(st.toasts.is_empty(), "quietly");

    let count = st.log.len();
    let file = parse_stored("gone", Path::new("gone.json"), br##"{"colors": {"terminal_red": "#nope"}}"##);
    let cat = ThemeCatalog { terminal: SchemeCatalog { files: vec![file], error: None }, ..ThemeCatalog::default() };
    run(&mut st, ThemeMsg::Loaded(cat));
    assert_eq!(st.log.len(), count + 1, "its problem, and no longer missing");
    assert!(st.log.last().unwrap().text.starts_with("Terminal color scheme gone.json: "));
    assert_eq!(st.themes.catalog.terminal.files.len(), 1);
}

#[test]
fn scheme_messages_are_global() {
    let all =
        [ThemeMsg::PickScheme, ThemeMsg::SchemePicked(None), ThemeMsg::SchemesImported { file: String::new(), result: Err(String::new()) }];
    for m in all {
        assert!(m.on_inactive().is_none());
    }
}
