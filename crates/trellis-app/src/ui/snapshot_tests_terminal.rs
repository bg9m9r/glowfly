//! The built-in terminal panel: a running shell's screen (from canned output, no real shell),
//! an ended session, the starting and failed states, keyboard forwarding, the keys a focused
//! terminal keeps from the app's shortcuts, and Ctrl+`. Shares the fixture repository and
//! renderer with `snapshot_tests`.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use super::{UiCache, avatars::Avatars, draw, theme::color};
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::msg::SelectionMsg;
use crate::msg::TabsMsg;
use crate::state::AppState;
use crate::state::terminal::{SessionStatus, TermSession};
use crate::terminal::{ScrollStep, TerminalMsg};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{self, Event, Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use trellis_core::settings::ThemeChoice;
use trellis_core::spell::Speller;
use trellis_core::terminal::{KeyMods, TermInput, TermKey, TermScreen};

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
pub(super) fn with_session(theme: ThemeChoice, name: &str, status: SessionStatus) -> (AppState, std::path::PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    let mut screen = TermScreen::new(24, 80);
    if status != SessionStatus::Starting {
        screen.feed(OUTPUT.as_bytes());
    }
    let workdir = st.snap.as_ref().unwrap().workdir.clone();
    st.terminal.sessions.push(TermSession {
        id: 1,
        number: 1,
        selection: None,
        workdir,
        program: "pwsh".into(),
        shell: Default::default(),
        name: None,
        screen,
        status,
    });
    st.terminal.next_id = 2;
    st.terminal.open = true;
    st.terminal.focus_requested = true;
    (st, dir)
}

pub(super) fn pushed(h: &Harness<'_, (AppState, Vec<Msg>)>, want: fn(&TerminalMsg) -> bool) -> bool {
    h.state().1.iter().any(|m| matches!(m, Msg::Terminal(t) if want(t)))
}

#[test]
fn terminal_panel_dark() {
    let (st, dir) = with_session(ThemeChoice::Dark, "terminal-panel", SessionStatus::Running);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("terminal_panel");
    assert!(pushed(&h, |t| matches!(t, TerminalMsg::Resize { .. })), "the grid is not 24x80");
    assert!(h.query_by_label("Kill terminal").is_some());
    assert!(h.query_by_label("Restart").is_none(), "Restart is only offered once the shell ends");
    assert!(h.query_by_label("pwsh · demo").is_some());
    drop(h);
    remove_fixture(&dir);
}

/// A long prompt as a pseudoconsole sends it: trimmed of its trailing space, erased to the end
/// of the line and the cursor placed one cell past the space, under a full row ending in red.
const LONG_PROMPT: &str = "\x1b[?25l\x1b[2J\x1b[m\x1b[H\
PS C:\\work> cd trellis-worktrees\\terminal-keeps-the-prompts-space\\src\r\n\
-------------------------------------------------------------------------------\x1b[31m|\x1b[0m\r\n\
PS C:\\work\\trellis-worktrees\\terminal-keeps-the-prompts-space\\src>\x1b[K\x1b[3;68H\x1b[?25h";

#[test]
fn terminal_prompt_space_dark() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "terminal-prompt-space");
    let mut screen = TermScreen::new(24, 80);
    screen.feed(LONG_PROMPT.as_bytes());
    assert_eq!(screen.cursor(), Some((2, 67)), "one cell past the prompt's space");
    let workdir = st.snap.as_ref().unwrap().workdir.clone();
    let status = SessionStatus::Running;
    st.terminal.sessions.push(TermSession {
        id: 1,
        number: 1,
        selection: None,
        workdir,
        program: "pwsh".into(),
        shell: Default::default(),
        name: None,
        screen,
        status,
    });
    st.terminal.next_id = 2;
    st.terminal.open = true;
    st.terminal.focus_requested = true;
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("terminal_prompt_space");
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
    h.query_all_by_label("Restart").next().expect("Restart").click();
    h.run();
    assert!(pushed(&h, |t| matches!(t, TerminalMsg::Restart)));
    h.get_by_label("Hide panel").click();
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
    assert!(!h.state().1.iter().any(|m| matches!(m, Msg::Repo(RepoMsg::CloseDiff) | Msg::Selection(SelectionMsg::CollapseSelection))));
    drop(h);
    remove_fixture(&dir);
}

/// The whole window with the app's shortcuts handled first, as the app runs them.
fn with_shortcuts(st: AppState) -> Harness<'static, (AppState, Vec<Msg>)> {
    let mut cache: Option<UiCache> = None;
    let mut h = Harness::builder().with_size(egui::vec2(1400.0, 820.0)).build_state(
        move |ctx, (st, msgs): &mut (AppState, Vec<Msg>)| {
            color::set_dark(true);
            super::shortcuts::input(ctx, st, msgs);
            let cache = cache.get_or_insert_with(|| UiCache {
                avatars: Avatars::default(),
                speller: Speller::new(),
                diff_rows: Default::default(),
                history_text: Default::default(),
                blame_rows: Default::default(),
                diff_images: Default::default(),
            });
            draw(ctx, st, cache, msgs);
        },
        (st, Vec::new()),
    );
    h.run();
    h
}

/// The terminal input the views pushed.
fn inputs(h: &Harness<'_, (AppState, Vec<Msg>)>) -> Vec<TermInput> {
    h.state()
        .1
        .iter()
        .filter_map(|m| match m {
            Msg::Terminal(TerminalMsg::Input(v)) => Some(v.clone()),
            _ => None,
        })
        .flatten()
        .collect()
}

#[test]
fn focused_terminal_keeps_tab_keys() {
    let (st, dir) = with_session(ThemeChoice::Dark, "terminal-kept-keys", SessionStatus::Running);
    let mut h = with_shortcuts(st);
    assert!(h.state().0.terminal.focused);
    h.state_mut().1.clear();
    // Ctrl as Windows and Linux report it.
    let ctrl = Modifiers { ctrl: true, command: true, ..Modifiers::NONE };
    for (modifiers, key) in [(ctrl, Key::W), (ctrl, Key::T), (Modifiers::NONE, Key::F5)] {
        h.input_mut().modifiers = modifiers;
        h.input_mut().events.push(Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers });
        h.step();
        h.input_mut().modifiers = Modifiers::NONE;
    }
    h.run();
    let ctrl_key = |c| TermInput::Key { key: TermKey::Char(c), mods: KeyMods { ctrl: true, ..Default::default() } };
    assert_eq!(inputs(&h), [ctrl_key('w'), ctrl_key('t'), TermInput::Key { key: TermKey::F(5), mods: KeyMods::default() }]);
    assert!(!h.state().1.iter().any(|m| matches!(m, Msg::Tabs(TabsMsg::CloseTab(_) | TabsMsg::NewTab) | Msg::Repo(RepoMsg::Refresh))));

    h.state_mut().1.clear();
    h.press_key_modifiers(Modifiers::SHIFT, Key::PageUp);
    h.run();
    assert!(pushed(&h, |t| matches!(t, TerminalMsg::ScrollPage(ScrollStep::PageUp))));
    assert!(inputs(&h).is_empty(), "nothing for the shell");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn ctrl_backtick_toggles() {
    let (st, dir) = with_session(ThemeChoice::Dark, "terminal-shortcut", SessionStatus::Running);
    let mut h = with_shortcuts(st);
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
