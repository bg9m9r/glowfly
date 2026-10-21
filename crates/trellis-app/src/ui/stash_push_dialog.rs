//! The Stash dialog (`Modal::StashPush`): a message, what goes into the stash (every change,
//! only the staged changes, or files ticked in a list of the working tree) and whether the
//! staged changes also stay staged (not offered with only the staged changes; unticked each
//! time the dialog opens). The list is built from the snapshot every frame, so a refresh adds
//! or removes rows; ticks are kept by path.
//! `update` checks the choice again before running anything.

use super::common::{status_color, status_label, widget_row_height};
use super::dialog_frame::{Cx, Dialog, Footer};
use super::theme::color;
use super::widgets::form_message::{self, Message};
use super::widgets::text_field::{self, TextField};
use super::widgets::{FieldGroup, buttons};
use crate::msg::{Msg, StashMsg};
use crate::state::stashes::{NO_FILES, NOTHING, StashPushForm, push_blocked, stashable, wip_files};
use crate::state::touched::Touched;
use crate::ui::theme::controls;
use crate::ui::theme::type_scale;
use eframe::egui::{self, RichText, Sense};
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
/// focus once when the dialog opens, unless `cx.keyboard` is false (a credential prompt is open
/// above it). Stash waits while a job runs (`idle` is false); on submit the dialog stays open
/// until `update` starts the stash.
pub fn show(ui: &mut egui::Ui, cx: Cx, form: &mut StashPushForm, snap: Option<&Snapshot>, idle: bool, msgs: &mut Vec<Msg>) -> bool {
    let on = match snap.and_then(|s| s.head_branch.as_deref()) {
        Some(b) => format!("on {b}"),
        None => "on detached HEAD".to_owned(),
    };
    let t = cx.t;
    let focus = cx.keyboard;
    let d = Dialog::new(cx, "Stash changes").subtitle(on).begin(ui);
    let r = FieldGroup::new("Message").optional(cx.t.tr("dialog.field.optional")).show(ui, |ui| ui.add(TextField::new(&mut form.message)));
    // Read before focus is taken, or the Enter that just left the field is missed.
    let submit = text_field::entered(ui, &r);
    if focus && std::mem::take(&mut form.focus_message) {
        r.request_focus();
    }
    ui.add_space(8.0);
    ui.label(type_scale::section_text(ui, "What to stash"));
    for (scope, text, note) in CHOICES {
        controls::radio_value(ui, &mut form.scope, scope, text);
        ui.indent(text, |ui| ui.label(RichText::new(note).color(color::weak())));
    }
    if let (StashScope::Files, Some(snap)) = (form.scope, snap) {
        ui.add_space(4.0);
        checklist(ui, form, snap);
    }
    ui.add_space(8.0);
    if form.scope == StashScope::Staged {
        // Drawn unticked whatever the form holds, which comes back with another choice.
        ui.add_enabled(false, controls::Check::new(&mut false, t.tr("stash.push.keep_index.check")))
            .on_disabled_hover_text(t.tr("stash.push.keep_index.staged_tip"));
    } else {
        controls::checkbox(ui, &mut form.keep_index, t.tr("stash.push.keep_index.check"));
    }
    ui.indent("keep-index", |ui| ui.add(egui::Label::new(RichText::new(t.tr("stash.push.keep_index.help")).color(color::weak())).wrap()));
    let paths = snap.map(|s| form.paths(s)).unwrap_or_default();
    let blocked = snap.and_then(|s| push_blocked(trellis_core::i18n::Lang::En, s, form.scope, &paths));
    match blocked {
        // A prompt for what the list needs, not a problem.
        Some(NO_FILES) => form_message::field(ui, &Touched::all(), "files", Some(Message::info("Choose at least one file."))),
        Some(why) => {
            ui.add_space(6.0);
            form_message::form(ui, Message::warning(why));
        }
        None => {}
    }
    // Without a snapshot there is nothing to check, and nothing to stash.
    // The body already says why, except without a snapshot.
    let why = if blocked.is_some() { None } else { snap.is_none().then_some(NOTHING).or((!idle).then_some(BUSY)) };
    let blocked = blocked.or(snap.is_none().then_some(NOTHING));
    let valid = blocked.is_none() && idle;
    let stash = buttons::primary("Stash").enabled(valid).why(why);
    let out = d.finish(ui, Footer::new().cancel().primary(stash).submit(submit));
    if out.primary {
        msgs.push(Msg::Stash(StashMsg::Push { scope: form.scope, message: form.message.clone(), paths, keep_index: form.keeps_index() }));
    }
    out.dismiss
}

/// Select all / Select none, the ticked count, and one row per working-tree file: its box,
/// status letter and path (a click on the path also toggles it). A row Trellis can't stash
/// safely (`stashable`) has its box off and says why after the path.
fn checklist(ui: &mut egui::Ui, form: &mut StashPushForm, snap: &Snapshot) {
    let files = wip_files(snap);
    let ticked = files.iter().filter(|f| stashable(f) && form.chosen.contains(&f.path)).count();
    ui.horizontal(|ui| {
        if ui.add(buttons::secondary("Select all").small()).clicked() {
            form.chosen.extend(files.iter().filter(|f| stashable(f)).map(|f| f.path.clone()));
        }
        if ui.add(buttons::secondary("Select none").small()).clicked() {
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
                    let boxed = ui.add_enabled(usable, controls::Check::without_text(&mut on)).changed();
                    ui.label(
                        RichText::new(status_label(f.status).to_string())
                            .font(type_scale::mono(type_scale::BODY))
                            .color(status_color(f.status)),
                    );
                    let sense = if usable { Sense::click() } else { Sense::hover() };
                    let clicked = ui.add(egui::Label::new(&f.path).truncate().selectable(false).sense(sense)).clicked();
                    super::path_warnings::mark(ui, trellis_core::i18n::Lang::En, f);
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
