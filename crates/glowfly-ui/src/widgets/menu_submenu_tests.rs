//! Submenu hover intent and placement: a context menu in a small harness stepped 20 ms at a
//! time, so egui's input time says when a submenu opens and closes.

use super::*;
use crate::ui::widgets::menu;
use eframe::egui::{Event, Modifiers, PointerButton, Rect, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

const STEP: f32 = 0.02;
const SCREEN: Vec2 = vec2(600.0, 400.0);

/// A harness whose whole window opens a menu on a right-click: Alpha, More ▸ (One, Two, Three),
/// Other ▸ (Four), Plain.
fn harness() -> Harness<'static> {
    let mut h = crate::ui::snapshot_check::builder().with_size(SCREEN).with_step_dt(STEP).with_max_steps(200).build_ui(|ui| {
        let r = ui.allocate_rect(ui.max_rect(), Sense::click());
        menu::context_menu(&r, |ui| {
            let _ = ui.button("Alpha");
            menu::submenu(ui, "More", |ui| {
                for item in ["One", "Two", "Three"] {
                    let _ = ui.button(item);
                }
            });
            menu::submenu(ui, "Other", |ui| {
                let _ = ui.button("Four");
            });
            let _ = ui.button("Plain");
        });
    });
    h.run();
    h
}

/// Presses and releases `button` at `at`, a frame each.
fn press(h: &mut Harness<'static>, at: Pos2, button: PointerButton) {
    for pressed in [true, false] {
        let events = &mut h.input_mut().events;
        events.push(Event::PointerMoved(at));
        events.push(Event::PointerButton { pos: at, button, pressed, modifiers: Modifiers::NONE });
        h.step();
    }
}

/// Right-clicks at `at`, opening the menu there (egui keeps it inside the window).
fn open_menu(h: &mut Harness<'static>, at: Pos2) {
    press(h, at, PointerButton::Secondary);
    h.run();
}

/// Clicks the row `label` with the pointer (an accessibility click doesn't move it there).
fn click(h: &mut Harness<'static>, label: &str) {
    let at = rect_of(h, label).center();
    press(h, at, PointerButton::Primary);
}

fn now(h: &Harness<'static>) -> f64 {
    h.ctx.input(|i| i.time)
}

/// Whether `label` shows in a submenu: drawn on a layer of its own (not when only laid out
/// unseen, to be measured, nor in egui's invisible first frame).
fn shows(h: &Harness<'static>, label: &str) -> bool {
    let Some(item) = h.query_by_label(label).and_then(|n| n.bounding_box()) else {
        return false;
    };
    let at = pos2(((item.x0 + item.x1) / 2.0) as f32, ((item.y0 + item.y1) / 2.0) as f32);
    let here = h.ctx.layer_id_at(at);
    here.is_some() && here != h.ctx.layer_id_at(rect_of(h, "More").center())
}

fn rect_of(h: &Harness<'static>, label: &str) -> Rect {
    let b = h.get_by_label(label).bounding_box().expect(label);
    Rect::from_min_max(pos2(b.x0 as f32, b.y0 as f32), pos2(b.x1 as f32, b.y1 as f32))
}

/// The rect of the menu (an egui area) that draws `label`.
fn menu_of(h: &Harness<'static>, label: &str) -> Rect {
    let layer = h.ctx.layer_id_at(rect_of(h, label).center()).expect("a menu under the item");
    h.ctx.memory(|m| m.area_rect(layer.id)).expect("the menu's area")
}

/// Steps until `label` shows, for at most `secs`; the time it took.
fn until_shown(h: &mut Harness<'static>, label: &str, secs: f64) -> f64 {
    let start = now(h);
    while !shows(h, label) {
        assert!(now(h) - start <= secs, "{label} didn't show within {secs} s");
        h.step();
    }
    now(h) - start
}

/// Steps for `secs`, checking `ok` after each step.
fn for_secs(h: &mut Harness<'static>, secs: f64, ok: impl Fn(&Harness<'static>) -> bool, what: &str) {
    let start = now(h);
    while now(h) - start < secs - 1e-6 {
        h.step();
        assert!(ok(h), "{what} at {:.2} s", now(h) - start);
    }
}

#[test]
fn hover_opens_after_resting_on_the_row() {
    let mut h = harness();
    open_menu(&mut h, pos2(100.0, 100.0));
    h.get_by_label("More").hover();
    h.step();
    for_secs(&mut h, OPEN_DELAY - 0.03, |h| !shows(h, "One"), "the submenu stays shut during the delay");
    let took = until_shown(&mut h, "One", 0.2);
    assert!(took >= 0.02, "not before the delay is over");
    for_secs(&mut h, 1.0, |h| shows(h, "One"), "the submenu stays open while the pointer rests");
}

#[test]
fn passing_over_a_row_doesnt_open_it() {
    let mut h = harness();
    open_menu(&mut h, pos2(100.0, 100.0));
    h.get_by_label("More").hover();
    for_secs(&mut h, 0.1, |h| !shows(h, "One"), "a brief pass opens nothing");
    h.get_by_label("Plain").hover();
    for_secs(&mut h, 1.0, |h| !shows(h, "One"), "leaving before the delay");
}

