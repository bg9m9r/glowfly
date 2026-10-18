//! Accessible names for widgets egui can't name itself: text fields that only show hint text,
//! and buttons drawn with the painter. Screen readers read these names, and the whole-app tests
//! (`src/journey/`) find widgets by them. Nothing here changes a pixel.

use eframe::egui::{Response, WidgetInfo, WidgetType};

/// Names the text field `resp` (call it after the field is drawn, so its own node is filled).
pub fn name_field(resp: &Response, name: &str) {
    resp.ctx.accesskit_node_builder(resp.id, |n| n.set_label(name));
}

/// Names the painted button `resp`, and says whether it can be clicked.
pub fn name_button(resp: &Response, enabled: bool, name: &str) {
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, name));
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    #[test]
    fn fields_and_painted_buttons_get_their_names() {
        let mut text = String::new();
        let mut h = Harness::new_ui(|ui| {
            let r = ui.add(egui::TextEdit::singleline(&mut text).hint_text("Type here"));
            name_field(&r, "Search text");
            let (_, b) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::hover());
            name_button(&b, false, "Painted");
        });
        h.run();
        let field = h.get_by_label("Search text");
        assert_eq!(field.role(), egui::accesskit::Role::TextInput);
        assert!(h.get_by_label("Painted").is_disabled());
    }
}
