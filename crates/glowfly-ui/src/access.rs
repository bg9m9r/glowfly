//! Accessible names for widgets egui can't name itself: text fields that only show hint text,
//! and buttons drawn with the painter, plus a longer description (why a menu item is disabled). Screen readers read these names, and the whole-app tests
//! (`src/journey/`) find widgets by them. A form row's label names the fields and combos drawn
//! in it that have no name of their own (`with_label`, `from_label`). Nothing here changes a
//! pixel.

use eframe::egui::{Context, Id, Response, WidgetInfo, WidgetType};

/// Where `with_label` keeps the label naming the controls being drawn.
const LABEL_KEY: &str = "glowfly-access-label";

/// Names the text field `resp` (call it after the field is drawn, so its own node is filled).
pub fn name_field(resp: &Response, name: &str) {
    resp.ctx.accesskit_node_builder(resp.id, |n| n.set_label(name));
}

/// Names the painted button `resp`, and says whether it can be clicked.
pub fn name_button(resp: &Response, enabled: bool, name: &str) {
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, name));
}

/// Gives `resp` a description that screen readers read after its name (does nothing while
/// AccessKit is off).
pub fn describe(resp: &Response, text: &str) {
    resp.ctx.accesskit_node_builder(resp.id, |n| n.set_description(text));
}

/// Runs `f` with the node `label` (None: no label) naming the controls it draws that have no
/// name of their own, as `from_label` finds them.
pub fn with_label<R>(ctx: &Context, label: Option<Id>, f: impl FnOnce() -> R) -> R {
    let key = Id::new(LABEL_KEY);
    let outer = ctx.data(|d| d.get_temp::<Option<Id>>(key)).flatten();
    ctx.data_mut(|d| d.insert_temp(key, label));
    let out = f();
    ctx.data_mut(|d| d.insert_temp(key, outer));
    out
}

/// Names the control `resp` by the label `with_label` set, unless it has a name of its own.
pub fn from_label(resp: &Response) {
    let label = resp.ctx.data(|d| d.get_temp::<Option<Id>>(Id::new(LABEL_KEY))).flatten();
    if let Some(label) = label.filter(|_| unnamed(resp)) {
        // An empty name would hide the labelling node from screen readers.
        resp.ctx.accesskit_node_builder(resp.id, |n| n.clear_label());
        resp.clone().labelled_by(label);
    }
}

/// Names `resp` `name` unless it has a name of its own (a label or a labelling node).
pub fn name_if_unnamed(resp: &Response, name: &str) {
    if !name.is_empty() && unnamed(resp) {
        name_field(resp, name);
    }
}

/// Whether `resp`'s node has no name (always false while AccessKit is off).
fn unnamed(resp: &Response) -> bool {
    resp.ctx.accesskit_node_builder(resp.id, |n| n.label().is_none_or(str::is_empty) && n.labelled_by().is_empty()).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui;
    use eframe::egui::accesskit::Role;
    use egui_kittest::kittest::Queryable;

    #[test]
    fn fields_and_painted_buttons_get_their_names() {
        let mut text = String::new();
        let mut h = crate::ui::snapshot_check::builder().build_ui(|ui| {
            let r = ui.add(egui::TextEdit::singleline(&mut text).hint_text("Type here"));
            name_field(&r, "Search text");
            let (_, b) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::hover());
            name_button(&b, false, "Painted");
            describe(&b, "Why");
        });
        h.run();
        let field = h.get_by_label("Search text");
        assert_eq!(field.role(), egui::accesskit::Role::TextInput);
        let painted = h.get_by_label("Painted");
        assert!(painted.is_disabled());
        assert_eq!(painted.description().as_deref(), Some("Why"));
    }

    #[test]
    fn a_label_names_only_the_unnamed_controls_drawn_under_it() {
        let (mut a, mut b, mut on) = (String::new(), String::new(), false);
        let mut h = crate::ui::snapshot_check::builder().build_ui(|ui| {
            let label = ui.label("Editor").id;
            with_label(&ui.ctx().clone(), Some(label), || {
                crate::ui::theme::controls::field(ui, egui::TextEdit::singleline(&mut a));
                let named = crate::ui::theme::controls::field(ui, egui::TextEdit::singleline(&mut b));
                name_field(&named, "Arguments");
                ui.add(crate::ui::theme::controls::Check::without_text(&mut on));
            });
            // Outside: nothing names it.
            let mut c = String::new();
            let after = crate::ui::theme::controls::field(ui, egui::TextEdit::singleline(&mut c));
            name_if_unnamed(&after, "Hint");
            name_if_unnamed(&after, "Second");
        });
        h.run();
        let roles: Vec<_> = h.query_all_by_label("Editor").map(|n| n.role()).collect();
        assert_eq!(roles, [Role::TextInput, Role::CheckBox], "the bare field and the bare box, not the named field or the one after");
        assert!(h.query_by_label("Arguments").is_some(), "a named field keeps its name");
        assert!(h.query_by_label("Hint").is_some() && h.query_by_label("Second").is_none(), "the first name stays");
    }

    #[test]
    fn a_section_header_is_a_button_that_says_whether_it_is_open() {
        let seen = std::cell::Cell::new(None);
        let mut h = crate::ui::snapshot_check::builder().build_ui(|ui| {
            let mut open = true;
            let r = crate::ui::widgets::section_header(ui, "Local", Some(3), &mut open);
            seen.set(ui.ctx().accesskit_node_builder(r.id, |n| n.is_expanded()).flatten());
        });
        h.run();
        assert_eq!(h.get_by_label("Local, 3").role(), Role::Button);
        assert_eq!(seen.get(), Some(true));
    }
}
