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

#[test]
fn a_fields_placeholder_is_faint_in_every_builtin() {
    use eframe::egui::Shape;
    use glowfly_core::settings::BuiltinTheme;
    for t in BuiltinTheme::ALL {
        let p = *super::builtin(t);
        let mut text = String::new();
        let mut h = crate::ui::snapshot_check::builder().build_ui(|ui| {
            super::color::set_palette(&p);
            super::apply_style(ui.ctx(), &p);
            ui.ctx().set_theme(super::egui_theme(&p));
            ui.add(Field(egui::TextEdit::singleline(&mut text).hint_text("Filter")));
        });
        h.run();
        let hint = h.output().shapes.iter().find_map(|s| match &s.shape {
            Shape::Text(g) if g.galley.text() == "Filter" => Some(g.fallback_color),
            _ => None,
        });
        assert_eq!(hint, Some(p.faint), "{t:?}: the placeholder is faint");
    }
}

#[test]
fn sized_fields_take_the_height_width_and_padding_of_their_size() {
    use super::sizes::ControlSize;
    let mut seen = Vec::new();
    let (mut a, mut b, mut c, mut d) = (String::new(), String::new(), String::new(), String::new());
    let mut h = crate::ui::snapshot_check::builder().with_size(egui::vec2(400.0, 300.0)).build_ui(|ui| {
        super::apply_theme(ui.ctx());
        let regular = ui.add(controls::SizedField::new(ControlSize::Regular, egui::TextEdit::singleline(&mut a)).width(200.0));
        let large = ui.add(controls::SizedField::new(ControlSize::Large, egui::TextEdit::singleline(&mut b)));
        let multi = ui.add(controls::SizedField::new(ControlSize::Regular, egui::TextEdit::multiline(&mut c).desired_rows(4)).multiline());
        let room =
            ui.add(controls::SizedField::new(ControlSize::Regular, egui::TextEdit::singleline(&mut d)).width(200.0).trailing_room(26.0));
        seen = vec![regular.rect, large.rect, multi.rect, room.rect];
    });
    h.run();
    drop(h);
    let [regular, large, multi, room] = seen[..] else { panic!("{seen:?}") };
    assert_eq!((regular.height(), regular.width()), (28.0, 200.0), "inline: 28 px, the width given is the whole field's");
    assert_eq!(large.height(), 36.0, "dialog: 36 px");
    assert!(multi.height() > 4.0 * 14.0 + 2.0 * controls::field_pad_y(ControlSize::Regular, true), "{multi:?}");
    assert_eq!(room.width(), 200.0, "the clear button's room stays inside the given width");
    assert_eq!(controls::field_margin(ControlSize::Regular, false, Some(26.0)).left, 10);
    assert_eq!(controls::field_text_width(200.0), 180.0);
}

/// A disabled radio is drawn at full strength, so its marks meet 3:1 in every theme: the
/// unselected ring's edge on the panel, the selected one's `weak` fill on the panel, and its
/// `input` dot on that fill.
#[test]
fn disabled_radio_marks_stay_visible() {
    use super::contrast::ratio;
    let mut failures = Vec::new();
    for (t, p) in super::tests::palettes() {
        for (what, fg, bg) in [("ring", p.input_border, p.panel), ("selected fill", p.weak, p.panel), ("dot", p.input, p.weak)] {
            let r = ratio(fg, bg);
            if r < 3.0 {
                failures.push(format!("{t}: disabled radio {what} is {r:.2}:1"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A disabled radio's label is `weak`, even when the label brings its own colour (Preferences >
/// Git program's labels are `RichText` in `text`), so it never looks enabled.
#[test]
fn a_disabled_radio_label_is_weak_even_with_its_own_colour() {
    use super::color;
    use eframe::egui::{RichText, Shape};
    let mut h = crate::ui::snapshot_check::builder().build_ui(|ui| {
        super::apply_theme(ui.ctx());
        ui.add_enabled(false, Radio::new(false, "Plain"));
        ui.add_enabled(false, Radio::new(true, RichText::new("Coloured").color(egui::Color32::RED)));
        ui.add(Radio::new(false, "Enabled"));
    });
    h.run();
    let mut colours = std::collections::BTreeMap::new();
    for c in &h.output().shapes {
        if let Shape::Text(t) = &c.shape {
            colours.insert(t.galley.text().to_owned(), t.override_text_color);
        }
    }
    let weak = Some(color::weak());
    assert_eq!(colours.get("Plain"), Some(&weak), "{colours:?}");
    assert_eq!(colours.get("Coloured"), Some(&weak), "{colours:?}");
    assert_ne!(colours.get("Enabled"), Some(&weak), "an enabled label keeps its colour: {colours:?}");
}
