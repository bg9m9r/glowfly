//! Theme files: reloading the themes folder, opening it, creating a file from the theme in use,
//! importing terminal colour schemes (terminal_schemes.rs), and reporting the problems of what
//! was loaded.

use crate::effect::Effect;
use crate::effect::OsEffect;
use crate::state::AppState;
use crate::themes::{ThemeEffect, ThemeMsg};
use glowfly_core::settings::themes::{BuiltinTheme, starter_json};
use glowfly_core::settings::{ThemeCatalog, ThemeChoice, ThemeProblem};
use std::time::Instant;

pub(super) fn handle(st: &mut AppState, msg: ThemeMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        ThemeMsg::Reload => fx.push(Effect::Theme(ThemeEffect::Load)),
        ThemeMsg::OpenFolder => fx.push(Effect::Theme(ThemeEffect::OpenFolder)),
        ThemeMsg::CreateFromCurrent { colors, dark } => {
            let theme = &st.settings.theme;
            let (label, base) = match theme {
                ThemeChoice::Custom(id) => {
                    let label = st.themes.catalog.get(id).map_or_else(|| id.clone(), |f| f.name.clone());
                    (label, st.themes.catalog.base_of(theme, dark))
                }
                // The view sent the colors in use, so "Match system" copies the theme it shows.
                ThemeChoice::System => {
                    let base = if dark { BuiltinTheme::DEFAULT_DARK } else { BuiltinTheme::DEFAULT_LIGHT };
                    (base.label().to_owned(), base)
                }
                other => (other.label(), st.themes.catalog.base_of(other, dark)),
            };
            let json = starter_json(&format!("{label} (copy)"), base, &colors);
            fx.push(Effect::Theme(ThemeEffect::Create { json }));
        }
        ThemeMsg::Loaded(cat) => loaded(st, cat),
        ThemeMsg::Created(Ok((id, path))) => {
            let file = format!("{id}.json");
            st.settings.theme = ThemeChoice::Custom(id);
            fx.push(super::prefs::save_settings(st));
            fx.push(Effect::Theme(ThemeEffect::Load));
            fx.push(Effect::Os(OsEffect::RevealPath(path)));
            st.toast(true, format!("Created theme {file}. Edit it, then press Reload."), now);
        }
        ThemeMsg::Created(Err(e)) => st.log(false, format!("Could not create a theme file: {e}")),
        ThemeMsg::PickScheme => fx.push(Effect::Theme(ThemeEffect::PickScheme)),
        ThemeMsg::SchemePicked(None) => {}
        ThemeMsg::SchemePicked(Some(path)) => fx.push(Effect::Theme(ThemeEffect::ImportScheme { path })),
        ThemeMsg::SchemesImported { file, result } => super::terminal_schemes::imported(st, &file, result, now, fx),
    }
}

/// Stands for "the file is gone" in `ThemeState::reported`.
fn missing_marker() -> Vec<ThemeProblem> {
    vec![ThemeProblem::Unreadable("not in the themes folder".into())]
}

/// Keeps a freshly read catalog, logs every problem quietly, and announces once when the
/// selected theme file is missing or has new problems.
fn loaded(st: &mut AppState, cat: ThemeCatalog) {
    for f in &cat.files {
        for p in &f.problems {
            st.log_quiet(false, format!("Theme {}: {p}", f.file_name()));
        }
    }
    for note in cat.error.iter().chain(&cat.skipped) {
        st.log_quiet(false, format!("Themes: {note}"));
    }
    for f in &cat.terminal.files {
        for p in &f.problems {
            st.log_quiet(false, format!("Terminal color scheme {}: {p}", f.file_name()));
        }
    }
    if let Some(note) = &cat.terminal.error {
        st.log_quiet(false, format!("Terminal color schemes: {note}"));
    }
    st.themes.catalog = cat;
    st.themes.generation += 1;
    super::terminal_schemes::check_selected(st);
    let ThemeChoice::Custom(id) = &st.settings.theme else {
        st.themes.reported = None;
        return;
    };
    let (problems, text) = match st.themes.catalog.get(id) {
        None => (missing_marker(), format!("Theme {id}.json is not in the themes folder; using {}.", BuiltinTheme::DEFAULT_DARK.label())),
        Some(f) if f.problems.is_empty() => {
            st.themes.reported = None;
            return;
        }
        Some(f) => {
            let text = format!("Theme {} has problems (listed above); colors not set come from {}.", f.file_name(), f.base.label());
            (f.problems.clone(), text)
        }
    };
    let report = Some((id.clone(), problems));
    if st.themes.reported != report {
        st.log(false, text);
        st.themes.reported = report;
    }
}
