//! The Add submodule dialog (a URL, the folder it goes in, an optional branch), the Edit
//! submodule dialog (its URL and branch) and the Remove submodule confirmation (what it deletes
//! and what would be lost), each with a side pane that explains the step. The Add dialog's text
//! checks are the pure rules in `state::submodules::add`; the folder's disk check is the
//! runner's, asked for by `update` after each edit and only read here.

use super::dialog_frame::{self, Cx, Dialog, Footer};
use super::dialog_guides;
use super::widgets::text_field::{self, TextField};
use super::widgets::{FieldGroup, buttons};
use crate::msg::Msg;
use crate::state::submodules::add::{AddForm, Disk, relative_url_hint};
use crate::state::submodules::edit::EditForm;
use crate::state::submodules::remove::RemoveForm;
use crate::submodule::SubmoduleMsg;
use crate::ui::widgets::form_message::{self, Message};
use eframe::egui::{self, Id};
use glowfly_core::repo::Snapshot;
use glowfly_core::submodule_ops::default_path;

/// The Add dialog; returns true when Cancel closes it (`update` closes it once the job starts).
/// The URL field has the keyboard focus unless another field has it, or `focus` is false (a
/// credential prompt is open above it). The button is disabled while a job runs (`idle` false).
pub fn add(ui: &mut egui::Ui, cx: Cx, form: &mut AddForm, snap: Option<&Snapshot>, idle: bool, msgs: &mut Vec<Msg>) -> bool {
    let focus = cx.keyboard;
    let (url_id, path_id, branch_id) =
        (Id::new("glowfly-submodule-url"), Id::new("glowfly-submodule-path"), Id::new("glowfly-submodule-branch"));
    let (mut enter, mut edited) = (false, false);
    let d = Dialog::new(cx, "Add submodule").guide(dialog_guides::submodule_add(cx.t)).begin(ui);

    let url_bad = form.url_problem();
    let group = match url_bad {
        // An empty URL only says what to enter.
        Some(p) if form.url.trim().is_empty() => FieldGroup::new("Repository URL").help(p),
        Some(p) => FieldGroup::new("Repository URL").message(&form.touched, "url", Some(Message::error(p))),
        None => match relative_url_hint(&form.url) {
            Some(h) => FieldGroup::new("Repository URL").help(h),
            None => FieldGroup::new("Repository URL"),
        },
    };
    let r = group
        .show(ui, |ui| ui.add(TextField::new(&mut form.url).id(url_id).hint("https://… or the path of a repository on this computer")));
    if r.changed() {
        form.touched.mark("url");
        if !form.path_edited {
            form.path = default_path(&form.url);
            // The folder follows the URL being typed.
            form.touched.mark("path");
        }
        edited = true;
    }
    // Read before focus is requested again below, which would hide that Enter left the field.
    enter |= text_field::entered(ui, &r);
    if focus && !ui.memory(|m| m.has_focus(path_id) || m.has_focus(branch_id)) {
        r.request_focus();
    }

    // Worked out before the group is drawn, which takes its lines up front.
    let path = path_lines(form, snap);
    let mut group = FieldGroup::new("Path").note(path.note).messages(&form.touched, "path", path.problems);
    if let Some(help) = path.help {
        group = group.help(help);
    }
    let r = group.show(ui, |ui| ui.add(TextField::new(&mut form.path).id(path_id).hint("A folder in this repository, e.g. libs/lib")));
    if r.changed() {
        form.path_edited = true;
        form.touched.mark("path");
        edited = true;
    }
    enter |= text_field::entered(ui, &r);

    let branch_bad = form.branch_problem();
    let r = FieldGroup::new("Branch")
        .optional(cx.t.tr("dialog.field.optional"))
        .help("Leave empty for the remote's default branch")
        .message(&form.touched, "branch", branch_bad.map(Message::error))
        .show(ui, |ui| ui.add(TextField::new(&mut form.branch).id(branch_id)));
    form_message::track(&mut form.touched, "branch", &r);
    enter |= text_field::entered(ui, &r);

    let valid = url_bad.is_none() && path.ok && branch_bad.is_none() && idle;
    let out = d.finish(ui, Footer::new().cancel().primary(buttons::primary("Add submodule").enabled(valid)).submit(enter));
    if out.attempted {
        form.touched.attempt();
    }
    if out.primary {
        msgs.push(Msg::Submodule(SubmoduleMsg::Add));
    }
    if edited {
        msgs.push(Msg::Submodule(SubmoduleMsg::AddEdited));
    }
    out.dismiss
}

/// What the Path group shows.
struct PathLines {
    /// The folder is usable.
    ok: bool,
    /// What to enter, while the field is empty.
    help: Option<String>,
    /// Shown at once (the disk check is running).
    note: Option<Message<'static>>,
    /// Shown once the field was edited or Add was tried.
    problems: Vec<Message<'static>>,
}

