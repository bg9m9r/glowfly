//! The keyboard shortcut sheet (Ctrl+/): a filter field over every binding in the table,
//! grouped, each row with its keys drawn as key caps. A reference only; rows don't run
//! anything.

use super::Caps;
use crate::keymap::{Binding, Keymap};
use crate::ui::dialog_frame::{Cx, Dialog, Footer};
use crate::ui::theme::color;
use crate::ui::theme::controls;
use eframe::egui::{Align, Id, Layout, RichText, Sense, TextEdit, Ui, vec2};

/// Draws the sheet inside the dialog with the keys in effect; `cx.keyboard` gives the filter
/// field the keyboard (not under a credential prompt). Returns true when Close is clicked.
pub fn show(ui: &mut Ui, cx: Cx, filter: &mut String, keys: &Keymap) -> bool {
    let mut d = Dialog::new(cx, "Keyboard shortcuts").wide().resizable("shortcut-sheet").begin(ui);
    let field = TextEdit::singleline(filter)
        .id(Id::new("trellis-shortcut-filter"))
        .hint_text("Filter by action or key")
        .desired_width(f32::INFINITY);
    let r = controls::field(ui, field);
    if cx.keyboard {
        r.request_focus();
    }
    ui.add_space(4.0);
    let groups = keys.filter(filter);
    d.body(ui, |ui| {
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
    d.finish(ui, Footer::new().note("Esc to close").close()).dismiss
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
