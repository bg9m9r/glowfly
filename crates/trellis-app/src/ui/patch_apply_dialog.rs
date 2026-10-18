//! The Apply patch dialog (`Modal::ApplyPatch`): what the chosen file holds (its commits and
//! the files it touches), how to apply it (its changes with the working tree, index, fallback
//! and reverse options, or as commits), why it can't be applied now, then Apply. `update`
//! checks the choice again before running anything.

use super::theme::color;
use crate::msg::Msg;
use crate::patch::PatchMsg;
use crate::state::patch::apply::{ApplyForm, ApplyMode, NO_MESSAGES, NOTHING_TO_APPLY, THREE_WAY_INDEX, apply_blocked};
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
pub fn show(ui: &mut egui::Ui, form: &mut ApplyForm, snap: Option<&Snapshot>, idle: bool, msgs: &mut Vec<Msg>) -> bool {
    let mut close = false;
    ui.heading("Apply patch");
    ui.label(RichText::new(&form.name).color(color::weak()));
    ui.add_space(6.0);
    match form.summary_line() {
        Some(line) => ui.label(RichText::new(line).strong()),
        None => ui.label(RichText::new(NOTHING_TO_APPLY).color(color::red())),
    };
    files(ui, form);
    ui.add_space(8.0);
    ui.label(RichText::new("How to apply").strong());
    ui.radio_value(&mut form.mode, ApplyMode::Changes, "Apply the changes to your files");
    let commits = ui.add_enabled(form.summary.commits > 0, egui::RadioButton::new(form.mode == ApplyMode::Commits, "Apply as commits"));
    if commits.on_disabled_hover_text(NO_MESSAGES).clicked() {
        form.mode = ApplyMode::Commits;
    }
    ui.add_space(4.0);
    ui.indent("apply-options", |ui| match form.mode {
        ApplyMode::Changes => changes_options(ui, form),
        ApplyMode::Commits => {
            ui.checkbox(&mut form.am_three_way, "Three-way fallback when a patch doesn't apply");
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
        ui.add_space(4.0);
        ui.add(egui::Label::new(RichText::new(why).color(color::red())).wrap());
    }
    // Without a snapshot there is no repository to apply to.
    let blocked = blocked.or(snap.is_none().then(|| NOTHING_TO_APPLY.to_owned()));
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        let mut button = ui.add_enabled(blocked.is_none() && idle, egui::Button::new("Apply"));
        if let Some(why) = blocked.as_deref().or((!idle).then_some(BUSY)) {
            button = button.on_disabled_hover_text(why);
        }
        if button.clicked() {
            msgs.push(Msg::Patch(PatchMsg::Apply(form.request())));
        }
        if ui.button("Cancel").clicked() {
            close = true;
        }
    });
    close
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
    ui.label(RichText::new("Where").strong());
    let only = ui.add_enabled(!three_way, egui::RadioButton::new(!form.opts.index && !three_way, "Working tree only"));
    if only.on_disabled_hover_text(THREE_WAY_INDEX).clicked() {
        form.opts.index = false;
    }
    if ui.radio(form.opts.index || three_way, "Working tree and index (stage the changes)").clicked() {
        form.opts.index = true;
    }
    ui.add_space(4.0);
    ui.label(RichText::new("If part of the patch doesn't apply").strong());
    for (choice, text) in ON_FAIL {
        ui.radio_value(&mut form.opts.on_fail, choice, text);
    }
    ui.add_space(4.0);
    ui.checkbox(&mut form.opts.reverse, "Reverse: take these changes out");
}
