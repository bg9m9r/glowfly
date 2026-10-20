//! The terminal's 16 basic colours in every built-in theme: each as text, each as a background
//! under text, and bold 0-7 (drawn in the bright colours).

use super::snapshot_tests::{harness_palette, remove_fixture};
use super::snapshot_tests_terminal::with_session;
use super::theme::builtin;
use crate::state::terminal::SessionStatus;
use crate::ui::snapshot_check::CheckSnapshot;
use trellis_core::settings::{BuiltinTheme, ThemeChoice};
use trellis_core::terminal::TermScreen;

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
        h.check_snapshot(&name);
        drop(h);
        remove_fixture(&dir);
    }
}