#[test]
fn a_click_opens_at_once() {
    let mut h = harness();
    open_menu(&mut h, pos2(100.0, 100.0));
    let start = now(&h);
    click(&mut h, "More");
    until_shown(&mut h, "One", 1.0);
    assert!(now(&h) - start < OPEN_DELAY, "opened {:.2} s after the click", now(&h) - start);
}

#[test]
fn enter_space_and_right_open_at_once() {
    for key in [Key::Enter, Key::Space, Key::ArrowRight] {
        let mut h = harness();
        open_menu(&mut h, pos2(100.0, 100.0));
        h.get_by_label("More").hover();
        h.step();
        let start = now(&h);
        h.press_key(key);
        until_shown(&mut h, "One", 1.0);
        assert!(now(&h) - start < OPEN_DELAY - 0.1, "{key:?} opened it {:.2} s later", now(&h) - start);
    }
}

#[test]
fn leaving_keeps_it_open_for_the_grace() {
    let mut h = harness();
    open_menu(&mut h, pos2(100.0, 100.0));
    click(&mut h, "More");
    until_shown(&mut h, "One", 1.0);
    h.get_by_label("Plain").hover();
    h.step();
    for_secs(&mut h, CLOSE_GRACE - 0.03, |h| shows(h, "One"), "open during the grace");
    for_secs(&mut h, 0.1, |_| true, "");
    assert!(!shows(&h, "One"), "closed after the grace");
}

#[test]
fn the_submenu_itself_keeps_it_open() {
    let mut h = harness();
    open_menu(&mut h, pos2(100.0, 100.0));
    click(&mut h, "More");
    until_shown(&mut h, "One", 1.0);
    h.get_by_label("Two").hover();
    for_secs(&mut h, 1.0, |h| shows(h, "One"), "open while the pointer is on it");
}

#[test]
fn resting_on_another_submenu_row_switches() {
    let mut h = harness();
    open_menu(&mut h, pos2(100.0, 100.0));
    click(&mut h, "More");
    until_shown(&mut h, "One", 1.0);
    h.get_by_label("Other").hover();
    h.step();
    for_secs(&mut h, OPEN_DELAY - 0.03, |h| shows(h, "One") && !shows(h, "Four"), "More's stays during Other's delay");
    until_shown(&mut h, "Four", 0.2);
    h.step();
    assert!(!shows(&h, "One"), "Other's replaced More's");
}

#[test]
fn it_opens_right_of_the_menu_level_with_its_row() {
    let mut h = harness();
    open_menu(&mut h, pos2(100.0, 100.0));
    click(&mut h, "More");
    until_shown(&mut h, "One", 1.0);
    h.run();
    let (parent, sub) = (menu_of(&h, "More"), menu_of(&h, "One"));
    assert!(sub.left() >= parent.right(), "right of the menu: {sub:?} beside {parent:?}");
    assert!((rect_of(&h, "One").top() - rect_of(&h, "More").top()).abs() < 0.5, "its first row level with More");
}

#[test]
fn near_the_right_and_bottom_edges_it_flips_left_and_shifts_up() {
    let mut h = harness();
    open_menu(&mut h, pos2(SCREEN.x - 16.0, SCREEN.y - 16.0));
    click(&mut h, "More");
    until_shown(&mut h, "One", 1.0);
    h.run();
    let (parent, sub) = (menu_of(&h, "More"), menu_of(&h, "One"));
    assert!(sub.right() <= parent.left(), "left of the menu: {sub:?} beside {parent:?}");
    assert!(sub.left() >= EDGE);
    assert!((sub.bottom() - (SCREEN.y - EDGE)).abs() < 0.5, "8 px above the bottom edge, no higher: {sub:?}");
    assert!(rect_of(&h, "One").top() < rect_of(&h, "More").top(), "shifted up");
}

#[test]
fn placement_prefers_the_right_then_the_left() {
    let screen = Rect::from_min_size(Pos2::ZERO, SCREEN);
    let size = vec2(150.0, 100.0);
    let menu = Rect::from_min_size(pos2(100.0, 50.0), vec2(200.0, 150.0));
    assert_eq!(placement(menu, 60.0, size, screen, 2.0), pos2(302.0, 60.0), "fits on the right");
    let menu = Rect::from_min_size(pos2(300.0, 50.0), vec2(200.0, 150.0));
    assert_eq!(placement(menu, 60.0, size, screen, 2.0), pos2(148.0, 60.0), "flipped left");
    let wide = vec2(320.0, 100.0);
    let menu = Rect::from_min_size(pos2(160.0, 50.0), vec2(300.0, 150.0));
    assert_eq!(placement(menu, 60.0, wide, screen, 2.0), pos2(EDGE, 60.0), "neither: the roomier left, kept 8 px in");
    assert_eq!(placement(menu, 350.0, size, screen, 2.0).y, SCREEN.y - EDGE - 100.0, "shifted up only as needed");
    assert_eq!(placement(menu, 2.0, size, screen, 2.0).y, EDGE, "and never above the top edge's 8 px");
}
