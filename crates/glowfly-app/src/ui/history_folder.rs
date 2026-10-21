//! A folder history entry's changed files: the files under the folder that the selected commit
//! changed (against its first parent), one selected, whose diff or text shows below.

use super::common::{menu_item, status_color};
use super::file_menu;
use super::theme::color;
use super::widgets;
use crate::msg::{HistoryMsg, Msg};
use crate::state::AppState;
use eframe::egui::{self, RichText, Ui};
use glowfly_core::repo::HistoryEntry;

/// The caption and the list of `e`'s files, in a scroll area of its own capped at about a third
/// of the panel's height; `scroll` brings that file into view.
pub(super) fn files(ui: &mut Ui, st: &AppState, e: &HistoryEntry, scroll: Option<usize>, msgs: &mut Vec<Msg>) {
    let Some(view) = &st.history else { return };
    let folder = &view.path;
    let caption = match e.files.len() {
        0 => "No file changes in this folder".to_owned(),
        1 => format!("1 file changed in {folder}/"),
        n => format!("{n} files changed in {folder}/"),
    };
    ui.label(RichText::new(caption).color(color::weak()));
    if e.files.is_empty() {
        return;
    }
    let prefix = format!("{folder}/");
    egui::ScrollArea::vertical().id_salt(("history-folder-files", e.oid)).max_height(ui.available_height() * 0.35).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for (i, f) in e.files.iter().enumerate() {
            let shown = f.path.strip_prefix(&prefix).unwrap_or(&f.path);
            let selected = view.file == i;
            let r = widgets::file_row(ui, selected, shown, |ui| {
                ui.label(RichText::new(f.status.to_string()).monospace().color(status_color(f.status)));
                widgets::row_text(ui, RichText::new(shown), selected);
            })
            .response;
            if scroll == Some(i) {
                r.scroll_to_me(None);
            }
            if r.clicked() {
                msgs.push(Msg::History(HistoryMsg::SelectHistoryFile(i)));
            }
            r.context_menu(|ui| {
                let deleted = (f.status == 'D').then_some("This commit deletes the file");
                file_menu::history_items(ui, st.settings.lang(), msgs, &f.path, Some(e.oid), deleted);
                menu_item(msgs, ui, "Copy path", Msg::Copy(f.path.clone()));
            });
        }
    });
}
