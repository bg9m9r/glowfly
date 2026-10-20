//! The Init dialog: the folder for the new repository (checked on disk by `update`), the initial
//! branch, a `.gitignore` template, a license and whether to commit them, checked as the user
//! types with the pure rules in `state::init`.

use super::clone_dialog::{destination, note};
use super::dialog_frame::{Cx, Dialog, Footer};
use super::theme::color;
use super::widgets::text_field::{self, TextField};
use super::widgets::{FieldGroup, buttons};
use crate::msg::{FolderPurpose, InitMsg, Msg, PrefsMsg};
use crate::state::init::{FolderLine, GITIGNORE_KEPT, InitForm};
use crate::ui::theme::controls;
use crate::ui::widgets::form_message::{self, Message};
use eframe::egui::{self, Id, RichText};
use trellis_core::init_ops::{GitignoreTemplate, INITIAL_COMMIT, License};

/// The widest a template or license picker gets.
const PICKER_W: f32 = 280.0;

fn folder(ui: &mut egui::Ui, form: &mut InitForm, focus: bool, msgs: &mut Vec<Msg>) {
    let (shown, help) = match form.folder_line() {
        Some(FolderLine::Problem(t)) => (Some(Message::error(t.to_owned())), None),
        Some(FolderLine::Note(t)) => (None, Some(t.to_owned())),
        None => (None, None),
    };
    let mut group = FieldGroup::new("Folder").message(&form.touched, "folder", shown);
    if let Some(help) = help {
        group = group.help(help);
    }
    let mut browse = false;
    let edit = group.show(ui, |ui| {
        let field = TextField::new(&mut form.folder).id(Id::new("trellis-init-folder")).hint("Folder for the new repository");
        let (r, b) = text_field::with_button(ui, field, buttons::secondary("Browse…"));
        browse = b.clicked();
        r
    });
    form_message::track(&mut form.touched, "folder", &edit);
    if edit.changed() {
        msgs.push(Msg::Init(InitMsg::FolderEdited));
    }
    // Not on the Enter that left it: the dialog's frame reads that as running Create.
    if focus && !edit.lost_focus() && ui.memory(|m| m.focused().is_none()) {
        edit.request_focus();
    }
    if browse {
        form.touched.mark("folder");
        msgs.push(Msg::Prefs(PrefsMsg::PickFolder(FolderPurpose::InitRepo)));
    }
    if let Some(f) = form.facts() {
        destination(ui, &trellis_core::native_path::shown(&f.path));
        // As far from the next group as a group's own gap.
        ui.add_space(12.0);
    }
}

fn gitignore(ui: &mut egui::Ui, form: &mut InitForm) {
    FieldGroup::new(".gitignore").show(ui, |ui| {
        let shown = form.gitignore.map_or("None", GitignoreTemplate::label);
        let w = ui.available_width().min(PICKER_W);
        egui::ComboBox::from_id_salt("trellis-init-gitignore")
            .selected_text(shown)
            .width(w)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut form.gitignore, None, "None");
                for t in GitignoreTemplate::ALL {
                    ui.selectable_value(&mut form.gitignore, Some(t), t.label());
                }
            })
            .response
    });
    controls::checkbox(ui, &mut form.os_editor, "Also ignore operating system and editor files (macOS, Windows, common editors)");
    if form.facts().is_some_and(|f| f.gitignore) {
        note(ui, GITIGNORE_KEPT);
    }
    // The checkbox and its note close the .gitignore part, as a group's help line would.
    ui.add_space(12.0);
}

fn license(ui: &mut egui::Ui, form: &mut InitForm, year: i32) {
    FieldGroup::new("License").show(ui, |ui| {
        let shown = form.license.map_or("None", License::label);
        let w = ui.available_width().min(PICKER_W);
        egui::ComboBox::from_id_salt("trellis-init-license")
            .selected_text(shown)
            .width(w)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut form.license, None, "None");
                for l in License::ALL {
                    ui.selectable_value(&mut form.license, Some(l), l.label());
                }
            })
            .response
    });
    if form.license.is_some_and(License::needs_holder) {
        let holder = form.problems().holder;
        let r = FieldGroup::new("Copyright holder")
            .message(&form.touched, "holder", holder.map(Message::error))
            .help(format!("{year} and this name go in the copyright line."))
            .show(ui, |ui| ui.add(TextField::new(&mut form.holder).id(Id::new("trellis-init-holder"))));
        form_message::track(&mut form.touched, "holder", &r);
    }
    if form.license.is_some()
        && let Some(kept) = form.license_kept()
    {
        note(ui, &kept);
    }
}

/// The Init dialog; returns true when it should close. `year` is the local year, for a
/// license's copyright line. The folder field takes the focus when nothing has it, unless
/// `cx.keyboard` is false (a credential prompt is open above it).
pub fn show(ui: &mut egui::Ui, cx: Cx, form: &mut InitForm, year: i32, msgs: &mut Vec<Msg>) -> bool {
    let mut d = Dialog::new(cx, "Start a new repository").begin(ui);
    d.body(ui, |ui| body(ui, cx, form, year, msgs));
    let blocked = form.problems().blocked;
    let out = d.finish(ui, Footer::new().cancel().primary(buttons::primary("Create repository").enabled(!blocked)));
    if out.attempted {
        form.touched.attempt();
    }
    let mut close = out.dismiss;
    if out.primary
        && let Some(req) = form.request(year)
    {
        msgs.push(Msg::Init(InitMsg::Start(req)));
        close = true;
    }
    close
}

fn body(ui: &mut egui::Ui, cx: Cx, form: &mut InitForm, year: i32, msgs: &mut Vec<Msg>) {
    let (t, focus) = (cx.t, cx.keyboard);
    folder(ui, form, focus, msgs);
    let branch = form.problems().branch;
    let r = FieldGroup::new("Initial branch")
        .optional(t.tr("dialog.field.optional"))
        .help("Leave empty for git's default branch name")
        .message(&form.touched, "branch", branch.map(Message::error))
        .show(ui, |ui| ui.add(TextField::new(&mut form.branch).id(Id::new("trellis-init-branch"))));
    form_message::track(&mut form.touched, "branch", &r);
    gitignore(ui, form);
    license(ui, form, year);
    let commit_text = format!("Commit these files as \"{INITIAL_COMMIT}\"");
    if form.can_commit(year) {
        controls::checkbox(ui, &mut form.commit, commit_text);
    } else {
        // Treated as off: nothing would be committed.
        ui.add_enabled(false, controls::Check::new(&mut false, RichText::new(commit_text).color(color::weak())));
    }
}
