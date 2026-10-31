//! The menu keyboard in small harnesses, one per menu family, all with the same rows: a group
//! header, Alpha, Beta (blocked), a separator, Cherry with its preview button, More ▸ (One,
//! Two) and Delta. A context menu opened with Shift+F10 on a focused button, a dropdown button
//! (`menu::custom_button`) and a popup (`menu::popup_below`).

use crate::ui::widgets::{Icon, menu};
use eframe::egui::{self, Event, Id, Key, Modifiers, PointerButton, PopupCloseBehavior, Pos2, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

type H = Harness<'static, Vec<String>>;

/// The rows every harness's menu shows; what was clicked goes in `log`.
fn rows(ui: &mut egui::Ui, log: &mut Vec<String>) {
    let mut done = |ui: &mut egui::Ui, what: &str| {
        log.push(what.to_owned());
        ui.close_menu();
    };
    menu::group_header(ui, "Group");
    if menu::item("Alpha").show(ui).clicked() {
        done(ui, "Alpha");
    }
    let _ = menu::item("Beta").blocked(Some("Not now")).show(ui);
    menu::separator(ui);
    match menu::item("Cherry").trailing_button(Icon::Eye, "Preview cherry").show_split(ui).clicked() {
        Some(menu::Part::Row) => done(ui, "Cherry"),
        Some(menu::Part::Button) => done(ui, "Preview"),
        None => {}
    }
    menu::submenu(ui, "More", |ui| {
        for n in ["One", "Two"] {
            if menu::item(n).show(ui).clicked() {
                done(ui, n);
            }
        }
    });
    if menu::item("Delta").show(ui).clicked() {
        done(ui, "Delta");
    }
}

fn build(app: impl FnMut(&mut egui::Ui, &mut Vec<String>) + 'static) -> H {
    let mut h = crate::ui::snapshot_check::builder()
        .with_size(vec2(640.0, 420.0))
        .with_step_dt(0.02)
        .with_max_steps(200)
        .build_ui_state(app, Vec::new());
    h.run();
    h
}

/// A button whose context menu shows the rows.
fn context() -> H {
    build(|ui, log| {
        let target = ui.button("Target");
        let _ = ui.button("Behind");
        menu::context_menu(&target, |ui| rows(ui, log));
    })
}

/// A dropdown button.
fn dropdown() -> H {
    build(|ui, log| {
        menu::custom_button(ui, egui::Button::new("Open menu"), |ui| rows(ui, log));
    })
}

/// A button toggling a popup that a click closes.
fn popup() -> H {
    build(|ui, log| {
        let r = ui.button("Caret");
        let id = Id::new("test-popup");
        if r.clicked() {
            ui.memory_mut(|m| m.toggle_popup(id));
        }
        menu::popup_below(ui, id, &r, PopupCloseBehavior::CloseOnClick, |ui| rows(ui, log));
    })
}

fn key(h: &mut H, k: Key) {
    h.press_key(k);
    h.run();
}

fn keys(h: &mut H, mods: Modifiers, k: Key) {
    h.press_key_modifiers(mods, k);
    h.run();
}

fn typed(h: &mut H, text: &str) {
    h.input_mut().events.push(Event::Text(text.into()));
    h.run();
}

fn focused(h: &H, label: &str) -> bool {
    h.query_all_by_label(label).any(|n| n.is_focused())
}

fn open(h: &H) -> bool {
    h.query_by_label("Alpha").is_some()
}

fn shows(h: &H, label: &str) -> bool {
    h.query_by_label(label).is_some()
}

fn centre(h: &H, label: &str) -> Pos2 {
    let b = h.get_by_label(label).bounding_box().expect(label);
    pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32)
}

/// A real pointer press and release of `button` on `label`, a frame each.
fn pointer(h: &mut H, label: &str, button: PointerButton) {
    let at = centre(h, label);
    for pressed in [true, false] {
        h.input_mut().events.push(Event::PointerMoved(at));
        h.input_mut().events.push(Event::PointerButton { pos: at, button, pressed, modifiers: Modifiers::NONE });
        h.step();
    }
    h.run();
}

