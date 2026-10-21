//! Theme files: reloading the themes folder, opening it, creating a file from the theme in use,
//! and reporting the problems of what was loaded.

use crate::effect::Effect;
use crate::effect::OsEffect;
use crate::state::AppState;
use crate::themes::{ThemeEffect, ThemeMsg};
use std::time::Instant;
use trellis_core::settings::themes::{BuiltinTheme, starter_json};
use trellis_core::settings::{ThemeCatalog, ThemeChoice, ThemeProblem};

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
                    let base = if dark { BuiltinTheme::Dark } else { BuiltinTheme::Light };
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
    st.themes.catalog = cat;
    st.themes.generation += 1;
    let ThemeChoice::Custom(id) = &st.settings.theme else {
        st.themes.reported = None;
        return;
    };
    let (problems, text) = match st.themes.catalog.get(id) {
        None => (missing_marker(), format!("Theme {id}.json is not in the themes folder; using {}.", BuiltinTheme::Dark.label())),
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
