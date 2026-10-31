//! The terminal panel's header: three pills (an automatic name, a command from the window title,
//! an ended shell with its amber dot), the + and the shell picker, the right-hand buttons and
//! what each pushes, the × on the active and on a hovered pill, and the row's 40 px height and
//! centre line; and the toolbar Terminal button's right-click item, which follows Preferences >
//! Terminal.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use super::snapshot_tests_select::{click, double_click};
use super::snapshot_tests_terminal::{pushed, with_session};
use crate::msg::{Msg, PrefsMsg};
use crate::state::terminal::{SessionStatus, TermSession};
use crate::state::{AppState, PrefPage};
use crate::ui::snapshot_check::CheckSnapshot;
use crate::ui::theme::spacing;
use crate::ui::widgets::tab_glyph;
use eframe::egui::{Modifiers, PointerButton, Rect, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::ThemeChoice;
use glowfly_core::terminal::TermScreen;
use glowfly_model::terminal::TerminalMsg;
use std::path::PathBuf;

/// The header's buttons, left to right after the strip.
pub(crate) const BUTTONS: [&str; 6] =
    ["Clear", "Kill terminal", "More terminal actions", "Terminal settings", "Maximize panel", "Hide panel"];

/// A session of the fixture repository numbered `id`, showing `output`.
fn session(st: &AppState, id: u64, output: &[u8], status: SessionStatus) -> TermSession {
    let mut screen = TermScreen::new(24, 80);
    screen.feed(output);
    let workdir = st.snap.as_ref().unwrap().workdir.clone();
    let (shell, name, selection) = (Default::default(), None, None);
    TermSession { id, number: id as u32, workdir, program: "pwsh".into(), shell, name, screen, status, selection }
}

/// Three shells of the fixture repository: "pwsh · demo", one titled "cargo test" (active), and
/// one that exited with code 1.
fn three_sessions(name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_session(ThemeChoice::Dark, name, SessionStatus::Running);
    let two =
        session(&st, 2, b"\x1b]0;cargo test\x07PS C:\\work\\demo> cargo test\r\n   Compiling demo v0.1.0\r\n", SessionStatus::Running);
    let three = session(&st, 3, b"PS C:\\work\\demo> exit 1\r\n", SessionStatus::Exited(Some(1)));
    st.terminal.sessions.extend([two, three]);
    let workdir = st.snap.as_ref().unwrap().workdir.clone();
    st.terminal.active.insert(workdir, 2);
    st.terminal.next_id = 4;
    (st, dir)
}

/// The bounds of the widget labelled `label`.
pub(crate) fn bounds(h: &Harness<'_, (AppState, Vec<Msg>)>, label: &str) -> Rect {
    let b = h.get_by_label(label).bounding_box().unwrap_or_else(|| panic!("{label} has bounds"));
    Rect::from_min_max(pos2(b.x0 as f32, b.y0 as f32), pos2(b.x1 as f32, b.y1 as f32))
}

/// Which message a click should push.
type Want = fn(&TerminalMsg) -> bool;

/// Whether a message `want` matches was pushed since the last clear.
fn pushed_msg(h: &Harness<'_, (AppState, Vec<Msg>)>, want: fn(&Msg) -> bool) -> bool {
    h.state().1.iter().any(want)
}

#[test]
fn terminal_tabs_dark() {
    let (st, dir) = three_sessions("terminal-tabs");
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("terminal_tabs");
    for label in [
        "pwsh \u{b7} demo",
        "cargo test",
        "pwsh \u{b7} demo (3)",
        "Close cargo test",
        "New terminal tab",
        "New terminal with another shell",
    ]
    .into_iter()
    .chain(BUTTONS)
    {
        assert!(h.query_by_label(label).is_some(), "{label}");
    }
    assert!(h.query_by_label("Close pwsh \u{b7} demo").is_none(), "no × on an inactive pill");
    let clicks: [(&str, Want); 7] = [
        ("Clear", |t| matches!(t, TerminalMsg::Clear)),
        ("Kill terminal", |t| matches!(t, TerminalMsg::Kill)),
        ("Maximize panel", |t| matches!(t, TerminalMsg::ToggleMaximize)),
        ("Hide panel", |t| matches!(t, TerminalMsg::Hide)),
        ("New terminal tab", |t| matches!(t, TerminalMsg::NewTab(None))),
        ("pwsh \u{b7} demo", |t| matches!(t, TerminalMsg::SelectTab(1))),
        ("Close cargo test", |t| matches!(t, TerminalMsg::CloseTab(2))),
    ];
    for (label, want) in clicks {
        h.state_mut().1.clear();
        h.get_by_label(label).click();
        h.run();
        assert!(pushed(&h, want), "{label}");
    }
    h.state_mut().1.clear();
    h.get_by_label("Terminal settings").click();
    h.run();
    assert!(pushed_msg(&h, |m| matches!(m, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Terminal)))));
    h.get_by_label("More terminal actions").click();
    h.run();
    h.get_by_label("Kill all terminals…").click();
    h.run();
    assert!(pushed(&h, |t| matches!(t, TerminalMsg::AskKillAll)), "two shells run");
    let at = bounds(&h, "pwsh \u{b7} demo (3)").center();
    h.state_mut().1.clear();
    double_click(&mut h, at, Modifiers::NONE);
    assert!(pushed(&h, |t| matches!(t, TerminalMsg::StartRename(3))));
    assert!(!pushed(&h, |t| matches!(t, TerminalMsg::SelectTab(_))), "a double-click only renames");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn the_header_is_one_40_px_row_on_one_centre_line() {
    let (st, dir) = three_sessions("terminal-tabs-line");
    let mut h = harness(st, true);
    h.run();
    let line = bounds(&h, "cargo test").center().y;
    let labels = ["pwsh \u{b7} demo", "pwsh \u{b7} demo (3)", "Close cargo test", "New terminal tab", "New terminal with another shell"];
    for label in labels.into_iter().chain(BUTTONS) {
        let b = bounds(&h, label);
        assert!((b.center().y - line).abs() <= 1.0, "{label} at {b:?}, the pill's centre at {line}");
    }
    // The grid starts one item gap under the 40 px row (and its hairline).
    let grid = bounds(&h, "Terminal screen");
    let row_bottom = grid.top() - spacing::ITEM_SPACING.y;
    assert!((row_bottom - line - 20.0).abs() <= 1.0, "the row ends at {row_bottom}, its centre is at {line}");
    let (plus, close) = (bounds(&h, "New terminal tab"), bounds(&h, "Close cargo test"));
    assert_eq!(plus.size(), close.size(), "the + and the × have one size");
    assert_eq!(plus.size(), vec2(tab_glyph::HIT, tab_glyph::HIT));
    h.get_by_label("New terminal tab").hover();
    h.run();
    h.check_snapshot("terminal_tabs_plus_hover");
    h.get_by_label("pwsh \u{b7} demo").hover();
    h.run();
    assert!(h.query_by_label("Close pwsh \u{b7} demo").is_some(), "the × shows on a hovered pill");
    h.check_snapshot("terminal_tabs_close_hover");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn one_terminal_is_one_pill() {
    let (st, dir) = with_session(ThemeChoice::Dark, "terminal-one-tab", SessionStatus::Running);
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("pwsh \u{b7} demo").is_some());
    assert!(h.query_by_label("Close pwsh \u{b7} demo").is_some(), "the active pill's ×");
    let (plus, hide) = (bounds(&h, "New terminal tab"), bounds(&h, "Hide panel"));
    assert!((plus.center().y - hide.center().y).abs() <= 1.0, "+ at {plus:?}, Hide at {hide:?}");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn toolbar_right_click_follows_the_setting() {
    for (in_panel, item) in [(false, "Open in external terminal"), (true, "New terminal tab")] {
        let (mut st, dir) = with_repo(ThemeChoice::Dark, if in_panel { "terminal-button-panel" } else { "terminal-button-external" });
        st.settings.terminal.open_in_panel = in_panel;
        let mut h = harness(st, true);
        h.run();
        let b = h.get_by_label("Show the terminal panel").bounding_box().expect("the toolbar button");
        click(&mut h, pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32), PointerButton::Secondary);
        h.get_by_label(item).click();
        h.run();
        assert!(h.state().1.iter().any(|m| matches!(m, Msg::OpenTerminal)), "{item}");
        drop(h);
        remove_fixture(&dir);
    }
}
