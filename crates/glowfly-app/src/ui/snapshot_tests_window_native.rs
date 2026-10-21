//! The drawn Windows frame's ties to the OS, as far as the views see them: the Maximize button
//! drawn hovered and pressed from the OS's tracking (Snap Layouts), and a right-click on the
//! empty title bar (or the drag zone over a dialog) asking for the system menu.

use crate::msg::{AppMsg, Msg};
use crate::state::window_chrome::{Chrome, WindowCmd};
use crate::state::window_native::CaptionButton;
use crate::state::{AppState, Modal};
use crate::ui::snapshot_check::CheckSnapshot;
use crate::ui::snapshot_tests::{harness_sized, remove_fixture, with_repo};
use crate::ui::tabs_tests::pointer;
use eframe::egui::{PointerButton, Pos2, pos2, vec2};
use egui_kittest::Harness;
use glowfly_core::settings::ThemeChoice;

type H = Harness<'static, (AppState, Vec<Msg>)>;

/// A point on the bar's empty space, between the tabs and the window buttons.
const EMPTY: Pos2 = pos2(1000.0, 20.0);

fn open(name: &str, chrome: Chrome, theme: ThemeChoice, change: impl FnOnce(&mut AppState)) -> (H, std::path::PathBuf) {
    let dark = theme == ThemeChoice::Dark;
    let (mut st, dir) = with_repo(theme, name);
    st.view.chrome = chrome;
    st.settings.reduce_motion = true;
    change(&mut st);
    let mut h = harness_sized(st, dark, vec2(1440.0, 880.0));
    h.run();
    (h, dir)
}

fn native_snapshot(name: &str, theme: ThemeChoice, button: CaptionButton) {
    let (mut h, dir) = open(name, Chrome::Windows, theme, |st| st.view.window.max_button = button);
    for _ in 0..4 {
        h.step();
    }
    h.run();
    h.check_snapshot(name);
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn the_os_tracked_maximize_button_draws_hover_and_press() {
    native_snapshot("title_bar_max_hover_native", ThemeChoice::Dark, CaptionButton { hovered: true, pressed: false });
    native_snapshot("title_bar_max_pressed_native", ThemeChoice::Light, CaptionButton { hovered: true, pressed: true });
}

/// The system menu requests sent so far.
fn menus_sent(h: &H) -> usize {
    h.state().1.iter().filter(|m| matches!(m, Msg::App(AppMsg::Window(WindowCmd::SystemMenu)))).count()
}

fn right_click(h: &mut H, at: Pos2) {
    pointer(h, at, Some((PointerButton::Secondary, true)));
    h.step();
    pointer(h, at, Some((PointerButton::Secondary, false)));
    h.run();
}

#[test]
fn right_click_on_the_empty_bar_opens_the_system_menu() {
    for (chrome, menus) in [(Chrome::Windows, 1), (Chrome::MacOs, 0)] {
        let (mut h, dir) = open(&format!("system-menu-bar-{chrome:?}"), chrome, ThemeChoice::Dark, |_| {});
        right_click(&mut h, EMPTY);
        assert_eq!(menus_sent(&h), menus, "{chrome:?}");
        drop(h);
        remove_fixture(&dir);
    }
    // Over a dialog the drag zone keeps it.
    let (mut h, dir) = open("system-menu-modal", Chrome::Windows, ThemeChoice::Dark, |st| st.modal = Some(Modal::About));
    right_click(&mut h, EMPTY);
    assert_eq!(menus_sent(&h), 1, "over a dialog");
    drop(h);
    remove_fixture(&dir);
    // A tab keeps its own right-click.
    let (mut h, dir) = open("system-menu-tab", Chrome::Windows, ThemeChoice::Dark, |_| {});
    let tab = crate::ui::tabs_tests::tab_centre(&h, 0);
    right_click(&mut h, tab);
    assert_eq!(menus_sent(&h), 0, "on a tab");
    drop(h);
    remove_fixture(&dir);
}
