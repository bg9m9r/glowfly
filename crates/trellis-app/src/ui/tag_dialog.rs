//! The New tag dialog: a plain tag, or an annotated one whose message is required. The name is
//! checked as it is typed, and the new tag can be pushed to a remote in the same job. Also the
//! Annotate dialog, which replaces a lightweight tag with an annotated one on the same commit.

use super::dialog_frame::{Cx, Dialog, Footer};
use super::theme::color;
use super::widgets::buttons;
use crate::msg::Msg;
use crate::state::tags::{AnnotateForm, TagForm, annotate_problem, push_remote, tag_problem};
use crate::tag::TagMsg;
use crate::ui::theme::controls;
use eframe::egui::{self, Id, Key, RichText};
use trellis_core::branch_ops::ref_name_warning;
use trellis_core::names::blocks_here;
use trellis_core::repo::Snapshot;
use trellis_core::tag_ops::TagNameProblem;

const PUSH_HINT: &str = "After creating the tag, push it to this remote in the same operation; you may be asked for credentials.";

/// The New tag dialog for `form`; returns true when Cancel closes it (`update` closes it once the
/// tag job starts). The name field keeps the keyboard focus unless the message field has it, or
/// `cx.keyboard` is false (a credential prompt is open above it). `push_on` is the saved state of
/// the push checkbox, shown when the repository has remotes. Ctrl/Cmd+Enter in the message
/// creates the tag.
pub fn new_tag(ui: &mut egui::Ui, cx: Cx, form: &mut TagForm, snap: Option<&Snapshot>, push_on: bool, msgs: &mut Vec<Msg>) -> bool {
    let focus = cx.keyboard;
    let short = &form.at[..7.min(form.at.len())];
    let d = if form.annotated {
        Dialog::new(cx, format!("New annotated tag at {short}")).subtitle("Annotated tags keep a message, the tagger and the date.")
    } else {
        Dialog::new(cx, format!("New tag at {short}"))
    };
    let d = d.begin(ui);
    let msg_id = Id::new("trellis-tag-message");
    let r = controls::field(
        ui,
        egui::TextEdit::singleline(&mut form.name).id(Id::new("trellis-tag-name")).hint_text("v1.0.0").desired_width(f32::INFINITY),
    );
    // Read before focus is requested again below, which would hide that Enter left the field.
    let enter = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
    let problem = tag_problem(snap, &form.name);
    if let Some(p) = problem.as_ref().filter(|p| **p != TagNameProblem::Empty) {
        ui.add_space(4.0);
        ui.label(RichText::new(p.message(form.name.trim())).color(color::red()));
    } else if problem.is_none()
        && let Some(w) = ref_name_warning(form.name.trim(), blocks_here())
    {
        // Allowed here, but Windows can't store it as a ref file.
        ui.add_space(4.0);
        ui.label(RichText::new(w).color(color::amber()));
    }
    let hint = if form.annotated { "Message (required)" } else { "Message (optional, makes an annotated tag)" };
    controls::field(
        ui,
        egui::TextEdit::multiline(&mut form.message).id(msg_id).hint_text(hint).desired_rows(3).desired_width(f32::INFINITY),
    );
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
    let out = d.finish(ui, Footer::new().cancel().primary(buttons::primary("Create tag").enabled(valid)).submit(enter));
    if out.primary {
        msgs.push(Msg::Tag(TagMsg::CreateTag {
            at: form.at.clone(),
            name: form.name.trim().into(),
            message: form.message.trim().into(),
            push: (push_on && remotes.is_some()).then(|| form.remote.clone()),
        }));
    }
    out.dismiss
}

/// "Push to <remote>" with one remote, or a checkbox and a remote picker with several. A remote
/// that is gone (remotes reloaded) is replaced by the default one first.
fn push_row(ui: &mut egui::Ui, form: &mut TagForm, snap: &Snapshot, push_on: bool, msgs: &mut Vec<Msg>) {
    if let Some(remote) = push_remote(snap, &form.remote) {
        form.remote = remote;
    }
    let mut on = push_on;
    let changed = if let [(only, _)] = snap.remotes.as_slice() {
        controls::checkbox(ui, &mut on, format!("Push to {only}")).on_hover_text(PUSH_HINT).changed()
    } else {
        ui.horizontal(|ui| {
            let changed = controls::checkbox(ui, &mut on, "Push to").on_hover_text(PUSH_HINT).changed();
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
/// job starts). The message field keeps the keyboard focus unless `cx.keyboard` is false (a
/// credential prompt is open above it); Ctrl/Cmd+Enter submits (the frame), plain Enter adds a
/// line.
pub fn annotate_tag(ui: &mut egui::Ui, cx: Cx, form: &mut AnnotateForm, snap: Option<&Snapshot>, msgs: &mut Vec<Msg>) -> bool {
    let short = &form.at[..7.min(form.at.len())];
    let d = Dialog::new(cx, format!("Annotate tag {}", form.tag)).begin(ui);
    let what = format!("Replaces the lightweight tag with an annotated one on commit {short}. The name and commit stay the same.");
    ui.label(RichText::new(what).color(color::weak()));
    if snap.is_some_and(|s| !s.remotes.is_empty()) {
        ui.label(RichText::new("Copies already pushed to remotes stay lightweight.").color(color::weak()));
    }
    ui.add_space(6.0);
    let id = Id::new("trellis-annotate-tag-message");
    let problem = annotate_problem(snap, &form.tag, &form.at);
    let r = controls::field(
        ui,
        egui::TextEdit::multiline(&mut form.message)
            .id(id)
            .hint_text("Tag message (required)")
            .desired_rows(4)
            .desired_width(f32::INFINITY),
    );
    if cx.keyboard && !r.has_focus() {
        r.request_focus();
    }
    if let Some(p) = &problem {
        ui.add_space(4.0);
        ui.label(RichText::new(p).color(color::red()));
    }
    let valid = problem.is_none() && !form.message.trim().is_empty();
    let out = d.finish(ui, Footer::new().cancel().primary(buttons::primary("Annotate").enabled(valid)));
    if out.primary {
        msgs.push(Msg::Tag(TagMsg::Annotate { tag: form.tag.clone(), at: form.at.clone(), message: form.message.trim().into() }));
    }
    out.dismiss
}
