//! The Create patch dialog for uncommitted changes (`Modal::SavePatch`): which changes go in
//! (all, only staged, only unstaged), untracked files, and Only selected files with a checklist
//! of the scope's files. The list is built from the snapshot every frame, so a refresh adds or
//! removes rows; ticks are kept by path. `update` checks the choice again before saving.

use super::common::{status_color, status_label, widget_row_height};
use super::theme::color;
use crate::msg::Msg;
use crate::patch::PatchMsg;
use crate::state::patch::changes::{SaveChangesForm, blocked, rows, untracked_count};
use crate::ui::theme::controls;
use eframe::egui::{self, RichText, Sense};
use trellis_core::patch_ops::{NO_CHANGES, WipScope};
use trellis_core::repo::Snapshot;

const BUSY: &str = "Another operation is still running.";

/// The choices: scope, label and the note under it.
const CHOICES: [(WipScope, &str, &str); 3] = [
    (WipScope::All, "All changes", "Staged and unstaged changes against HEAD."),
    (WipScope::Staged, "Only staged changes", "What the next commit would contain."),
    (WipScope::Unstaged, "Only unstaged changes", "Working-tree edits not yet staged."),
];

/// Draws the dialog; returns true when it should close. Save… waits while a job runs (`idle`
/// is false); on Save… the dialog stays open until `update` asks where to save.
pub fn show(ui: &mut egui::Ui, form: &mut SaveChangesForm, snap: Option<&Snapshot>, idle: bool, msgs: &mut Vec<Msg>) -> bool {
    let mut close = false;
    ui.heading("Create patch");
    let on = match snap.and_then(|s| s.head_branch.as_deref()) {
        Some(b) => format!("From uncommitted changes on {b}"),
        None => "From uncommitted changes on detached HEAD".to_owned(),
    };
    ui.label(RichText::new(on).color(color::weak()));
    ui.add_space(8.0);
    ui.label(RichText::new("What goes in").strong());
    for (scope, text, note) in CHOICES {
        controls::radio_value(ui, &mut form.scope, scope, text);
        ui.indent(text, |ui| ui.label(RichText::new(note).color(color::weak())));
    }
    ui.add_space(4.0);
    if form.scope != WipScope::Staged && !form.pick {
        let n = snap.map(|s| untracked_count(s, form.scope)).unwrap_or(0);
        controls::checkbox(ui, &mut form.untracked, format!("Include untracked files ({n})"));
    }
    controls::checkbox(ui, &mut form.pick, "Only selected files");
    if let (true, Some(snap)) = (form.pick, snap) {
        ui.add_space(4.0);
        checklist(ui, form, snap);
    }
    let blocked = snap.map_or(Some(NO_CHANGES), |s| blocked(form, s));
    if let Some(why) = blocked {
        ui.add_space(4.0);
        ui.label(RichText::new(why).color(color::red()));
    }
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        let mut button = ui.add_enabled(blocked.is_none() && idle, egui::Button::new("Save…"));
        if let Some(why) = blocked.or((!idle).then_some(BUSY)) {
            button = button.on_disabled_hover_text(why);
        }
        if button.clicked() {
            let paths = form.pick.then(|| snap.map(|s| form.ticked(s)).unwrap_or_default());
            msgs.push(Msg::Patch(PatchMsg::SaveChanges { scope: form.scope, paths, untracked: form.untracked }));
        }
        if ui.button("Cancel").clicked() {
            close = true;
        }
    });
    close
}

/// Select all / Select none, the ticked count, and one row per file of the scope: its box,
/// status letter and path (a click on the path also toggles it).
fn checklist(ui: &mut egui::Ui, form: &mut SaveChangesForm, snap: &Snapshot) {
    let files = rows(snap, form.scope);
    let ticked = files.iter().filter(|f| form.chosen.contains(&f.path)).count();
    ui.horizontal(|ui| {
        if ui.small_button("Select all").clicked() {
            form.chosen.extend(files.iter().map(|f| f.path.clone()));
        }
        if ui.small_button("Select none").clicked() {
            form.chosen.clear();
        }
        let noun = if files.len() == 1 { "file" } else { "files" };
        ui.label(RichText::new(format!("{ticked} of {} {noun}", files.len())).color(color::weak()));
    });
    let row_h = widget_row_height(ui);
    egui::ScrollArea::vertical().id_salt("patch-files").max_height(200.0).auto_shrink([false, true]).show_rows(
        ui,
        row_h,
        files.len(),
        |ui, range| {
            for f in &files[range] {
                ui.horizontal(|ui| {
                    let mut on = form.chosen.contains(&f.path);
                    let boxed = ui.add(controls::Check::without_text(&mut on)).changed();
                    ui.label(RichText::new(status_label(f.status).to_string()).monospace().strong().color(status_color(f.status)));
                    if ui.add(egui::Label::new(&f.path).truncate().selectable(false).sense(Sense::click())).clicked() {
                        on = !on;
                    } else if !boxed {
                        return;
                    }
                    if on {
                        form.chosen.insert(f.path.clone());
                    } else {
                        form.chosen.remove(&f.path);
                    }
                });
            }
        },
    );
}
