//! The terminal's 16 basic colours in every built-in theme: each as text, each as a background
//! under text, and bold 0-7 (drawn in the bright colours); and an imported scheme drawn in place
//! of the app theme's colours.

use super::snapshot_tests::{harness_palette, remove_fixture};
use super::snapshot_tests_terminal::with_session;
use super::theme::{builtin, color, terminal_scheme};
use crate::state::terminal::SessionStatus;
use crate::ui::snapshot_check::CheckSnapshot;
use glowfly_core::settings::terminal_schemes::{SchemeCatalog, import, parse_stored, scheme_json};
use glowfly_core::settings::{BuiltinTheme, TerminalColours, ThemeChoice};
use glowfly_core::terminal::TermScreen;

/// The colour pattern: three rows, every colour numbered.
fn pattern() -> String {
    let mut out = String::new();
    for i in 0..16 {
        let sgr = if i < 8 { 30 + i } else { 90 + i - 8 };
        out.push_str(&format!("\x1b[{sgr}m{i:>3}\x1b[0m"));
    }
    out.push_str("\r\n");
    for i in 0..16 {
        let sgr = if i < 8 { 40 + i } else { 100 + i - 8 };
        out.push_str(&format!("\x1b[{sgr}m{i:>3}\x1b[0m"));
    }
    out.push_str("\r\n");
    for i in 0..8 {
        out.push_str(&format!("\x1b[1;{}m{i:>3}\x1b[0m", 30 + i));
    }
    out.push_str("\r\n");
    out
}

/// The themes whose terminal is a pixel golden.
const GOLDENS: [&str; 3] = ["terminal_ansi_dark", "terminal_ansi_light", "terminal_ansi_high_contrast_dark"];

#[test]
fn terminal_ansi_every_theme() {
    for t in BuiltinTheme::ALL {
        let name = format!("terminal_ansi_{}", t.id().replace('-', "_"));
        let (mut st, dir) = with_session(ThemeChoice::Dark, &name.replace('_', "-"), SessionStatus::Running);
        let mut screen = TermScreen::new(24, 80);
        screen.feed(pattern().as_bytes());
        st.terminal.sessions[0].screen = screen;
        let mut h = harness_palette(st, *builtin(t));
        h.run();
        // The text is the same in every theme: the goldens judge the colours of a few (the
        // first keeps the text).
        if GOLDENS.contains(&name.as_str()) {
            h.check_snapshot(&name);
        }
        drop(h);
        remove_fixture(&dir);
    }
}

/// A dark Windows Terminal scheme, imported, drawn under the light Dawn app theme.
#[test]
fn terminal_custom_scheme() {
    let bytes = include_bytes!("../../../glowfly-core/src/settings/terminal_schemes/fixtures/harbour.json");
    let scheme = import("harbour", bytes).unwrap().schemes.remove(0);
    let file = parse_stored("harbour-night", std::path::Path::new("harbour-night.json"), scheme_json(&scheme).as_bytes());
    let cat = SchemeCatalog { files: vec![file], error: None };
    let app = *builtin(BuiltinTheme::Light);
    let t = terminal_scheme::resolve(&TerminalColours::Scheme("harbour-night".into()), &app, &cat);
    assert_ne!(t, app.ansi);
    let (mut st, dir) = with_session(ThemeChoice::Light, "terminal-custom-scheme", SessionStatus::Running);
    let mut screen = TermScreen::new(24, 80);
    screen.feed(pattern().as_bytes());
    screen.feed(super::snapshot_tests_terminal::OUTPUT.as_bytes());
    st.terminal.sessions[0].screen = screen;
    color::set_terminal(Some(t));
    let mut h = harness_palette(st, app);
    h.run();
    h.check_snapshot("terminal_custom_scheme");
    drop(h);
    color::set_terminal(None);
    remove_fixture(&dir);
}
