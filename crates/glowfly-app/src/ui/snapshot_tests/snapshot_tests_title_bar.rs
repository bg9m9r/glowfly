//! The title bar Glowfly draws on Windows and macOS: snapshots of the bar (dark, light,
//! maximized, full screen, macOS, Close hovered, the menu and its File submenu, Preferences, German, many
//! tabs), and what its parts send: the window buttons, moving and maximizing from the empty
//! bar or the only tab, resizing from the edges, all of it over a dialog, and the Alt tap. With the system
//! frame none of it is drawn.

use crate::msg::{AppMsg, Msg, TabsMsg};
use crate::state::window_chrome::{Chrome, ResizeEdge, WindowCmd, WindowInfo};
use crate::state::{AppState, Modal, PrefPage, PrefsState, Tab};
use crate::ui::menu_testing::open_submenu;
use crate::ui::snapshot_check::CheckSnapshot;
use crate::ui::snapshot_tests::{harness_sized, remove_fixture, with_repo};
use crate::ui::tabs_tests::pointer;
use eframe::egui::{self, Event, Key, Modifiers, PointerButton, Pos2, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glowfly_core::i18n::Lang;
use glowfly_core::settings::ThemeChoice;
use std::path::PathBuf;

type H = Harness<'static, (AppState, Vec<Msg>)>;

/// The window size the app opens at.
const WINDOW: egui::Vec2 = vec2(1440.0, 880.0);
/// A point on the bar's empty space, between the tabs and the window buttons.
const EMPTY: Pos2 = pos2(1000.0, 20.0);

fn fixture(chrome: Chrome, theme: ThemeChoice, name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    st.view.chrome = chrome;
    st.settings.reduce_motion = true;
    (st, dir)
}

fn open(st: AppState, dark: bool) -> H {
    let mut h = harness_sized(st, dark, WINDOW);
    h.run();
    h
}

fn settle(h: &mut H) {
    for _ in 0..4 {
        h.step();
    }
    h.run();
}

/// The window actions sent so far.
fn sent(h: &H) -> Vec<WindowCmd> {
    h.state().1.iter().filter_map(|m| if let Msg::App(AppMsg::Window(c)) = m { Some(*c) } else { None }).collect()
}

fn quit_sent(h: &H) -> bool {
    h.state().1.iter().any(|m| matches!(m, Msg::App(AppMsg::Quit)))
}

/// Presses at `at`, moves 20 px right and releases.
fn drag_from(h: &mut H, at: Pos2) {
    pointer(h, at, Some((PointerButton::Primary, true)));
    h.step();
    pointer(h, at + vec2(10.0, 0.0), None);
    h.step();
    pointer(h, at + vec2(20.0, 0.0), None);
    h.step();
    pointer(h, at + vec2(20.0, 0.0), Some((PointerButton::Primary, false)));
    h.run();
}

fn snapshot(name: &str, chrome: Chrome, theme: ThemeChoice, dark: bool, change: impl FnOnce(&mut AppState), act: impl FnOnce(&mut H)) {
    let (mut st, dir) = fixture(chrome, theme, name);
    change(&mut st);
    let mut h = open(st, dark);
    act(&mut h);
    settle(&mut h);
    h.check_snapshot(name);
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn title_bar_snapshots() {
    use Chrome::{MacOs, Windows};
    use ThemeChoice::{Dark, Light};
    snapshot("title_bar_windows_dark", Windows, Dark, true, |_| {}, |_| {});
    snapshot("title_bar_windows_light", Windows, Light, false, |_| {}, |_| {});
    snapshot("title_bar_windows_maximized", Windows, Dark, true, |st| st.view.window.maximized = true, |_| {});
    snapshot("title_bar_mac_dark", MacOs, Dark, true, |_| {}, |_| {});
    snapshot("title_bar_close_hover", Windows, Dark, true, |_| {}, |h| h.get_by_label("Close window").hover());
    snapshot("title_bar_menu_open", Windows, Dark, true, |_| {}, |h| h.get_by_label("Menu").click());
    let file = |h: &mut H| {
        h.get_by_label("Menu").click();
        h.run();
        open_submenu(h, "File");
    };
    snapshot("title_bar_menu_file", Windows, Dark, true, |_| {}, file);
    let prefs = |st: &mut AppState| st.prefs = Some(PrefsState::new(PrefPage::General));
    snapshot("title_bar_preferences", Windows, Dark, true, prefs, |_| {});
    let german = |st: &mut AppState| st.settings.language = "de".into();
    snapshot("title_bar_german", Windows, Dark, true, german, |h| h.get_by_label(Lang::De.tr("title_bar.menu.tip")).click());
}

#[test]
fn title_bar_many_tabs() {
    let (mut st, dir) = fixture(Chrome::Windows, ThemeChoice::Dark, "title-bar-many-tabs");
    let base = glowfly_core::paths::temp_dir().join("glowfly-title-bar-tabs");
    for name in ["docs", "api-server", "a-repository-with-a-very-long-folder-name", "web", "infra", "tools", "notes"] {
        st.tabs.push(Tab::unloaded(base.join(name)));
    }
    let mut h = open(st, true);
    settle(&mut h);
    h.check_snapshot("title_bar_many_tabs");
    // The drag space before the buttons stays empty.
    h.state_mut().1.clear();
    drag_from(&mut h, pos2(1440.0 - 138.0 - 30.0, 20.0));
    assert_eq!(sent(&h), [WindowCmd::StartDrag]);
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn the_window_buttons_send_their_actions() {
    let (st, dir) = fixture(Chrome::Windows, ThemeChoice::Dark, "title-bar-buttons");
    let mut h = open(st, true);
    h.get_by_label("Minimize window").click();
    h.run();
    assert_eq!(sent(&h), [WindowCmd::Minimize]);
    h.state_mut().1.clear();
    h.get_by_label("Maximize window").click();
    h.run();
    assert_eq!(sent(&h), [WindowCmd::SetMaximized(true)]);
    h.state_mut().1.clear();
    h.state_mut().0.view.window.maximized = true;
    h.run();
    assert!(h.query_by_label("Maximize window").is_none());
    h.get_by_label("Restore window").click();
    h.run();
    assert_eq!(sent(&h), [WindowCmd::SetMaximized(false)]);
    h.get_by_label("Close window").click();
    h.run();
    assert!(quit_sent(&h), "Close does what File > Quit does");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn title_bar_windows_full_screen() {
    let (mut st, dir) = fixture(Chrome::Windows, ThemeChoice::Dark, "title-bar-full-screen");
    st.view.window.fullscreen = true;
    let mut h = open(st, true);
    settle(&mut h);
    h.check_snapshot("title_bar_windows_full_screen");
    let t = h.state().0.settings.lang();
    assert!(h.query_by_label(t.tr("title_bar.maximize")).is_none(), "the middle button is Restore");
    h.get_by_label(t.tr("title_bar.restore")).click();
    h.run();
    // `update` turns it into leaving full screen.
    assert_eq!(sent(&h), [WindowCmd::SetMaximized(false)]);
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn the_empty_bar_moves_and_maximizes_the_window() {
    let (st, dir) = fixture(Chrome::Windows, ThemeChoice::Dark, "title-bar-drag");
    let mut h = open(st, true);
    drag_from(&mut h, EMPTY);
    assert_eq!(sent(&h), [WindowCmd::StartDrag], "one move, no maximize");
    h.state_mut().1.clear();
    // A double-click: two presses and releases in one frame.
    let events = &mut h.input_mut().events;
    events.push(Event::PointerMoved(EMPTY));
    for pressed in [true, false, true, false] {
        events.push(Event::PointerButton { pos: EMPTY, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
    }
    h.run();
    assert_eq!(sent(&h), [WindowCmd::SetMaximized(true)]);
    h.state_mut().1.clear();
    // A drag that starts on a tab reorders the tabs instead (from the first tab's centre to the
    // second's).
    let (tab, over) = (crate::ui::tabs_tests::tab_centre(&h, 0), crate::ui::tabs_tests::tab_centre(&h, 1));
    pointer(&mut h, tab, Some((PointerButton::Primary, true)));
    h.step();
    pointer(&mut h, tab + vec2(12.0, 0.0), None);
    h.step();
    pointer(&mut h, over, None);
    h.step();
    pointer(&mut h, over, Some((PointerButton::Primary, false)));
    h.run();
    assert!(sent(&h).is_empty(), "{:?}", sent(&h));
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Tabs(TabsMsg::MoveTab { from: 0, to: 1 }))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn dragging_the_only_tab_moves_the_window() {
    for (chrome, moves) in [(Chrome::Windows, true), (Chrome::MacOs, true), (Chrome::System, false)] {
        let (mut st, dir) = fixture(chrome, ThemeChoice::Dark, &format!("title-bar-one-tab-{chrome:?}"));
        st.tabs.truncate(1);
        let mut h = open(st, true);
        let tab = crate::ui::tabs_tests::tab_centre(&h, 0);
        drag_from(&mut h, tab);
        let want: &[WindowCmd] = if moves { &[WindowCmd::StartDrag] } else { &[] };
        assert_eq!(sent(&h), want, "{chrome:?}");
        assert!(!h.state().1.iter().any(|m| matches!(m, Msg::Tabs(TabsMsg::MoveTab { .. }))), "{chrome:?}");
        drop(h);
        remove_fixture(&dir);
    }
}

/// The window actions a press at `at` sends with `chrome`, maximized or not.
fn pressed_at(chrome: Chrome, maximized: bool, at: Pos2) -> Vec<WindowCmd> {
    pressed_in(chrome, WindowInfo { maximized, ..WindowInfo::default() }, at)
}

/// The window actions a press at `at` sends with `chrome` and the window in state `window`.
fn pressed_in(chrome: Chrome, window: WindowInfo, at: Pos2) -> Vec<WindowCmd> {
    let (m, f) = (window.maximized, window.fullscreen);
    let (mut st, dir) = fixture(chrome, ThemeChoice::Dark, &format!("title-bar-resize-{chrome:?}-{m}-{f}-{}", at.x));
    st.view.window = window;
    let mut h = open(st, true);
    pointer(&mut h, at, Some((PointerButton::Primary, true)));
    h.run();
    pointer(&mut h, at, Some((PointerButton::Primary, false)));
    h.run();
    let out = sent(&h);
    drop(h);
    remove_fixture(&dir);
    out
}

#[test]
fn the_edges_resize_the_window_on_windows_only() {
    assert_eq!(pressed_at(Chrome::Windows, false, pos2(1.0, 400.0)), [WindowCmd::BeginResize(ResizeEdge::West)]);
    assert_eq!(pressed_at(Chrome::Windows, false, pos2(1438.0, 878.0)), [WindowCmd::BeginResize(ResizeEdge::SouthEast)]);
    assert!(pressed_at(Chrome::Windows, true, pos2(1.0, 400.0)).is_empty(), "not while maximized");
    let full = WindowInfo { fullscreen: true, ..WindowInfo::default() };
    assert!(pressed_in(Chrome::Windows, full, pos2(1.0, 400.0)).is_empty(), "not in full screen");
    assert!(pressed_at(Chrome::MacOs, false, pos2(1.0, 400.0)).is_empty());
    assert!(pressed_at(Chrome::System, false, pos2(1.0, 400.0)).is_empty());
}

#[test]
fn the_frame_works_over_a_dialog() {
    let (mut st, dir) = fixture(Chrome::Windows, ThemeChoice::Dark, "title-bar-modal");
    st.modal = Some(Modal::About);
    let mut h = open(st, true);
    drag_from(&mut h, EMPTY);
    assert_eq!(sent(&h), [WindowCmd::StartDrag], "the bar still moves the window");
    h.get_by_label("Close window").click();
    h.run();
    assert!(quit_sent(&h));
    drop(h);
    remove_fixture(&dir);
}

/// Holds Alt for one run (with `key` pressed meanwhile), then lets go.
fn alt_tap(h: &mut H, key: Option<Key>) {
    h.input_mut().modifiers = Modifiers::ALT;
    h.run();
    if let Some(key) = key {
        h.input_mut().events.push(Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::ALT });
        h.input_mut().events.push(Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers: Modifiers::ALT });
        h.run();
    }
    h.input_mut().modifiers = Modifiers::NONE;
    h.run();
}

#[test]
fn an_alt_tap_opens_and_closes_the_menu() {
    let (st, dir) = fixture(Chrome::Windows, ThemeChoice::Dark, "title-bar-alt");
    let mut h = open(st, true);
    assert!(h.query_by_label("File").is_none(), "no menu bar");
    alt_tap(&mut h, None);
    assert!(h.query_by_label("File").is_some(), "the menu opened");
    alt_tap(&mut h, None);
    assert!(h.query_by_label("File").is_none(), "a second tap closes it");
    alt_tap(&mut h, Some(Key::J));
    assert!(h.query_by_label("File").is_none(), "Alt with a key is no tap");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn the_system_frame_draws_none_of_it() {
    let (mut st, dir) = fixture(Chrome::System, ThemeChoice::Dark, "title-bar-system");
    st.settings.show_menu_bar = true;
    let mut h = open(st, true);
    for name in ["Menu", "Minimize window", "Maximize window", "Close window"] {
        assert!(h.query_by_label(name).is_none(), "{name}");
    }
    assert!(h.query_by_label("File").is_some(), "the menu bar");
    drag_from(&mut h, pos2(1000.0, 10.0));
    assert!(sent(&h).is_empty());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn drawn_chrome_drops_the_menu_bar_toggle() {
    for (chrome, offered) in [(Chrome::Windows, false), (Chrome::MacOs, false), (Chrome::System, true)] {
        let (mut st, dir) = fixture(chrome, ThemeChoice::Dark, &format!("title-bar-toggle-{chrome:?}"));
        st.settings.show_menu_bar = true;
        let mut h = open(st, true);
        if chrome.drawn() {
            h.get_by_label("Menu").click();
            h.run();
            open_submenu(&mut h, "View");
        } else {
            h.get_by_label("View").click();
            h.run();
        }
        assert_eq!(h.query_by_label("Menu bar").is_some(), offered, "{chrome:?}: View > Menu bar");
        drop(h);
        remove_fixture(&dir);
        let (mut st, dir) = fixture(chrome, ThemeChoice::Dark, &format!("title-bar-toggle-prefs-{chrome:?}"));
        st.prefs = Some(PrefsState::new(PrefPage::Ui));
        let h = open(st, true);
        assert_eq!(h.query_by_label("Show the menu bar").is_some(), offered, "{chrome:?}: Appearance");
        drop(h);
        remove_fixture(&dir);
    }
}
