//! The Apply patch dialog (`Modal::ApplyPatch`): what the chosen file holds (its commits and
//! the files it touches), how to apply it (its changes with the working tree, index, fallback
//! and reverse options, or as commits), why it can't be applied now, then Apply. `update`
//! checks the choice again before running anything.

use super::dialog_frame::{Cx, Dialog, Footer};
use super::dialog_guides;
use super::theme::color;
use super::widgets::buttons;
use crate::msg::Msg;
use crate::patch::PatchMsg;
use crate::state::patch::apply::{ApplyForm, ApplyMode, NO_MESSAGES, NOTHING_TO_APPLY, THREE_WAY_INDEX, apply_blocked};
use crate::ui::theme::controls;
use crate::ui::theme::type_scale;
use crate::ui::widgets::form_message::{self, Message};
use eframe::egui::{self, RichText};
use trellis_core::patch_ops::OnFail;
use trellis_core::repo::Snapshot;

const BUSY: &str = "Another operation is still running.";

/// What to do with a part that doesn't apply: the choice and its label.
const ON_FAIL: [(OnFail, &str); 3] = [
    (OnFail::Refuse, "Change nothing"),
    (OnFail::ThreeWay, "Merge with a three-way fallback (conflicts to resolve)"),
    (OnFail::Reject, "Apply what fits and save the rest in .rej files"),
];

/// Draws the dialog; returns true when it should close. Apply waits while a job runs (`idle`
/// is false); on Apply the dialog stays open until `update` starts the job.
pub fn show(ui: &mut egui::Ui, cx: Cx, form: &mut ApplyForm, snap: Option<&Snapshot>, idle: bool, msgs: &mut Vec<Msg>) -> bool {
    let mut d = Dialog::new(cx, "Apply patch").subtitle(form.name.clone()).guide(dialog_guides::patch_apply(cx.t)).begin(ui);
    let blocked = d.body(ui, |ui| body(ui, form, snap));
    // The body already says why it can't be applied; the footer only says it waits.
    let why = if blocked.is_some() { None } else { (!idle).then_some(BUSY) };
    let button = buttons::primary("Apply").enabled(blocked.is_none() && idle).why(why);
    let out = d.finish(ui, Footer::new().cancel().primary(button));
    if out.primary {
        msgs.push(Msg::Patch(PatchMsg::Apply(form.request())));
    }
    out.dismiss
}

/// What the patch holds and how to apply it; returns why it can't be applied now.
fn body(ui: &mut egui::Ui, form: &mut ApplyForm, snap: Option<&Snapshot>) -> Option<String> {
    match form.summary_line() {
        Some(line) => {
            ui.label(RichText::new(line).font(type_scale::strong(ui, type_scale::BODY)));
        }
        None => form_message::form(ui, Message::warning(NOTHING_TO_APPLY)),
    }
    files(ui, form);
    ui.add_space(8.0);
    ui.label(type_scale::section_text(ui, "How to apply"));
    controls::radio_value(ui, &mut form.mode, ApplyMode::Changes, "Apply the changes to your files");
    let commits = ui.add_enabled(form.summary.commits > 0, controls::Radio::new(form.mode == ApplyMode::Commits, "Apply as commits"));
    if commits.on_disabled_hover_text(NO_MESSAGES).clicked() {
        form.mode = ApplyMode::Commits;
    }
    ui.add_space(4.0);
    ui.indent("apply-options", |ui| match form.mode {
        ApplyMode::Changes => changes_options(ui, form),
        ApplyMode::Commits => {
            controls::checkbox(ui, &mut form.am_three_way, "Three-way fallback when a patch doesn't apply");
            let on = match snap.and_then(|s| s.head_branch.as_deref()) {
                Some(b) => b.to_owned(),
                None => "detached HEAD".to_owned(),
            };
            let note = format!("Each patch becomes a commit on {on}, keeping its author and message.");
            ui.add(egui::Label::new(RichText::new(note).color(color::weak())).wrap());
        }
    });
    let blocked = snap.and_then(|s| apply_blocked(form, s));
    if let Some(why) = blocked.as_deref().filter(|w| *w != NOTHING_TO_APPLY) {
        ui.add_space(6.0);
        form_message::form(ui, Message::warning(why));
    }
    // Without a snapshot there is no repository to apply to.
    blocked.or(snap.is_none().then(|| NOTHING_TO_APPLY.to_owned()))
}

/// The files the patch touches, a few rows high.
fn files(ui: &mut egui::Ui, form: &ApplyForm) {
    if form.summary.files.is_empty() {
        return;
    }
    let row_h = super::common::widget_row_height(ui);
    egui::ScrollArea::vertical().id_salt("apply-patch-files").max_height(120.0).auto_shrink([false, true]).show_rows(
        ui,
        row_h,
        form.summary.files.len(),
        |ui, range| {
            for f in &form.summary.files[range] {
                ui.add(egui::Label::new(RichText::new(f).monospace().color(color::text())).truncate());
            }
        },
    );
}

/// Where the changes go, what to do when part doesn't fit, and reverse.
fn changes_options(ui: &mut egui::Ui, form: &mut ApplyForm) {
    let three_way = form.opts.on_fail == OnFail::ThreeWay;
    ui.label(type_scale::section_text(ui, "Where"));
    let only = ui.add_enabled(!three_way, controls::Radio::new(!form.opts.index && !three_way, "Working tree only"));
    if only.on_disabled_hover_text(THREE_WAY_INDEX).clicked() {
        form.opts.index = false;
    }
    if controls::radio(ui, form.opts.index || three_way, "Working tree and index (stage the changes)").clicked() {
        form.opts.index = true;
    }
    ui.add_space(4.0);
    ui.label(type_scale::section_text(ui, "If part of the patch doesn't apply"));
    for (choice, text) in ON_FAIL {
        controls::radio_value(ui, &mut form.opts.on_fail, choice, text);
    }
    ui.add_space(4.0);
    controls::checkbox(ui, &mut form.opts.reverse, "Reverse: take these changes out");
}
