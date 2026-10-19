//! The Add submodule dialog (a URL, the folder it goes in, an optional branch), the Edit
//! submodule dialog (its URL and branch) and the Remove submodule confirmation (what it deletes
//! and what would be lost). The Add dialog's text checks
//! are the pure rules in `state::submodules::add`; the folder's disk check is the runner's, asked
//! for by `update` after each edit and only read here.

use super::theme::color;
use crate::msg::Msg;
use crate::state::submodules::add::{AddForm, Disk, relative_url_hint};
use crate::state::submodules::edit::EditForm;
use crate::state::submodules::remove::RemoveForm;
use crate::submodule::SubmoduleMsg;
use crate::ui::theme::controls;
use eframe::egui::{self, Id, Key, RichText};
use trellis_core::repo::Snapshot;
use trellis_core::submodule_ops::default_path;

/// The Add dialog; returns true when Cancel closes it (`update` closes it once the job starts).
/// The URL field has the keyboard focus unless another field has it, or `focus` is false (a
/// credential prompt is open above it). The button is disabled while a job runs (`idle` false).
pub fn add(ui: &mut egui::Ui, form: &mut AddForm, snap: Option<&Snapshot>, focus: bool, idle: bool, msgs: &mut Vec<Msg>) -> bool {
    let mut close = false;
    let (url_id, path_id, branch_id) =
        (Id::new("trellis-submodule-url"), Id::new("trellis-submodule-path"), Id::new("trellis-submodule-branch"));
    let (mut enter, mut edited) = (false, false);
    let pressed = |ui: &egui::Ui, r: &egui::Response| r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
    ui.heading("Add submodule");
    ui.add_space(6.0);

    ui.label(RichText::new("Repository URL").color(color::weak()));
    let r = controls::field(
        ui,
        egui::TextEdit::singleline(&mut form.url)
            .id(url_id)
            .hint_text("https://… or the path of a repository on this computer")
            .desired_width(f32::INFINITY),
    );
    if r.changed() {
        if !form.path_edited {
            form.path = default_path(&form.url);
        }
        edited = true;
    }
    // Read before focus is requested again below, which would hide that Enter left the field.
    enter |= pressed(ui, &r);
    if focus && !ui.memory(|m| m.has_focus(path_id) || m.has_focus(branch_id)) {
        r.request_focus();
    }
    let url_bad = form.url_problem();
    if let Some(p) = url_bad {
        let col = if form.url.trim().is_empty() { color::weak() } else { color::red() };
        ui.label(RichText::new(p).color(col));
    } else if let Some(h) = relative_url_hint(&form.url) {
        ui.label(RichText::new(h).color(color::weak()));
    }

    ui.add_space(6.0);
    ui.label(RichText::new("Path").color(color::weak()));
    let r = controls::field(
        ui,
        egui::TextEdit::singleline(&mut form.path)
            .id(path_id)
            .hint_text("A folder in this repository, e.g. libs/lib")
            .desired_width(f32::INFINITY),
    );
    if r.changed() {
        form.path_edited = true;
        edited = true;
    }
    enter |= pressed(ui, &r);
    let path_ok = match snap.map(|s| form.rel(s)) {
        None => false,
        Some(Err(e)) => {
            let col = if form.path.trim().is_empty() { color::weak() } else { color::red() };
            ui.label(RichText::new(e).color(col));
            false
        }
        Some(Ok(rel)) => match form.disk_state(&rel) {
            Disk::Waiting => {
                ui.label(RichText::new("Checking…").color(color::weak()));
                false
            }
            Disk::Err(e) => {
                ui.label(RichText::new(e).color(color::red()));
                false
            }
            Disk::Ok(warnings) => {
                for w in warnings {
                    ui.label(RichText::new(w).color(color::amber()));
                }
                true
            }
        },
    };

    ui.add_space(6.0);
    ui.label(RichText::new("Branch").color(color::weak()));
    let r = controls::field(
        ui,
        egui::TextEdit::singleline(&mut form.branch)
            .id(branch_id)
            .hint_text("Optional. Leave empty for the remote's default branch")
            .desired_width(f32::INFINITY),
    );
    enter |= pressed(ui, &r);
    let branch_bad = form.branch_problem();
    if let Some(p) = branch_bad {
        ui.label(RichText::new(p).color(color::red()));
    }

    ui.add_space(6.0);
    ui.label(
        RichText::new("The repository is cloned into the folder and staged with .gitmodules; a commit records the new submodule.")
            .color(color::weak()),
    );
    ui.add_space(8.0);
    let valid = url_bad.is_none() && path_ok && branch_bad.is_none() && idle;
    ui.horizontal(|ui| {
        if ui.add_enabled(valid, egui::Button::new("Add submodule")).clicked() || (enter && valid) {
            msgs.push(Msg::Submodule(SubmoduleMsg::Add));
        }
        if ui.button("Cancel").clicked() {
            close = true;
        }
    });
    if edited {
        msgs.push(Msg::Submodule(SubmoduleMsg::AddEdited));
    }
    close
}

