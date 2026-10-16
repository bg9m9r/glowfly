//! The Stash dialog (`Modal::StashPush`): a message and what goes into the stash (every change,
//! only the staged changes, or files ticked in a list of the working tree). The list is built
//! from the snapshot every frame, so a refresh adds or removes rows; ticks are kept by path.
//! `update` checks the choice again before running anything.

use super::common::{status_color, status_label, widget_row_height};
use super::theme::color;
use crate::msg::{Msg, StashMsg};
use crate::state::stashes::{NO_FILES, NOTHING, StashPushForm, push_blocked, stashable, wip_files};
use eframe::egui::{self, Key, RichText, Sense};
use trellis_core::repo::Snapshot;
use trellis_core::stash_ops::StashScope;

const BUSY: &str = "Another operation is still running.";

/// The choices: scope, label and the note under it.
const CHOICES: [(StashScope, &str, &str); 3] = [
    (StashScope::All, "All changes", "Untracked files included."),
    (StashScope::Staged, "Only staged changes", "Unstaged changes and untracked files stay in the working tree. Needs git 2.35 or newer."),
    (StashScope::Files, "Selected files", "Only the files you tick, tracked or untracked."),
];

/// Draws the dialog; returns true when it should close. The message field takes the keyboard
/// focus once when the dialog opens, unless `focus` is false (a credential prompt is open above
/// it). Stash waits while a job runs (`idle` is false); on submit the dialog stays open until
/// `update` starts the stash.
pub fn show(ui: &mut egui::Ui, form: &mut StashPushForm, snap: Option<&Snapshot>, focus: bool, idle: bool, msgs: &mut Vec<Msg>) -> bool {
    let mut close = false;
    ui.heading("Stash changes");
    let on = match snap.and_then(|s| s.head_branch.as_deref()) {
        Some(b) => format!("on {b}"),
        None => "on detached HEAD".to_owned(),
    };
    ui.label(RichText::new(on).color(color::weak()));
    ui.add_space(6.0);
    let r = ui.add(egui::TextEdit::singleline(&mut form.message).hint_text("Message (optional)").desired_width(f32::INFINITY));
    // Read before focus is taken, or the Enter that just left the field is missed.
    let submit = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
    if focus && std::mem::take(&mut form.focus_message) {
        r.request_focus();
    }
    ui.add_space(8.0);
    ui.label(RichText::new("What to stash").strong());
    for (scope, text, note) in CHOICES {
        ui.radio_value(&mut form.scope, scope, text);
        ui.indent(text, |ui| ui.label(RichText::new(note).color(color::weak())));
    }
    if let (StashScope::Files, Some(snap)) = (form.scope, snap) {
        ui.add_space(4.0);
        checklist(ui, form, snap);
    }
    let paths = snap.map(|s| form.paths(s)).unwrap_or_default();
    let blocked = snap.and_then(|s| push_blocked(s, form.scope, &paths));
    if let Some(why) = blocked {
        ui.add_space(4.0);
        let text = if why == NO_FILES { "Choose at least one file." } else { why };
        ui.label(RichText::new(text).color(color::red()));
    }
    // Without a snapshot there is nothing to check, and nothing to stash.
    let blocked = blocked.or(snap.is_none().then_some(NOTHING));
    ui.add_space(10.0);
    let valid = blocked.is_none() && idle;
    ui.horizontal(|ui| {
        let mut button = ui.add_enabled(valid, egui::Button::new("Stash"));
        if let Some(why) = blocked.or((!idle).then_some(BUSY)) {
            button = button.on_disabled_hover_text(why);
        }
        if button.clicked() || (submit && valid) {
            msgs.push(Msg::Stash(StashMsg::Push { scope: form.scope, message: form.message.clone(), paths: paths.clone() }));
        }
        if ui.button("Cancel").clicked() {
            close = true;
        }
    });
    close
}

/// Select all / Select none, the ticked count, and one row per working-tree file: its box,
/// status letter and path (a click on the path also toggles it). A row Trellis can't stash
/// safely (`stashable`) has its box off and says why after the path.
fn checklist(ui: &mut egui::Ui, form: &mut StashPushForm, snap: &Snapshot) {
    let files = wip_files(snap);
    let ticked = files.iter().filter(|f| stashable(f) && form.chosen.contains(&f.path)).count();
    ui.horizontal(|ui| {
        if ui.small_button("Select all").clicked() {
            form.chosen.extend(files.iter().filter(|f| stashable(f)).map(|f| f.path.clone()));
        }
        if ui.small_button("Select none").clicked() {
            form.chosen.clear();
        }
        let total = files.iter().filter(|f| stashable(f)).count();
        let noun = if total == 1 { "file" } else { "files" };
        ui.label(RichText::new(format!("{ticked} of {total} {noun}")).color(color::weak()));
    });
    let row_h = widget_row_height(ui);
    egui::ScrollArea::vertical().id_salt("stash-push-files").max_height(220.0).auto_shrink([false, true]).show_rows(
        ui,
        row_h,
        files.len(),
        |ui, range| {
            for f in &files[range] {
                ui.horizontal(|ui| {
                    let usable = stashable(f);
                    let mut on = usable && form.chosen.contains(&f.path);
                    let boxed = ui.add_enabled(usable, egui::Checkbox::without_text(&mut on)).changed();
                    ui.label(RichText::new(status_label(f.status).to_string()).monospace().strong().color(status_color(f.status)));
                    let sense = if usable { Sense::click() } else { Sense::hover() };
                    let clicked = ui.add(egui::Label::new(&f.path).truncate().selectable(false).sense(sense)).clicked();
                    super::path_warnings::mark(ui, f);
                    if clicked {
                        on = !on;
                    }
                    if boxed || clicked {
                        if on {
                            form.chosen.insert(f.path.clone());
                        } else {
                            form.chosen.remove(&f.path);
                        }
                    }
                });
            }
        },
    );
}
