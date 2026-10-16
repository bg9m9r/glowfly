//! The Init dialog: the folder for the new repository (checked on disk by `update`), the initial
//! branch, a `.gitignore` template, a license and whether to commit them, checked as the user
//! types with the pure rules in `state::init`.

use super::clone_dialog::{destination, note, problem};
use super::theme::color;
use crate::msg::{FolderPurpose, InitMsg, Msg, PrefsMsg};
use crate::state::init::{FolderLine, GITIGNORE_KEPT, InitForm};
use eframe::egui::{self, Id, RichText};
use trellis_core::init_ops::{GitignoreTemplate, INITIAL_COMMIT, License};

/// Width of the labels before the fields, so the fields line up.
const LABEL_WIDTH: f32 = 110.0;

fn label(ui: &mut egui::Ui, text: &str) {
    let size = egui::vec2(LABEL_WIDTH, ui.spacing().interact_size.y);
    ui.allocate_ui_with_layout(size, egui::Layout::left_to_right(egui::Align::Center), |ui| {
        ui.set_min_width(LABEL_WIDTH);
        ui.label(text);
    });
}

fn folder(ui: &mut egui::Ui, form: &mut InitForm, focus: bool, msgs: &mut Vec<Msg>) {
    ui.horizontal(|ui| {
        label(ui, "Folder");
        let edit = ui.add(
            egui::TextEdit::singleline(&mut form.folder)
                .id(Id::new("trellis-init-folder"))
                .hint_text("Folder for the new repository")
                .desired_width(250.0),
        );
        if edit.changed() {
            msgs.push(Msg::Init(InitMsg::FolderEdited));
        }
        if focus && ui.memory(|m| m.focused().is_none()) {
            edit.request_focus();
        }
        if ui.button("Browse…").clicked() {
            msgs.push(Msg::Prefs(PrefsMsg::PickFolder(FolderPurpose::InitRepo)));
        }
    });
    match form.folder_line() {
        Some(FolderLine::Problem(t)) => problem(ui, Some(&t.to_owned())),
        Some(FolderLine::Note(t)) => note(ui, t),
        None => {}
    }
    if let Some(f) = form.facts() {
        destination(ui, &trellis_core::native_path::shown(&f.path));
    }
}

fn gitignore(ui: &mut egui::Ui, form: &mut InitForm) {
    ui.horizontal(|ui| {
        label(ui, ".gitignore");
        let shown = form.gitignore.map_or("None", GitignoreTemplate::label);
        egui::ComboBox::from_id_salt("trellis-init-gitignore").selected_text(shown).width(160.0).show_ui(ui, |ui| {
            ui.selectable_value(&mut form.gitignore, None, "None");
            for t in GitignoreTemplate::ALL {
                ui.selectable_value(&mut form.gitignore, Some(t), t.label());
            }
        });
    });
    ui.checkbox(&mut form.os_editor, "Also ignore operating system and editor files (macOS, Windows, common editors)");
    if form.facts().is_some_and(|f| f.gitignore) {
        note(ui, GITIGNORE_KEPT);
    }
}

fn license(ui: &mut egui::Ui, form: &mut InitForm, year: i32) {
    ui.horizontal(|ui| {
        label(ui, "License");
        let shown = form.license.map_or("None", License::label);
        egui::ComboBox::from_id_salt("trellis-init-license").selected_text(shown).width(160.0).show_ui(ui, |ui| {
            ui.selectable_value(&mut form.license, None, "None");
            for l in License::ALL {
                ui.selectable_value(&mut form.license, Some(l), l.label());
            }
        });
    });
    if form.license.is_some_and(License::needs_holder) {
        ui.horizontal(|ui| {
            label(ui, "Copyright holder");
            ui.add(egui::TextEdit::singleline(&mut form.holder).id(Id::new("trellis-init-holder")).desired_width(250.0));
        });
        problem(ui, form.problems().holder.as_ref());
        note(ui, &format!("{year} and this name go in the copyright line."));
    }
    if form.license.is_some()
        && let Some(kept) = form.license_kept()
    {
        note(ui, &kept);
    }
}

/// The Init dialog; returns true when it should close. `year` is the local year, for a
/// license's copyright line. The folder field takes the focus when nothing has it, unless
/// `focus` is false (a credential prompt is open above it).
pub fn show(ui: &mut egui::Ui, form: &mut InitForm, year: i32, focus: bool, msgs: &mut Vec<Msg>) -> bool {
    let mut close = false;
    ui.heading("Start a new repository");
    ui.add_space(6.0);
    folder(ui, form, focus, msgs);
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        label(ui, "Initial branch");
        ui.add(
            egui::TextEdit::singleline(&mut form.branch).id(Id::new("trellis-init-branch")).hint_text("git's default").desired_width(160.0),
        );
    });
    problem(ui, form.problems().branch.as_ref());
    ui.add_space(6.0);
    gitignore(ui, form);
    ui.add_space(6.0);
    license(ui, form, year);
    ui.add_space(6.0);
    let commit_text = format!("Commit these files as \"{INITIAL_COMMIT}\"");
    if form.can_commit(year) {
        ui.checkbox(&mut form.commit, commit_text);
    } else {
        // Treated as off: nothing would be committed.
        ui.add_enabled(false, egui::Checkbox::new(&mut false, RichText::new(commit_text).color(color::weak())));
    }
    ui.add_space(8.0);
    let blocked = form.problems().blocked;
    ui.horizontal(|ui| {
        if ui.add_enabled(!blocked, egui::Button::new("Create repository")).clicked()
            && let Some(req) = form.request(year)
        {
            msgs.push(Msg::Init(InitMsg::Start(req)));
            close = true;
        }
        if ui.button("Cancel").clicked() {
            close = true;
        }
    });
    close
}