/// The context menu open from the keyboard on its focused button.
fn context_open() -> H {
    let mut h = context();
    h.get_by_label("Target").focus();
    h.run();
    keys(&mut h, Modifiers::SHIFT, Key::F10);
    h
}

#[test]
fn shift_f10_opens_a_context_menu_below_its_widget_with_the_first_row_focused() {
    let h = context_open();
    assert!(open(&h), "Shift+F10 opened it");
    assert!(focused(&h, "Alpha"), "its first row has the focus (the header can't take it)");
    let target = h.get_by_label("Target").bounding_box().unwrap();
    assert!(h.get_by_label("Alpha").bounding_box().unwrap().y0 >= target.y1, "below the widget");
}

#[test]
fn up_and_down_wrap_skip_headers_and_separators_and_stop_on_blocked_rows() {
    let mut h = context_open();
    for label in ["Beta", "Cherry", "More", "Delta", "Alpha"] {
        key(&mut h, Key::ArrowDown);
        assert!(focused(&h, label), "Down to {label}");
    }
    key(&mut h, Key::ArrowUp);
    assert!(focused(&h, "Delta"), "Up from the first wraps to the last");
    key(&mut h, Key::Home);
    assert!(focused(&h, "Alpha"));
    key(&mut h, Key::End);
    assert!(focused(&h, "Delta"));
    assert!(!focused(&h, "Behind"), "the arrows never reached the button behind");
}

#[test]
fn typeahead_jumps_to_the_next_label_starting_with_the_letter() {
    let mut h = context_open();
    typed(&mut h, "d");
    assert!(focused(&h, "Delta"));
    typed(&mut h, "C");
    assert!(focused(&h, "Cherry"), "case-folded");
    typed(&mut h, "b");
    assert!(focused(&h, "Beta"), "a blocked row takes the focus too");
    typed(&mut h, "a");
    assert!(focused(&h, "Alpha"));
}

#[test]
fn right_and_left_move_between_a_split_row_and_its_button() {
    let mut h = context_open();
    typed(&mut h, "c");
    key(&mut h, Key::ArrowRight);
    assert!(focused(&h, "Preview cherry"), "Right reaches the preview button");
    key(&mut h, Key::ArrowLeft);
    assert!(focused(&h, "Cherry"), "Left goes back to the row");
    key(&mut h, Key::ArrowRight);
    key(&mut h, Key::Enter);
    assert_eq!(h.state(), &["Preview"], "Enter on the button runs the preview");
    assert!(!open(&h));
}

#[test]
fn right_opens_a_submenu_and_left_and_esc_close_one_level() {
    let mut h = context_open();
    typed(&mut h, "m");
    key(&mut h, Key::ArrowRight);
    assert!(shows(&h, "One"), "Right opened More");
    assert!(focused(&h, "One"), "with its first row focused");
    key(&mut h, Key::ArrowDown);
    assert!(focused(&h, "Two"));
    key(&mut h, Key::ArrowDown);
    assert!(focused(&h, "One"), "the submenu wraps too");
    key(&mut h, Key::ArrowLeft);
    assert!(!shows(&h, "One"), "Left closed it");
    assert!(focused(&h, "More") && open(&h), "back on its row, the menu still open");
    key(&mut h, Key::Enter);
    assert!(focused(&h, "One"), "Enter opens it too");
    key(&mut h, Key::Escape);
    assert!(!shows(&h, "One") && open(&h), "Esc closes one level");
    assert!(focused(&h, "More"));
    key(&mut h, Key::Space);
    assert!(focused(&h, "One"), "and Space");
    key(&mut h, Key::ArrowDown);
    key(&mut h, Key::Enter);
    assert_eq!(h.state(), &["Two"], "Enter runs the submenu's item");
    assert!(!open(&h) && !shows(&h, "Two"), "and closes the whole menu");
}

