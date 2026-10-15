//! The New tag dialog: a plain tag, or an annotated one whose message is required. The name is
//! checked as it is typed, and the new tag can be pushed to a remote in the same job. Also the
//! Annotate dialog, which replaces a lightweight tag with an annotated one on the same commit.

use super::theme::color;
use crate::msg::Msg;
use crate::state::tags::{AnnotateForm, TagForm, annotate_problem, push_remote, tag_problem};
use crate::tag::TagMsg;
use eframe::egui::{self, Id, Key, Modifiers, RichText};
use trellis_core::repo::Snapshot;
use trellis_core::tag_ops::TagNameProblem;

const PUSH_HINT: &str = "After creating the tag, push it to this remote in the same operation; you may be asked for credentials.";

/// The New tag dialog for `form`; returns true when Cancel closes it (`update` closes it once the
/// tag job starts). The name field keeps the keyboard focus unless the message field has it, or
/// `focus` is false (a credential prompt is open above it). `push_on` is the saved state of the
/// push checkbox, shown when the repository has remotes.
pub fn new_tag(ui: &mut egui::Ui, form: &mut TagForm, snap: Option<&Snapshot>, push_on: bool, focus: bool, msgs: &mut Vec<Msg>) -> bool {
    let mut close = false;
    let short = &form.at[..7.min(form.at.len())];
    if form.annotated {
        ui.heading(format!("New annotated tag at {short}"));
        ui.label(RichText::new("Annotated tags keep a message, the tagger and the date.").color(color::weak()));
    } else {
        ui.heading(format!("New tag at {short}"));
    }
    ui.add_space(6.0);
    let msg_id = Id::new("trellis-tag-message");
    let r =
        ui.add(egui::TextEdit::singleline(&mut form.name).id(Id::new("trellis-tag-name")).hint_text("v1.0.0").desired_width(f32::INFINITY));
    // Read before focus is requested again below, which would hide that Enter left the field.
    let enter = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
    let problem = tag_problem(snap, &form.name);
    if let Some(p) = problem.as_ref().filter(|p| **p != TagNameProblem::Empty) {
        ui.add_space(4.0);
        ui.label(RichText::new(p.message(form.name.trim())).color(color::red()));
    }
    let hint = if form.annotated { "Message (required)" } else { "Message (optional, makes an annotated tag)" };
    ui.add(egui::TextEdit::multiline(&mut form.message).id(msg_id).hint_text(hint).desired_rows(3).desired_width(f32::INFINITY));
    let valid = problem.is_none() && (!form.annotated || !form.message.trim().is_empty());
    if enter && problem.is_none() && form.annotated && form.message.trim().is_empty() {
        // The name is in; the message is still needed.
        ui.memory_mut(|m| m.request_focus(msg_id));
    } else if focus && !ui.memory(|m| m.has_focus(msg_id)) {
        r.request_focus();
    }
    let remotes = snap.filter(|s| !s.remotes.is_empty());
    if let Some(snap) = remotes {
        ui.add_space(4.0);
        push_row(ui, form, snap, push_on, msgs);
    }
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if ui.add_enabled(valid, egui::Button::new("Create tag")).clicked() || (enter && valid) {
            msgs.push(Msg::Tag(TagMsg::CreateTag {
                at: form.at.clone(),
                name: form.name.trim().into(),
                message: form.message.trim().into(),
                push: (push_on && remotes.is_some()).then(|| form.remote.clone()),
            }));
        }
        if ui.button("Cancel").clicked() {
            close = true;
        }
    });
    close
}

/// "Push to <remote>" with one remote, or a checkbox and a remote picker with several. A remote
/// that is gone (remotes reloaded) is replaced by the default one first.
fn push_row(ui: &mut egui::Ui, form: &mut TagForm, snap: &Snapshot, push_on: bool, msgs: &mut Vec<Msg>) {
    if let Some(remote) = push_remote(snap, &form.remote) {
        form.remote = remote;
    }
    let mut on = push_on;
    let changed = if let [(only, _)] = snap.remotes.as_slice() {
        ui.checkbox(&mut on, format!("Push to {only}")).on_hover_text(PUSH_HINT).changed()
    } else {
        ui.horizontal(|ui| {
            let changed = ui.checkbox(&mut on, "Push to").on_hover_text(PUSH_HINT).changed();
            egui::ComboBox::from_id_salt("tag-push-remote").selected_text(form.remote.as_str()).width(180.0).show_ui(ui, |ui| {
                for (name, _) in &snap.remotes {
                    ui.selectable_value(&mut form.remote, name.clone(), name.as_str());
                }
            });
            changed
        })
        .inner
    };
    if changed {
        msgs.push(Msg::Tag(TagMsg::SetPushNewTags(on)));
    }
}

/// The Annotate dialog for `form`; returns true when Cancel closes it (`update` closes it once the
/// job starts). The message field keeps the keyboard focus unless `focus` is false (a credential
/// prompt is open above it); Ctrl+Enter submits, plain Enter adds a line.
pub fn annotate_tag(ui: &mut egui::Ui, form: &mut AnnotateForm, snap: Option<&Snapshot>, focus: bool, msgs: &mut Vec<Msg>) -> bool {
    let mut close = false;
    let short = &form.at[..7.min(form.at.len())];
    ui.heading(format!("Annotate tag {}", form.tag));
    let what = format!("Replaces the lightweight tag with an annotated one on commit {short}. The name and commit stay the same.");
    ui.label(RichText::new(what).color(color::weak()));
    if snap.is_some_and(|s| !s.remotes.is_empty()) {
        ui.label(RichText::new("Copies already pushed to remotes stay lightweight.").color(color::weak()));
    }
    ui.add_space(6.0);
    let id = Id::new("trellis-annotate-tag-message");
    let problem = annotate_problem(snap, &form.tag, &form.at);
    // Taken before the field draws, so Ctrl+Enter never reaches it as a new line.
    let submit = problem.is_none()
        && !form.message.trim().is_empty()
        && ui.memory(|m| m.has_focus(id))
        && ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Enter));
    let r = ui.add(
        egui::TextEdit::multiline(&mut form.message)
            .id(id)
            .hint_text("Tag message (required)")
            .desired_rows(4)
            .desired_width(f32::INFINITY),
    );
    if focus && !r.has_focus() {
        r.request_focus();
    }
    if let Some(p) = &problem {
        ui.add_space(4.0);
        ui.label(RichText::new(p).color(color::red()));
    }
    let valid = problem.is_none() && !form.message.trim().is_empty();
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if ui.add_enabled(valid, egui::Button::new("Annotate")).clicked() || submit {
            msgs.push(Msg::Tag(TagMsg::Annotate { tag: form.tag.clone(), at: form.at.clone(), message: form.message.trim().into() }));
        }
        if ui.button("Cancel").clicked() {
            close = true;
        }
    });
    close
}