const BRANCH_HINT: &str = "The branch an update with --remote follows. Empty means the remote's default branch; \".\" \
                           follows this repository's current branch name.";
const EDIT_NOTE: &str = "The change is written to .gitmodules (commit it to share it), and to this repository's config \
                         and the submodule's origin when it is checked out.";

/// The Edit dialog; returns true when Cancel closes it (`update` closes it once the job starts).
/// The URL field has the keyboard focus unless the branch field has it, or `focus` is false. Save
/// is disabled while nothing changed, a field is invalid or a job runs (`idle` false).
pub fn edit(ui: &mut egui::Ui, form: &mut EditForm, focus: bool, idle: bool, msgs: &mut Vec<Msg>) -> bool {
    let mut close = false;
    let (url_id, branch_id) = (Id::new("trellis-submodule-edit-url"), Id::new("trellis-submodule-edit-branch"));
    let mut enter = false;
    let pressed = |ui: &egui::Ui, r: &egui::Response| r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
    ui.heading(format!("Edit submodule {}", form.path));
    if form.name != form.path {
        ui.label(RichText::new(format!("Named {} in .gitmodules", form.name)).color(color::weak()));
    }
    ui.add_space(6.0);

    ui.label(RichText::new("Repository URL").color(color::weak()));
    let r = controls::field(ui, egui::TextEdit::singleline(&mut form.url).id(url_id).desired_width(f32::INFINITY));
    enter |= pressed(ui, &r);
    if focus && !ui.memory(|m| m.has_focus(branch_id)) {
        r.request_focus();
    }
    let url_bad = form.url_problem();
    if let Some(p) = url_bad {
        ui.label(RichText::new(p).color(color::red()));
    } else if let Some(h) = relative_url_hint(&form.url) {
        ui.label(RichText::new(h).color(color::weak()));
    }

    ui.add_space(6.0);
    ui.label(RichText::new("Branch").color(color::weak()));
    let r = controls::field(
        ui,
        egui::TextEdit::singleline(&mut form.branch)
            .id(branch_id)
            .hint_text("Empty for the remote's default branch")
            .desired_width(f32::INFINITY),
    );
    enter |= pressed(ui, &r);
    let branch_bad = form.branch_problem();
    if let Some(p) = branch_bad {
        ui.label(RichText::new(p).color(color::red()));
    } else {
        ui.label(RichText::new(BRANCH_HINT).color(color::weak()));
    }

    ui.add_space(6.0);
    ui.label(RichText::new(EDIT_NOTE).color(color::weak()));
    ui.add_space(8.0);
    let valid = form.changed() && url_bad.is_none() && branch_bad.is_none() && idle;
    ui.horizontal(|ui| {
        if ui.add_enabled(valid, egui::Button::new("Save")).clicked() || (enter && valid) {
            msgs.push(Msg::Submodule(SubmoduleMsg::Edit));
        }
        if ui.button("Cancel").clicked() {
            close = true;
        }
    });
    close
}

/// The Remove confirmation; returns true when Cancel closes it. The button is disabled while a
/// job runs (`idle` false).
pub fn remove(ui: &mut egui::Ui, form: &RemoveForm, idle: bool, msgs: &mut Vec<Msg>) -> bool {
    let mut close = false;
    ui.heading(format!("Remove submodule {}", form.path));
    ui.add_space(6.0);
    ui.label("This deletes:");
    ui.indent("trellis-submodule-remove-lines", |ui| {
        for line in form.deleted_lines() {
            ui.label(line);
        }
    });
    for w in form.warnings() {
        ui.add_space(6.0);
        ui.label(RichText::new(w).color(color::red()));
    }
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        if ui.add_enabled(idle, egui::Button::new(RichText::new("Remove submodule").color(color::red()))).clicked() {
            msgs.push(Msg::Submodule(SubmoduleMsg::Remove));
        }
        if ui.button("Cancel").clicked() {
            close = true;
        }
    });
    close
}