#[test]
fn a_click_in_a_keyboard_opened_submenu_is_not_a_click_outside() {
    let mut h = context_open();
    typed(&mut h, "m");
    key(&mut h, Key::ArrowRight);
    assert!(shows(&h, "Two"));
    pointer(&mut h, "Two", PointerButton::Primary);
    assert_eq!(h.state(), &["Two"], "the click ran the item");
    assert!(!open(&h));
}

#[test]
fn esc_closes_the_root_menu_and_gives_the_focus_back() {
    let mut h = context_open();
    key(&mut h, Key::ArrowDown);
    key(&mut h, Key::Escape);
    assert!(!open(&h), "closed");
    assert!(focused(&h, "Target"), "the focus is back on the widget it opened from");
}

#[test]
fn down_or_up_in_a_menu_opened_with_the_mouse_focuses_the_first_or_last_row() {
    for (k, want) in [(Key::ArrowDown, "Alpha"), (Key::ArrowUp, "Delta")] {
        let mut h = context();
        pointer(&mut h, "Target", PointerButton::Secondary);
        assert!(open(&h) && !focused(&h, "Alpha"), "a right-click opens it with nothing focused");
        key(&mut h, k);
        assert!(focused(&h, want), "{k:?} focused {want}");
    }
}

#[test]
fn esc_in_a_pointer_opened_submenu_closes_only_it() {
    let mut h = context();
    pointer(&mut h, "Target", PointerButton::Secondary);
    let at = centre(&h, "More");
    for _ in 0..40 {
        h.input_mut().events.push(Event::PointerMoved(at));
        h.step();
        if shows(&h, "One") {
            break;
        }
    }
    h.run();
    assert!(shows(&h, "One"), "resting on More opened it");
    key(&mut h, Key::Escape);
    assert!(!shows(&h, "One"), "Esc closed the submenu");
    assert!(open(&h), "and left the menu open");
    key(&mut h, Key::Escape);
    assert!(!open(&h));
}

#[test]
fn dropdown_buttons_open_on_enter_space_and_alt_down() {
    for (mods, k) in [(Modifiers::NONE, Key::Enter), (Modifiers::NONE, Key::Space), (Modifiers::ALT, Key::ArrowDown)] {
        let mut h = dropdown();
        h.get_by_label("Open menu").focus();
        h.run();
        keys(&mut h, mods, k);
        assert!(open(&h), "{k:?} opened it");
        assert!(focused(&h, "Alpha"), "{k:?}: the first row focused");
        key(&mut h, Key::Escape);
        assert!(!open(&h));
        assert!(focused(&h, "Open menu"), "Esc gives the button the focus back");
    }
}

#[test]
fn popup_menus_take_the_keyboard_too() {
    let mut h = popup();
    h.get_by_label("Caret").focus();
    h.run();
    keys(&mut h, Modifiers::ALT, Key::ArrowDown);
    assert!(open(&h) && focused(&h, "Alpha"), "Alt+Down opened it, its first row focused");
    key(&mut h, Key::Escape);
    assert!(!open(&h) && focused(&h, "Caret"), "Esc closed it and gave the caret the focus");
    key(&mut h, Key::Enter);
    assert!(open(&h) && focused(&h, "Alpha"), "Enter on the caret opens it");
    typed(&mut h, "m");
    key(&mut h, Key::ArrowRight);
    assert!(focused(&h, "One"), "Right opened More with its first row focused");
    key(&mut h, Key::Escape);
    assert!(!shows(&h, "One") && open(&h), "Esc closed More only");
    assert!(focused(&h, "More"));
    typed(&mut h, "d");
    key(&mut h, Key::Enter);
    assert_eq!(h.state(), &["Delta"]);
    assert!(!open(&h), "Enter on an item closes a popup that a click would");
}
