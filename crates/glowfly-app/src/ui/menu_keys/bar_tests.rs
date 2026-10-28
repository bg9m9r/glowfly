//! The menu bar through the shared menu keyboard: View's Theme submenu opened with Right (not a
//! step to the next menu), Left and Esc closing it alone, typeahead on the bar's menus, and the
//! focus kept inside the open menu. The app's frame, in the app's order (`keys_harness`).

use crate::ui::menu_bar::keys_tests::{H, focused, key, keys_harness, press, widest};
use crate::ui::menu_bar::snapshot_tests::with_bar;
use crate::ui::snapshot_tests::remove_fixture;
use eframe::egui::{self, Event, Key, Modifiers, Order};
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::ThemeChoice;

const SIZE: egui::Vec2 = egui::vec2(1400.0, 820.0);

/// The layer of the focused widget.
fn focus_layer(h: &H) -> Option<egui::LayerId> {
    let f = h.ctx.memory(|m| m.focused())?;
    h.ctx.read_response(f).map(|r| r.layer_id)
}

fn view_open(h: &H) -> bool {
    h.query_by_label("Left panel").is_some()
}

/// View open from the keyboard (Alt+F, then Right twice), Theme focused.
fn on_theme(name: &str) -> (H, std::path::PathBuf) {
    let (st, dir) = with_bar(ThemeChoice::Dark, name);
    let mut h = keys_harness(st, SIZE);
    press(&mut h, Modifiers::ALT, Key::F);
    key(&mut h, Key::ArrowRight);
    key(&mut h, Key::ArrowRight);
    assert!(view_open(&h), "View is open");
    for _ in 0..30 {
        if focused(&h, "Theme") {
            break;
        }
        key(&mut h, Key::ArrowDown);
    }
    assert!(focused(&h, "Theme"));
    (h, dir)
}

#[test]
fn right_on_a_submenu_row_opens_it_instead_of_the_next_menu() {
    let (mut h, dir) = on_theme("menu-keys-bar-theme");
    let view_layer = focus_layer(&h);
    key(&mut h, Key::ArrowRight);
    assert!(view_open(&h), "View stays open: Right opened the submenu, not Window");
    let sub = focus_layer(&h);
    assert!(sub.is_some_and(|l| l.order == Order::Foreground) && sub != view_layer, "the submenu's first row has the focus");
    key(&mut h, Key::ArrowLeft);
    assert!(focused(&h, "Theme") && view_open(&h), "Left closed the submenu alone");
    key(&mut h, Key::Enter);
    assert_ne!(focus_layer(&h), view_layer, "Enter opens it too");
    key(&mut h, Key::Escape);
    assert!(focused(&h, "Theme") && view_open(&h), "Esc closes one level");
    key(&mut h, Key::Escape);
    assert!(!view_open(&h), "and Esc again the menu");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn typeahead_works_in_the_bars_menus() {
    let (st, dir) = with_bar(ThemeChoice::Dark, "menu-keys-bar-type");
    let mut h = keys_harness(st, SIZE);
    press(&mut h, Modifiers::ALT, Key::F);
    h.input_mut().events.push(Event::Text("q".into()));
    h.run_ok();
    assert!(focused(&h, "Quit"), "q jumps to Quit");
    h.input_mut().events.push(Event::Text("N".into()));
    h.run_ok();
    assert!(widest(&h, "New tab").is_focused(), "N, case-folded and wrapping, to New tab");
    assert!(h.state().1.is_empty(), "the letters ran nothing");
    drop(h);
    remove_fixture(&dir);
}
