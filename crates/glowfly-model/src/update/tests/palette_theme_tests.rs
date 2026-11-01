//! Headless tests for the palette's theme commands: every built-in theme and every theme file,
//! running one, and which of them recents remember.

use super::tests::state;
use super::update;
use crate::effect::Effect;
use crate::msg::{Msg, PaletteMsg, PrefsMsg};
use crate::state::AppState;
use crate::state::palette::{PaletteCommand, shown};
use glowfly_core::settings::themes::parse;
use glowfly_core::settings::{ThemeCatalog, ThemeChoice};
use std::path::Path;
use std::time::Instant;

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn with_ocean() -> AppState {
    let mut st = state();
    let ocean = parse("ocean", Path::new("ocean.json"), br#"{"name": "Ocean", "base": "light"}"#);
    st.themes.catalog = ThemeCatalog { files: vec![ocean], ..ThemeCatalog::default() };
    st
}

#[test]
fn theme_entries_list_built_ins_and_files() {
    let mut st = with_ocean();
    st.settings.theme = ThemeChoice::Custom("ocean".into());
    let list = shown(&st, "Theme:");
    let themes: Vec<(String, Option<String>)> =
        list.into_iter().filter(|s| s.entry.label.starts_with("Theme: ")).map(|s| (s.entry.label, s.entry.detail)).collect();
    let labels: Vec<&str> = themes.iter().map(|(l, _)| l.as_str()).collect();
    for want in [
        "Theme: Dusk",
        "Theme: Dawn",
        "Theme: Follow system",
        "Theme: High contrast dark",
        "Theme: High contrast light",
        "Theme: Midnight",
        "Theme: Mist",
        "Theme: Graphite",
        "Theme: Porcelain",
        "Theme: Ocean",
    ] {
        assert!(labels.contains(&want), "{want} in {labels:?}");
    }
    assert_eq!(labels.len(), 10);
    let current: Vec<&str> = themes.iter().filter(|(_, d)| d.as_deref() == Some("current")).map(|(l, _)| l.as_str()).collect();
    assert_eq!(current, ["Theme: Ocean"]);
    assert!(shown(&st, "custom").iter().any(|s| s.entry.label == "Theme: Ocean"), "files match \"custom\"");
}

#[test]
fn running_a_file_theme_sets_and_saves_it() {
    let mut st = with_ocean();
    run(&mut st, Msg::Palette(PaletteMsg::TogglePalette));
    let ocean = ThemeChoice::Custom("ocean".into());
    let fx = run(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(PaletteCommand::Theme(ocean.clone()))));
    assert_eq!(st.settings.theme, ocean);
    assert!(matches!(&fx[..], [Effect::SaveSettings(s)] if s.theme == ocean), "{fx:?}");
    assert!(run(&mut st, Msg::Prefs(PrefsMsg::SetTheme(ocean.clone()))).is_empty(), "saved only when it changes");
    let fx = run(&mut st, Msg::Prefs(PrefsMsg::SetTheme(ThemeChoice::Custom("forest".into()))));
    assert!(matches!(&fx[..], [Effect::SaveSettings(_)]), "{fx:?}");
    assert_eq!(st.palette_recent.position("theme-porcelain"), None);
    assert!(st.palette_recent.keys().iter().all(|k| !k.contains("ocean")), "files aren't remembered");
}

#[test]
fn recents_remember_built_in_themes_only() {
    let mut st = with_ocean();
    run(&mut st, Msg::Palette(PaletteMsg::TogglePalette));
    run(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(PaletteCommand::Theme(ThemeChoice::Porcelain))));
    assert_eq!(st.palette_recent.keys(), ["theme-porcelain"]);
    run(&mut st, Msg::Palette(PaletteMsg::TogglePalette));
    run(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(PaletteCommand::Theme(ThemeChoice::Custom("ocean".into())))));
    assert_eq!(st.palette_recent.keys(), ["theme-porcelain"]);
    assert_eq!(PaletteCommand::Theme(ThemeChoice::HighContrastDark).recall_key(), Some("theme-high-contrast-dark"));
    assert_eq!(PaletteCommand::Theme(ThemeChoice::HighContrastLight).recall_key(), Some("theme-high-contrast-light"));
}