fn path_lines(form: &AddForm, snap: Option<&Snapshot>) -> PathLines {
    let lines = |ok, help, note, problems| PathLines { ok, help, note, problems };
    match snap.map(|s| form.rel(s)) {
        None => lines(false, None, None, Vec::new()),
        Some(Err(e)) if form.path.trim().is_empty() => lines(false, Some(e), None, Vec::new()),
        Some(Err(e)) => lines(false, None, None, vec![Message::error(e)]),
        Some(Ok(rel)) => match form.disk_state(&rel) {
            Disk::Waiting => lines(false, None, Some(Message::info("Checking…")), Vec::new()),
            Disk::Err(e) => lines(false, None, None, vec![Message::error(e.to_owned())]),
            Disk::Ok(warnings) => lines(true, None, None, warnings.iter().map(|w| Message::warning(w.clone())).collect()),
        },
    }
}

const BRANCH_HINT: &str = "The branch an update with --remote follows. Empty means the remote's default branch; \".\" \
                           follows this repository's current branch name.";

/// The Edit dialog; returns true when Cancel closes it (`update` closes it once the job starts).
/// The URL field has the keyboard focus unless the branch field has it, or `focus` is false. Save
/// is disabled while nothing changed, a field is invalid or a job runs (`idle` false).
pub fn edit(ui: &mut egui::Ui, cx: Cx, form: &mut EditForm, idle: bool, msgs: &mut Vec<Msg>) -> bool {
    let focus = cx.keyboard;
    let (url_id, branch_id) = (Id::new("glowfly-submodule-edit-url"), Id::new("glowfly-submodule-edit-branch"));
    let mut enter = false;
    let mut d = Dialog::new(cx, format!("Edit submodule {}", form.path)).guide(dialog_guides::submodule_edit(cx.t));
    if form.name != form.path {
        d = d.subtitle(format!("Named {} in .gitmodules", form.name));
    }
    let d = d.begin(ui);

    let url_bad = form.url_problem();
    let mut group = FieldGroup::new("Repository URL").message(&form.touched, "url", url_bad.map(Message::error));
    if let Some(h) = relative_url_hint(&form.url) {
        group = group.help(h);
    }
    let r = group.show(ui, |ui| ui.add(TextField::new(&mut form.url).id(url_id)));
    form_message::track(&mut form.touched, "url", &r);
    enter |= text_field::entered(ui, &r);
    if focus && !ui.memory(|m| m.has_focus(branch_id)) {
        r.request_focus();
    }

    let branch_bad = form.branch_problem();
    let r = FieldGroup::new("Branch")
        .optional(cx.t.tr("dialog.field.optional"))
        .help(BRANCH_HINT)
        .message(&form.touched, "branch", branch_bad.map(Message::error))
        .show(ui, |ui| ui.add(TextField::new(&mut form.branch).id(branch_id)));
    form_message::track(&mut form.touched, "branch", &r);
    enter |= text_field::entered(ui, &r);

    let valid = form.changed() && url_bad.is_none() && branch_bad.is_none() && idle;
    let out = d.finish(ui, Footer::new().cancel().primary(buttons::primary("Save").enabled(valid)).submit(enter));
    if out.attempted {
        form.touched.attempt();
    }
    if out.primary {
        msgs.push(Msg::Submodule(SubmoduleMsg::Edit));
    }
    out.dismiss
}

/// The Remove confirmation; returns true when Cancel closes it. The danger button is disabled
/// while a job runs (`idle` false), and, when work would be lost, until the acknowledgement is
/// ticked.
pub fn remove(ui: &mut egui::Ui, cx: Cx, form: &mut RemoveForm, idle: bool, msgs: &mut Vec<Msg>) -> bool {
    let mut d = Dialog::new(cx, format!("Remove submodule {}", form.path)).guide(dialog_guides::submodule_remove(cx.t)).begin(ui);
    d.body(ui, |ui| {
        ui.label("This deletes:");
        ui.indent("glowfly-submodule-remove-lines", |ui| {
            for line in form.deleted_lines() {
                ui.label(line);
            }
        });
        for w in form.warnings() {
            ui.add_space(6.0);
            form_message::form(ui, Message::warning(w));
        }
    });
    if let Some(label) = form.ack_label() {
        dialog_frame::ack(ui, cx.t, &mut form.acked, Some(&label));
    }
    let ok = idle && (!form.needs_ack() || form.acked);
    let why = if idle { cx.t.tr("common.ack.tip") } else { "Another operation is still running." };
    let out = d.finish(ui, Footer::new().cancel().danger(buttons::danger("Remove submodule").enabled(ok).why(Some(why))));
    if out.primary {
        msgs.push(Msg::Submodule(SubmoduleMsg::Remove));
    }
    out.dismiss
}
