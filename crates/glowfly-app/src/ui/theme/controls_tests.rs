//! The themed form controls behave like egui's own: clicks, keys, changed() and what screen
//! readers (and the journey tests) see.

use super::controls::{self, Check, Field, Radio};
use crate::ui::access;
use eframe::egui::{self, Key, Stroke, accesskit::Role, accesskit::Toggled};
use egui_kittest::kittest::Queryable;

#[derive(Default)]
struct Form {
    on: bool,
    changed: bool,
    choice: u8,
    choice_changes: u32,
}

#[test]
fn a_check_toggles_on_click_and_reports_its_state() {
    let mut h = crate::ui::snapshot_check::builder().build_ui_state(
        |ui, f: &mut Form| {
            f.changed |= controls::checkbox(ui, &mut f.on, "Sign commits").changed();
        },
        Form::default(),
    );
    h.run();
    assert_eq!(h.get_by_label("Sign commits").toggled(), Some(Toggled::False));
    assert_eq!(h.get_by_label("Sign commits").role(), Role::CheckBox);
    h.get_by_label("Sign commits").click();
    h.run();
    assert!(h.state().on && h.state().changed);
    assert_eq!(h.get_by_label("Sign commits").toggled(), Some(Toggled::True));
}

#[test]
fn a_disabled_check_ignores_clicks() {
    let mut h = crate::ui::snapshot_check::builder().build_ui_state(
        |ui, f: &mut Form| {
            ui.add_enabled(false, Check::new(&mut f.on, "Locked"));
        },
        Form::default(),
    );
    h.run();
    assert!(h.get_by_label("Locked").is_disabled());
    h.get_by_label("Locked").click();
    h.run();
    assert!(!h.state().on);
}

#[test]
fn a_check_without_text_and_the_keyboard() {
    let mut h = crate::ui::snapshot_check::builder().build_ui_state(
        |ui, f: &mut Form| {
            ui.add(Check::without_text(&mut f.on));
        },
        Form::default(),
    );
    h.run();
    let node = h.get_by_role(Role::CheckBox);
    assert_eq!(node.toggled(), Some(Toggled::False));
    node.focus();
    h.run();
    h.press_key(Key::Space);
    h.run();
    assert!(h.state().on, "Space toggles the focused box");
    assert_eq!(h.get_by_role(Role::CheckBox).toggled(), Some(Toggled::True));
}

#[test]
fn radio_value_changes_only_on_a_different_option() {
    let mut h = crate::ui::snapshot_check::builder().build_ui_state(
        |ui, f: &mut Form| {
            for (v, name) in [(0, "Merge"), (1, "Rebase")] {
                if controls::radio_value(ui, &mut f.choice, v, name).changed() {
                    f.choice_changes += 1;
                }
            }
            ui.add(Radio::new(f.choice == 1, "Shown"));
        },
        Form::default(),
    );
    h.run();
    h.get_by_label("Merge").click();
    h.run();
    assert_eq!((h.state().choice, h.state().choice_changes), (0, 0), "already selected");
    h.get_by_label("Rebase").click();
    h.run();
    assert_eq!((h.state().choice, h.state().choice_changes), (1, 1));
    let shown = h.get_by_label("Shown");
    assert_eq!((shown.role(), shown.toggled()), (Role::RadioButton, Some(Toggled::True)));
    assert_eq!(h.get_by_label("Merge").toggled(), Some(Toggled::False));
}

#[test]
fn a_field_keeps_its_role_and_name_and_restores_the_style() {
    let mut seen = None;
    let mut text = String::new();
    let mut h = crate::ui::snapshot_check::builder().build_ui(|ui| {
        super::apply_theme(ui.ctx());
        let r = ui.add(Field(egui::TextEdit::singleline(&mut text).hint_text("Filter")));
        access::name_field(&r, "Filter branches");
        seen = Some(ui.visuals().widgets.inactive.bg_stroke);
    });
    h.run();
    let field = h.get_by_label("Filter branches");
    assert_eq!(field.role(), Role::TextInput);
    drop(h);
    let ctx = egui::Context::default();
    super::apply_theme(&ctx);
    let app: Stroke = ctx.style().visuals.widgets.inactive.bg_stroke;
    assert_eq!(seen, Some(app));
    assert_ne!(app.color, super::color::input_border());
}
