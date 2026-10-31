//! The keyboard shortcut sheet (Ctrl+/): a filter field over every binding in the table,
//! grouped, each row with its keys drawn as key caps; Repository commands rows without a key
//! are left out. The keys every menu answers to follow as a Menus group (`sheet_menus.rs`).
//! A reference only; rows don't run anything.

use super::Caps;
use super::sheet_menus;
use crate::keymap::{Chord, Keymap};
use crate::ui::dialog_frame::{Cx, Dialog, Footer, Size};
use crate::ui::theme::color;
use crate::ui::theme::type_scale;
use crate::ui::widgets::TextField;
use eframe::egui::{Align, Id, Layout, RichText, Sense, Ui, vec2};

/// Draws the sheet inside the dialog with the keys in effect; `cx.keyboard` gives the filter
/// field the keyboard (not under a credential prompt). Returns true when Close is clicked.
pub fn show(ui: &mut Ui, cx: Cx, filter: &mut String, keys: &Keymap) -> bool {
    let mut d = Dialog::new(cx, "Keyboard shortcuts").size(Size::Wide).resizable("shortcut-sheet").begin(ui);
    let r = ui.add(TextField::new(filter).id(Id::new("glowfly-shortcut-filter")).hint("Filter by action or key"));
    if cx.keyboard {
        r.request_focus();
    }
    ui.add_space(4.0);
    let groups = keys.filter_for_sheet(filter);
    let menus = sheet_menus::filtered(filter);
    d.body(ui, |ui| {
        if groups.is_empty() && menus.is_empty() {
            ui.add_space(8.0);
            ui.label(RichText::new("No shortcuts match").color(color::weak()));
        }
        for (group, rows) in &groups {
            title(ui, group.title());
            for b in rows {
                row(ui, b.label, b.when.note(), keys.chords(b.id), &keys.keys_text(b.id));
            }
        }
        if !menus.is_empty() {
            title(ui, sheet_menus::TITLE);
        }
        for m in menus {
            row(ui, m.label, None, m.keys, &sheet_menus::keys_text(m));
        }
    });
    d.finish(ui, Footer::new().note("Esc to close").close()).dismiss
}

fn title(ui: &mut Ui, text: &str) {
    ui.add_space(8.0);
    ui.label(RichText::new(text).font(type_scale::strong(ui, type_scale::SMALL)).color(color::weak()));
}

/// One row: the action and its context note on the left (wrapping when long), the key caps
/// right-aligned (with no keys, `keys_text` in plain text: the Menus group's typeahead).
fn row(ui: &mut Ui, label: &str, note: Option<&str>, chords: &[Chord], keys_text: &str) {
    let caps = Caps::new(ui, chords, false);
    ui.horizontal(|ui| {
        let left = (ui.available_width() - caps.size.x - 16.0).max(120.0);
        let wrap = Layout::left_to_right(Align::Center).with_main_wrap(true);
        ui.allocate_ui_with_layout(vec2(left, 0.0), wrap, |ui| {
            ui.label(RichText::new(label).color(color::text()));
            if let Some(note) = note {
                ui.label(RichText::new(note).size(type_scale::CAPTION).color(color::weak()));
            }
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if chords.is_empty() {
                ui.label(RichText::new(keys_text).color(color::weak()));
            } else {
                caps.paint(ui, keys_text, Sense::hover());
            }
        });
    });
}
