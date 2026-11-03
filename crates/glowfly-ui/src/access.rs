//! Accessible names for widgets egui can't name itself: text fields that only show hint text,
//! and buttons drawn with the painter, plus a longer description (why a menu item is disabled),
//! and live regions that screen readers announce as they appear or change (`announce`: the
//! toasts and the status bar's job). Screen readers read these names, and the whole-app tests
//! (`src/journey/`) find widgets by them. Nothing here changes a pixel.

pub use eframe::egui::accesskit::Live;
use eframe::egui::{Id, Response, WidgetInfo, WidgetType};

/// Names the text field `resp` (call it after the field is drawn, so its own node is filled).
pub fn name_field(resp: &Response, name: &str) {
    resp.ctx.accesskit_node_builder(resp.id, |n| n.set_label(name));
}

/// Names the painted button `resp`, and says whether it can be clicked.
pub fn name_button(resp: &Response, enabled: bool, name: &str) {
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, name));
}

/// Makes the label `resp` a live region: screen readers announce its text when it appears and
/// whenever it changes, after what they are reading (`Live::Polite`) or at once
/// (`Live::Assertive`, for failures). Its node moves to `key`: egui's own id for it shifts when
/// widgets drawn before it come and go, and a node under a new id is announced again. The node
/// left at `resp.id` says nothing. Does nothing while AccessKit is off.
pub fn announce(resp: &Response, key: Id, live: Live) {
    let Some(node) = resp.ctx.accesskit_node_builder(resp.id, std::mem::take) else { return };
    resp.ctx.accesskit_node_builder(key, |n| {
        *n = node;
        n.set_live(live);
    });
}

/// Gives `resp` a description that screen readers read after its name (does nothing while
/// AccessKit is off).
pub fn describe(resp: &Response, text: &str) {
    resp.ctx.accesskit_node_builder(resp.id, |n| n.set_description(text));
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui;
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
            announce(&ui.label("Done"), Id::new("done"), Live::Polite);
        });
        h.run();
        let field = h.get_by_label("Search text");
        assert_eq!(field.role(), egui::accesskit::Role::TextInput);
        let painted = h.get_by_label("Painted");
        assert!(painted.is_disabled());
        assert_eq!(painted.description().as_deref(), Some("Why"));
        let done = h.get_by_label("Done");
        assert_eq!((done.live(), done.role()), (Live::Polite, egui::accesskit::Role::Label));
        assert_eq!(done.id(), egui::accesskit::NodeId(Id::new("done").value()), "under its key");
        assert!(done.bounding_box().is_some(), "where it is drawn");
    }
}
