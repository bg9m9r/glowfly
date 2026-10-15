//! Preferences > External Tools snapshots: the page with nothing set (the terminal arguments
//! disabled), and with an editor, custom arguments naming an unknown placeholder, and a custom
//! terminal.

use super::snapshot_tests::{render, settings};
use crate::state::{AppState, PrefPage, PrefsState};
use std::time::Instant;
use trellis_core::settings::{ExternalTools, ThemeChoice};

fn page(theme: ThemeChoice, tools: ExternalTools) -> AppState {
    let mut s = settings(theme);
    s.external_tools = tools;
    let mut st = AppState::new(s, Vec::new(), Instant::now());
    // No git configuration is needed, so the page draws without waiting for it.
    st.prefs = Some(PrefsState::new(PrefPage::Tools));
    st
}

#[test]
fn preferences_tools() {
    render("prefs_tools", page(ThemeChoice::Dark, ExternalTools::default()), true);
}

#[test]
fn preferences_tools_custom_light() {
    let tools = ExternalTools {
        editor: "C:/Tools/Editor/editor.exe".into(),
        editor_file_args: "--goto \"{path}:{line}:{col}\"".into(),
        terminal: "wt".into(),
        terminal_args: "-d \"{repo}\"".into(),
        ..ExternalTools::default()
    };
    render("prefs_tools_custom_light", page(ThemeChoice::Light, tools), false);
}
