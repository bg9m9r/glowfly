//! The keyboard shortcut sheet (Ctrl+/): a filter field over every binding in the table,
//! grouped, each row with its keys drawn as key caps. A reference only; rows don't run
//! anything.

use super::Caps;
use crate::keymap::{Binding, Keymap};
use crate::ui::theme::color;
use eframe::egui::{Align, Id, Layout, RichText, ScrollArea, Sense, TextEdit, Ui, vec2};

const WIDTH: f32 = 540.0;
const LIST_MAX_H: f32 = 460.0;

/// Draws the sheet inside the dialog with the keys in effect; `focus` gives the filter field the
/// keyboard (not under a credential prompt). Returns true when Close is clicked.
pub fn show(ui: &mut Ui, filter: &mut String, focus: bool, keys: &Keymap) -> bool {
    ui.set_width(WIDTH);
    ui.heading("Keyboard shortcuts");
    ui.add_space(6.0);
    let field = TextEdit::singleline(filter)
        .id(Id::new("trellis-shortcut-filter"))
        .hint_text("Filter by action or key")
        .desired_width(f32::INFINITY);
    let r = ui.add(field);
    if focus {
        r.request_focus();
    }
    ui.add_space(4.0);
    let groups = keys.filter(filter);
    ScrollArea::vertical().id_salt("trellis-shortcut-list").max_height(LIST_MAX_H).show(ui, |ui| {
        ui.set_width(ui.available_width());
        if groups.is_empty() {
            ui.add_space(8.0);
            ui.label(RichText::new("No shortcuts match").color(color::weak()));
        }
        for (group, rows) in &groups {
            ui.add_space(8.0);
            ui.label(RichText::new(group.title()).strong().size(12.5).color(color::weak()));
            for b in rows {
                row(ui, b, keys);
            }
        }
    });
    ui.add_space(8.0);
    let mut close = false;
    ui.horizontal(|ui| {
        ui.label(RichText::new("Esc to close").color(color::weak()));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            close = ui.button("Close").clicked();
        });
    });
    close
}

/// One binding: the action and its context note on the left (wrapping when long), the key caps
/// right-aligned.
fn row(ui: &mut Ui, b: &Binding, keys: &Keymap) {
    let caps = Caps::new(ui, keys.chords(b.id), false);
    ui.horizontal(|ui| {
        let left = (ui.available_width() - caps.size.x - 16.0).max(120.0);
        let wrap = Layout::left_to_right(Align::Center).with_main_wrap(true);
        ui.allocate_ui_with_layout(vec2(left, 0.0), wrap, |ui| {
            ui.label(RichText::new(b.label).color(color::text()));
            if let Some(note) = b.when.note() {
                ui.label(RichText::new(note).size(11.5).color(color::weak()));
            }
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| caps.paint(ui, &keys.keys_text(b.id), Sense::hover()));
    });
}
