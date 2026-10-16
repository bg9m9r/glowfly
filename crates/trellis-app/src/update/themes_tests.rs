//! Headless tests for theme files: reload, the folder, creating a file from the theme in use,
//! and how loaded problems are logged.

use super::tests::state;
use super::update;
use crate::effect::Effect;
use crate::msg::Msg;
use crate::state::AppState;
use crate::themes::{ThemeEffect, ThemeMsg};
use std::path::{Path, PathBuf};
use std::time::Instant;
use trellis_core::settings::themes::parse;
use trellis_core::settings::{BuiltinTheme, ThemeCatalog, ThemeChoice, ThemeFile};

fn run(st: &mut AppState, m: ThemeMsg) -> Vec<Effect> {
    update(st, Msg::Theme(m), Instant::now())
}

fn file(id: &str, text: &str) -> ThemeFile {
    parse(id, Path::new(&format!("{id}.json")), text.as_bytes())
}

fn catalog(files: Vec<ThemeFile>) -> ThemeCatalog {
    ThemeCatalog { files, ..ThemeCatalog::default() }
}

/// The json a CreateFromCurrent asked to write, read back.
fn created(fx: &[Effect]) -> ThemeFile {
    match fx {
        [Effect::Theme(ThemeEffect::Create { json })] => file("new", json),
        other => panic!("{other:?}"),
    }
}

#[test]
fn reload_and_open_folder_are_effects() {
    let mut st = state();
    assert!(matches!(&run(&mut st, ThemeMsg::Reload)[..], [Effect::Theme(ThemeEffect::Load)]));
    assert!(matches!(&run(&mut st, ThemeMsg::OpenFolder)[..], [Effect::Theme(ThemeEffect::OpenFolder)]));
}

#[test]
fn create_from_current_names_and_bases_the_copy() {
    let mut st = state();
    st.settings.theme = ThemeChoice::Dark;
    let colors = vec![("bg", [1, 2, 3])];
    let f = created(&run(&mut st, ThemeMsg::CreateFromCurrent { colors: colors.clone(), dark: true }));
    assert_eq!((f.name.as_str(), f.base, f.colors.len()), ("Trellis Dark (copy)", BuiltinTheme::Dark, 1));
    assert!(f.problems.is_empty());

    st.themes.catalog = catalog(vec![file("ocean", r#"{"name": "Ocean", "base": "light"}"#)]);
    st.settings.theme = ThemeChoice::Custom("ocean".into());
    let f = created(&run(&mut st, ThemeMsg::CreateFromCurrent { colors: colors.clone(), dark: false }));
    assert_eq!((f.name.as_str(), f.base), ("Ocean (copy)", BuiltinTheme::Light));

    st.settings.theme = ThemeChoice::System;
    let f = created(&run(&mut st, ThemeMsg::CreateFromCurrent { colors, dark: false }));
    assert_eq!((f.name.as_str(), f.base), ("Trellis Light (copy)", BuiltinTheme::Light), "the theme the system picked");
}

#[test]
fn created_selects_reloads_and_reveals() {
    let mut st = state();
    let p = PathBuf::from("themes").join("my-theme.json");
    let fx = run(&mut st, ThemeMsg::Created(Ok(("my-theme".into(), p.clone()))));
    assert_eq!(st.settings.theme, ThemeChoice::Custom("my-theme".into()));
    assert!(
        matches!(&fx[..], [Effect::SaveSettings(s), Effect::Theme(ThemeEffect::Load), Effect::RevealPath(r)]
            if s.theme == ThemeChoice::Custom("my-theme".into()) && *r == p),
        "{fx:?}"
    );
    assert!(st.toasts.last().is_some_and(|t| t.ok && t.text == "Created theme my-theme.json. Edit it, then press Reload."));

    let fx = run(&mut st, ThemeMsg::Created(Err("disk full".into())));
    assert!(fx.is_empty());
    assert!(st.log.last().is_some_and(|l| !l.ok && l.text == "Could not create a theme file: disk full"));
    assert!(st.view.show_log);
}

#[test]
fn loaded_logs_problems_and_opens_the_log_once() {
    let mut st = state();
    let bad = || file("ocean", r##"{"colors": {"acent": "#000000", "panel": "#12345"}}"##);
    let fx = run(&mut st, ThemeMsg::Loaded(catalog(vec![bad()])));
    assert!(fx.is_empty());
    assert_eq!((st.themes.generation, st.themes.catalog.files.len()), (1, 1));
    let lines: Vec<&str> = st.log.iter().map(|l| l.text.as_str()).collect();
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(lines.iter().all(|l| l.starts_with("Theme ocean.json: ")), "{lines:?}");
    assert!(!st.view.show_log, "an unselected file's problems stay quiet");

    st.settings.theme = ThemeChoice::Custom("ocean".into());
    run(&mut st, ThemeMsg::Loaded(catalog(vec![bad()])));
    assert_eq!(st.themes.generation, 2);
    assert!(st.view.show_log);
    let last = &st.log.last().unwrap().text;
    assert!(last.contains("ocean.json") && last.contains("Trellis Dark"), "{last}");
    let count = st.log.len();

    st.view.show_log = false;
    run(&mut st, ThemeMsg::Loaded(catalog(vec![bad()])));
    assert_eq!(st.log.len(), count + 2, "only the quiet lines again");
    assert!(!st.view.show_log, "the same problems aren't announced twice");
}

#[test]
fn a_missing_selected_file_is_logged() {
    let mut st = state();
    st.settings.theme = ThemeChoice::Custom("gone".into());
    run(&mut st, ThemeMsg::Loaded(ThemeCatalog::default()));
    let last = &st.log.last().unwrap().text;
    assert!(last.contains("gone.json") && last.contains("Trellis Dark"), "{last}");
    assert!(st.view.show_log);
    st.view.show_log = false;
    run(&mut st, ThemeMsg::Loaded(ThemeCatalog::default()));
    assert!(!st.view.show_log);
}

#[test]
fn theme_messages_are_global() {
    let all = [
        ThemeMsg::Reload,
        ThemeMsg::OpenFolder,
        ThemeMsg::CreateFromCurrent { colors: Vec::new(), dark: true },
        ThemeMsg::Loaded(ThemeCatalog::default()),
        ThemeMsg::Created(Err("x".into())),
    ];
    for m in all {
        assert!(m.on_inactive().is_none());
    }
}
