//! Terminal colour schemes (Preferences > Terminal > Colours): Import scheme… shows a file
//! dialog, the runner reads the picked file and writes its schemes to the themes folder's
//! `terminal` folder, and the first one imported becomes the terminal's colours.

use crate::effect::Effect;
use crate::state::AppState;
use crate::themes::ThemeEffect;
use glowfly_core::settings::TerminalColours;
use glowfly_core::settings::terminal_schemes::ImportReport;
use std::time::Instant;

/// What an import wrote, or why it failed: the first scheme becomes the terminal's colours.
pub(super) fn imported(st: &mut AppState, file: &str, result: Result<ImportReport, String>, now: Instant, fx: &mut Vec<Effect>) {
    let report = match result {
        Ok(r) => r,
        Err(e) => return st.log(false, format!("Could not import {file}: {e}")),
    };
    for note in &report.skipped {
        st.log_quiet(false, format!("Skipped in {file}: {note}"));
    }
    let Some((id, name)) = report.saved.first() else { return };
    st.settings.terminal.colours = TerminalColours::Scheme(id.clone());
    fx.extend(super::prefs::save_settings(st));
    fx.push(Effect::Theme(ThemeEffect::Load));
    let mut text = match report.saved.len() {
        1 => format!("Imported the terminal color scheme {name} and switched the terminal to it."),
        n => format!("Imported {n} terminal color schemes from {file}; the terminal now uses {name}."),
    };
    if !report.skipped.is_empty() {
        text.push_str(&format!(" Skipped {}; the activity log says why.", report.skipped.len()));
    }
    st.toast(true, text, now);
}

/// Notes in the log when the scheme the terminal is set to isn't in the folder any more (the
/// terminal then matches the app theme).
pub(super) fn check_selected(st: &mut AppState) {
    if let TerminalColours::Scheme(id) = &st.settings.terminal.colours
        && st.themes.catalog.terminal.get(id).is_none()
    {
        let text = format!(
            "Terminal color scheme {id}.json is not in the themes folder's terminal folder; the terminal uses the app theme's colors."
        );
        st.log_quiet(false, text);
    }
}
