//! The row menu's Open in external editor: its own group between the copy items and the line
//! actions, for the diff's working file (`editor_line::working_file`), opening at the first
//! selected line, else the line the menu was opened on (`editor_line::choose`).

use super::DiffView;
use crate::widgets::menu;
use eframe::egui::Ui;
use glowfly_model::msg::Msg;
use glowfly_model::state::diff_text::{LineRef, TextSel};
use glowfly_model::state::tools::editor_line::{self, Picked};

/// The entry for the menu opened on line `at`, with the text selection `sel`; none when the
/// diff has no working file.
pub(super) fn row_item(ui: &mut Ui, v: &DiffView, sel: Option<&TextSel>, at: LineRef, msgs: &mut Vec<Msg>) {
    let Some(f) = (v.working_file)() else { return };
    menu::separator(ui);
    let p = Picked::of(v.target, v.diff, v.sel, sel, v.text.shown_gen, Some(at));
    let line = editor_line::choose(&p, || None);
    crate::file_menu::editor_item(ui, v.t, msgs, &f.path, f.missing, f.setup, line);
}
