//! What waits for a repository to be trusted outside the trust prompt: the built-in terminal's
//! held tab (a shield, why, and Trust and run…).

use super::snapshot_tests::{harness, remove_fixture};
use super::snapshot_tests_terminal::with_session;
use super::snapshot_tests_trust::make_untrusted;
use crate::msg::{Msg, TrustMsg};
use crate::state::terminal::SessionStatus;
use crate::state::trust::record;
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use trellis_core::settings::{RepoTrust, ThemeChoice};

#[test]
fn terminal_held() {
    let (mut st, dir) = with_session(ThemeChoice::Dark, "terminal-held", SessionStatus::Held);
    let real = make_untrusted(&mut st, &dir);
    // Maximised, tall enough for the shield; the light one is a default-height panel, without it.
    st.terminal.maximized = true;
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("terminal_held");
    assert!(h.query_by_label("The terminal waits until you trust this repository").is_some());
    assert!(h.query_by_label_contains("Shell prompts often run git").is_some());
    h.get_by_label("Trust and run…").click();
    h.run();
    let asked = h.state().1.iter().any(|m| matches!(m, Msg::Trust(TrustMsg::AskForTerminal { real: r }) if *r == real));
    assert!(asked, "Trust and run… asks on the shells' behalf");
    h.get_by_label("pwsh \u{b7} demo").hover();
    for _ in 0..4 {
        h.run();
    }
    assert!(h.query_by_label_contains("Waiting until you trust this repository").is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn terminal_held_refused_light() {
    let (mut st, dir) = with_session(ThemeChoice::Light, "terminal-held-refused", SessionStatus::Held);
    let real = make_untrusted(&mut st, &dir);
    record(&mut st.settings, &real, RepoTrust::NotTrusted);
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("terminal_held_refused_light");
    assert!(h.query_by_label_contains("You chose not to trust this repository.").is_some());
    drop(h);
    remove_fixture(&dir);
}
