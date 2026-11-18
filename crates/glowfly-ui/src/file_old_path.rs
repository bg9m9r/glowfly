//! A renamed or copied file's old path on a file row: the weak note ("from old/path.rs", "copy
//! of src/path.rs") right after the new path, in monospace at the path's size. When the row is
//! narrow the note gives way first: it is cut at its end with "…", dropped whole under
//! `MIN_NOTE`, and only then is the new path cut (`share`). Both texts show the whole "old to
//! new" sentence on hover. The counts, marks and the row's end part keep their width, as they do
//! beside a plain path. The Stash dialog's checklist shares `share`.

use crate::list_row::file_tree::file_text;
use crate::theme::{color, spacing};
use crate::widgets;
use eframe::egui::{self, RichText, TextStyle, TextWrapMode, Ui, WidgetText};
use glowfly_core::i18n::Lang;
use glowfly_core::repo::FileEntry;
use glowfly_model::state::file_tree::file_name;
use glowfly_model::state::old_path::{OldPath, shown_in_tree};

/// A cut note is dropped when less than this much is left for it.
pub(crate) const MIN_NOTE: f32 = 48.0;

/// The space between the new path and its note.
pub(crate) const NOTE_GAP: f32 = spacing::GRID;

/// How `room` px (the path's own room: the row's width less its padding and what follows) is
/// shared between a path of `path_w` and its note of `note_w`: the path's width, and the
/// note's room, `None` when the note is dropped. Both whole when they fit; else the note is cut
/// at its end while at least `MIN_NOTE` is left for it; else it is dropped and only then is the
/// path cut (`room` at most). Narrowing the room never brings a dropped note back.
pub(crate) fn share(room: f32, path_w: f32, note_w: f32) -> (f32, Option<f32>) {
    if path_w + NOTE_GAP + note_w <= room {
        return (path_w, Some(note_w));
    }
    let left = room - path_w - NOTE_GAP;
    if left >= MIN_NOTE { (path_w, Some(left)) } else { (room.min(path_w).max(0.0), None) }
}

/// A file row's path in monospace leaving `reserve` px after it, and, for a row with an old path,
/// its note. Without an old path it is `file_tree::file_text`. In a tree (`depth`) the row shows
/// the file's name and the note the old name when it is in the same folder (else the whole old
/// path). Returns the path's response.
pub(crate) fn path_text(ui: &mut Ui, t: Lang, f: &FileEntry, depth: Option<usize>, selected: bool, reserve: f32) -> egui::Response {
    let Some(old) = OldPath::of(f) else { return file_text(ui, &f.path, depth, selected, reserve) };
    let (path, shown_old) = match depth {
        Some(_) => (file_name(&f.path), shown_in_tree(old.old, &f.path)),
        None => (f.path.as_str(), old.old),
    };
    let whole = old.whole(t, &f.path);
    let note = old.note(t, shown_old);
    let pad = ui.spacing().button_padding;
    let room = (ui.available_width() - 2.0 * pad.x - reserve).max(0.0);
    let (_, note_room) = share(room, width(ui, path), width(ui, &note));
    // The path is cut to what the note leaves it; the note keeps the rest of the row's room.
    let after_path = reserve + note_room.map_or(0.0, |n| NOTE_GAP + n);
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let r = widgets::row_text_hover(ui, RichText::new(path).monospace(), selected, after_path, &whole);
        if note_room.is_some() {
            ui.add_space(NOTE_GAP);
            ui.spacing_mut().button_padding.x = 0.0;
            let weak = RichText::new(note).monospace().color(color::weak());
            widgets::row_text_hover(ui, weak, false, reserve, &whole);
        }
        r
    })
    .inner
}

/// The width of `text` in monospace on one line, as a row paints it.
pub(crate) fn width(ui: &Ui, text: &str) -> f32 {
    WidgetText::from(RichText::new(text).monospace()).into_galley(ui, Some(TextWrapMode::Extend), f32::INFINITY, TextStyle::Button).size().x
}

#[cfg(test)]
#[path = "file_old_path_tests.rs"]
mod tests;
