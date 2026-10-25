//! Headless tests for full screen and bringing hidden side panels back: the toggle asks the shell
//! and saves nothing, a full-screen window drops moves and resizes and leaves full screen on
//! Maximize or Restore, the palette entry follows the window, and the edge strips' message.

use super::tests::{scratch_repo, state};
use super::update;
use crate::effect::{Effect, UiEffect};
use crate::keymap::BindingId;
use crate::msg::{AppMsg, LayoutMsg, Msg, PaletteMsg};
use crate::state::AppState;
use crate::state::blame::BlameView;
use crate::state::layout::{DetailsShape, Panel, details_shape, shown};
use crate::state::palette::{PaletteCommand as C, entries};
use crate::state::window_chrome::{ResizeEdge, WindowCmd};
use glowfly_core::repo::{self, LoadOptions};
use std::time::Instant;

fn send(st: &mut AppState, m: Msg) -> Vec<Effect> {
    update(st, m, Instant::now())
}

fn saves(fx: &[Effect]) -> bool {
    fx.iter().any(|e| matches!(e, Effect::SaveSettings(_)))
}

/// The window commands among `fx`.
fn window_cmds(fx: &[Effect]) -> Vec<WindowCmd> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Ui(UiEffect::Window(c)) => Some(*c),
            _ => None,
        })
        .collect()
}

fn settings_json(st: &AppState) -> String {
    serde_json::to_string(&st.settings).unwrap()
}

fn labels(st: &AppState) -> Vec<String> {
    entries(st).into_iter().map(|e| e.label).collect()
}

#[test]
fn toggle_fullscreen_enters_then_leaves() {
    let mut st = state();
    assert!(!st.view.window.fullscreen);
    let before = settings_json(&st);
    let fx = send(&mut st, Msg::Layout(LayoutMsg::ToggleFullscreen));
    assert!(matches!(&fx[..], [Effect::Ui(UiEffect::Window(WindowCmd::SetFullscreen(true)))]), "{fx:?}");
    assert!(!saves(&fx));
    assert_eq!(settings_json(&st), before, "settings unchanged");
    assert!(st.toasts.is_empty());
    // The shell reports the window full screen on the next frame.
    st.view.window.fullscreen = true;
    let fx = send(&mut st, Msg::Layout(LayoutMsg::ToggleFullscreen));
    assert!(matches!(&fx[..], [Effect::Ui(UiEffect::Window(WindowCmd::SetFullscreen(false)))]), "{fx:?}");
    assert_eq!(settings_json(&st), before, "settings unchanged");
    assert!(st.toasts.is_empty());
}

#[test]
fn fullscreen_is_never_saved() {
    let mut st = state();
    let before = settings_json(&st);
    send(&mut st, Msg::Layout(LayoutMsg::ToggleFullscreen));
    st.view.window.fullscreen = true;
    send(&mut st, Msg::Layout(LayoutMsg::ToggleFullscreen));
    assert_eq!(settings_json(&st), before);
}

#[test]
fn fullscreen_keeps_nothing_for_inactive_tabs() {
    assert!(LayoutMsg::ToggleFullscreen.on_inactive().is_none());
}

#[test]
fn window_actions_in_full_screen() {
    let all = [
        WindowCmd::StartDrag,
        WindowCmd::BeginResize(ResizeEdge::SouthEast),
        WindowCmd::Minimize,
        WindowCmd::SetMaximized(true),
        WindowCmd::SetMaximized(false),
        WindowCmd::SystemMenu,
        WindowCmd::SetFullscreen(true),
        WindowCmd::SetFullscreen(false),
    ];
    let mut st = state();
    st.view.window.fullscreen = true;
    let mut run = |cmd| window_cmds(&send(&mut st, Msg::App(AppMsg::Window(cmd))));
    assert!(run(WindowCmd::StartDrag).is_empty(), "no moving a full-screen window");
    assert!(run(WindowCmd::BeginResize(ResizeEdge::SouthEast)).is_empty(), "nor resizing it");
    assert_eq!(run(WindowCmd::SetMaximized(true)), [WindowCmd::SetFullscreen(false)]);
    assert_eq!(run(WindowCmd::SetMaximized(false)), [WindowCmd::SetFullscreen(false)], "Restore leaves full screen");
    assert_eq!(run(WindowCmd::Minimize), [WindowCmd::Minimize]);
    assert_eq!(run(WindowCmd::SystemMenu), [WindowCmd::SystemMenu]);
    let mut st = state();
    for cmd in all {
        assert_eq!(window_cmds(&send(&mut st, Msg::App(AppMsg::Window(cmd)))), [cmd], "{cmd:?} windowed");
    }
}

#[test]
fn palette_full_screen_entry_follows_the_window() {
    let mut st = state();
    let all = labels(&st);
    assert!(all.iter().any(|l| l == "Enter full screen"), "{all:?}");
    assert!(!all.iter().any(|l| l == "Leave full screen"));
    let entry = entries(&st).into_iter().find(|e| e.label == "Enter full screen").unwrap();
    assert_eq!(entry.shortcut(&st.keymap), st.keymap.hint(BindingId::ToggleFullscreen));
    assert!(entry.shortcut(&st.keymap).is_some(), "the key is shown");
    let at = |l: &str| all.iter().position(|x| x == l).unwrap_or_else(|| panic!("{l} offered"));
    assert!(at("Enter full screen") < at("Hide left panel"), "before the panel entries");
    assert!(st.snap.is_none(), "offered without a repository");
    st.view.window.fullscreen = true;
    let all = labels(&st);
    assert!(all.iter().any(|l| l == "Leave full screen"));
    assert!(!all.iter().any(|l| l == "Enter full screen"));
}

#[test]
fn palette_runs_full_screen() {
    let mut st = state();
    send(&mut st, Msg::Palette(PaletteMsg::TogglePalette));
    let fx = send(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::Layout(LayoutMsg::ToggleFullscreen))));
    assert!(st.palette.is_none());
    assert_eq!(window_cmds(&fx), [WindowCmd::SetFullscreen(true)]);
    assert_eq!(st.palette_recent.keys(), ["full-screen"]);
}

#[test]
fn edge_strip_message_brings_side_panels_back() {
    let mut st = state();
    for panel in [Panel::Left, Panel::Details] {
        send(&mut st, Msg::Layout(LayoutMsg::SetPanel { panel, shown: false }));
        assert!(!shown(&st.settings, panel));
        // What the panel's edge strip sends.
        let fx = send(&mut st, Msg::Layout(LayoutMsg::SetPanel { panel, shown: true }));
        assert!(shown(&st.settings, panel), "{panel:?}");
        assert!(saves(&fx), "{panel:?}");
        assert!(!saves(&send(&mut st, Msg::Layout(LayoutMsg::SetPanel { panel, shown: true }))), "{panel:?} again");
    }

    // Beside a tool view, the strip brings back the rail: the expansion is per tool view.
    let dir = scratch_repo("edge-strip-blame");
    let mut st = state();
    st.snap = Some(repo::load(&dir, &LoadOptions::all(100)).unwrap());
    st.blame = Some(BlameView::loading("a.txt".into(), None));
    send(&mut st, Msg::Layout(LayoutMsg::SetPanel { panel: Panel::Details, shown: false }));
    assert_eq!(details_shape(&st), DetailsShape::Hidden);
    send(&mut st, Msg::Layout(LayoutMsg::SetPanel { panel: Panel::Details, shown: true }));
    assert_eq!(details_shape(&st), DetailsShape::Rail);
    drop(st);
    let _ = std::fs::remove_dir_all(dir);
}
