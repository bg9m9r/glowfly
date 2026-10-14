//! The built-in terminal panel: a running shell's screen (from canned output, no real shell),
//! an ended session, the starting and failed states, keyboard forwarding and Ctrl+`. Shares
//! the fixture repository and renderer with `snapshot_tests`.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use super::{UiCache, avatars::Avatars, draw, theme::color};
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::terminal::{SessionStatus, TermSession};
use crate::terminal::TerminalMsg;
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{self, Event, Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use trellis_core::settings::ThemeChoice;
use trellis_core::spell::Speller;
use trellis_core::terminal::{TermInput, TermKey, TermScreen};

/// A short session: a prompt, `git status` with a green and a red line, bold, 256-colour and
/// inverse text, and a prompt waiting with the cursor after it.
const OUTPUT: &str = "PS C:\\work\\demo> git status\r\n\
On branch main\r\n\
Changes to be committed:\r\n\
\x1b[32m        modified:   README.md\x1b[0m\r\n\
Changes not staged for commit:\r\n\
\x1b[31m        modified:   src/app.rs\x1b[0m\r\n\
\r\n\
\x1b[1mBuild finished\x1b[0m in 2.4s\r\n\
\x1b[38;5;208mwarning:\x1b[0m 1 unused import, see \x1b[7m cargo fix \x1b[0m\r\n\
PS C:\\work\\demo> ";

/// The fixture repository with the panel open on a session in `status` that has shown OUTPUT.
fn with_session(theme: ThemeChoice, name: &str, status: SessionStatus) -> (AppState, std::path::PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    let mut screen = TermScreen::new(24, 80);
    if status != SessionStatus::Starting {
        screen.feed(OUTPUT.as_bytes());
    }
    let workdir = st.snap.as_ref().unwrap().workdir.clone();
    st.terminal.sessions.push(TermSession { id: 1, workdir, program: "pwsh".into(), screen, status });
    st.terminal.next_id = 2;
    st.terminal.open = true;
    st.terminal.focus_requested = true;
    (st, dir)
}

fn pushed(h: &Harness<'_, (AppState, Vec<Msg>)>, want: fn(&TerminalMsg) -> bool) -> bool {
    h.state().1.iter().any(|m| matches!(m, Msg::Terminal(t) if want(t)))
}

#[test]
fn terminal_panel_dark() {
    let (st, dir) = with_session(ThemeChoice::Dark, "terminal-panel", SessionStatus::Running);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("terminal_panel");
    assert!(pushed(&h, |t| matches!(t, TerminalMsg::Resize { .. })), "the grid is not 24x80");
    assert!(h.query_by_label("Kill").is_some());
    assert!(h.query_by_label("Restart").is_none());
    assert!(h.query_by_label("Terminal · pwsh · demo").is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn terminal_exited_light() {
    let (st, dir) = with_session(ThemeChoice::Light, "terminal-exited", SessionStatus::Exited(Some(0)));
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("terminal_exited_light");
    assert!(h.query_by_label("Process exited with code 0").is_some());
    assert!(h.query_by_label("Kill").is_none());
    h.query_all_by_label("Restart").next().expect("Restart").click();
    h.run();
    assert!(pushed(&h, |t| matches!(t, TerminalMsg::Restart)));
    h.get_by_label("Hide").click();
    h.run();
    assert!(pushed(&h, |t| matches!(t, TerminalMsg::Hide)));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn starting_and_failed_states() {
    let (st, dir) = with_session(ThemeChoice::Dark, "terminal-starting", SessionStatus::Starting);
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("Starting shell…").is_some());
    drop(h);
    remove_fixture(&dir);

    let (st, dir) = with_session(ThemeChoice::Dark, "terminal-failed", SessionStatus::Failed("boom".into()));
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label_contains("boom").is_some());
    assert!(h.query_all_by_label("Restart").next().is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn focused_terminal_forwards_keys() {
    let (st, dir) = with_session(ThemeChoice::Dark, "terminal-keys", SessionStatus::Running);
    let mut h = harness(st, true);
    h.run();
    assert!(h.state().0.terminal.focused, "the terminal took the focus it was asked for");
    h.state_mut().1.clear();
    h.input_mut().events.push(Event::Text("ls".into()));
    h.press_key(Key::Enter);
    h.press_key(Key::Escape);
    h.run();
    let inputs: Vec<TermInput> = h
        .state()
        .1
        .iter()
        .filter_map(|m| match m {
            Msg::Terminal(TerminalMsg::Input(v)) => Some(v.clone()),
            _ => None,
        })
        .flatten()
        .collect();
    let key = |key| TermInput::Key { key, mods: Default::default() };
    assert_eq!(inputs, [TermInput::Text("ls".into()), key(TermKey::Enter), key(TermKey::Escape)]);
    assert!(h.state().0.terminal.focused, "Esc stays in the terminal");
    assert!(!h.state().1.iter().any(|m| matches!(m, Msg::CloseDiff | Msg::CollapseSelection)));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn ctrl_backtick_toggles() {
    let (st, dir) = with_session(ThemeChoice::Dark, "terminal-shortcut", SessionStatus::Running);
    let mut cache: Option<UiCache> = None;
    let mut h = Harness::builder().with_size(egui::vec2(1400.0, 820.0)).build_state(
        move |ctx, (st, msgs): &mut (AppState, Vec<Msg>)| {
            color::set_dark(true);
            super::terminal::shortcut(ctx, st, msgs);
            let cache = cache.get_or_insert_with(|| UiCache {
                avatars: Avatars::default(),
                speller: Speller::new(),
                diff_rows: Default::default(),
            });
            draw(ctx, st, cache, msgs);
        },
        (st, Vec::new()),
    );
    h.run();
    h.press_key_modifiers(Modifiers::CTRL, Key::Backtick);
    h.run();
    assert!(pushed(&h, |t| matches!(t, TerminalMsg::Toggle)));

    h.state_mut().1.clear();
    h.state_mut().0.palette = Some(Default::default());
    h.press_key_modifiers(Modifiers::CTRL, Key::Backtick);
    h.run();
    assert!(!pushed(&h, |t| matches!(t, TerminalMsg::Toggle)), "the open palette keeps the keyboard");
    drop(h);
    remove_fixture(&dir);
}
