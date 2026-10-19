//! The New file and Rename file dialogs. The path is checked live with the pure
//! `check_new_path` and against the snapshot; `update` checks it again and core checks the disk.
//! Submitting leaves the dialog open: `update` closes it on success or puts core's error in the
//! form.

use super::dialog_frame::{Cx, Dialog, Footer, Open};
use super::theme::color;
use super::widgets::buttons;
use crate::msg::Msg;
use crate::msg::WorkFileMsg;
use crate::state::files::{PathForm, listed, tracked};
use crate::ui::theme::controls;
use eframe::egui::{self, Key, RichText};
use trellis_core::repo::{PathProblem, Snapshot, check_new_path, new_path_warning};

/// The New file dialog; returns true when it should close (Cancel). Its field keeps the keyboard
/// focus unless `cx.keyboard` is false (a credential prompt is open above it).
pub fn new_file(ui: &mut egui::Ui, cx: Cx, form: &mut PathForm, snap: Option<&Snapshot>, msgs: &mut Vec<Msg>) -> bool {
    let d = Dialog::new(cx, "New file").subtitle("A path inside the repository; missing folders are created.").begin(ui);
    let (submit, valid) = path_field(ui, form, snap, None, cx.keyboard);
    footer(ui, d, form, "Create", submit, valid, || Msg::WorkFile(WorkFileMsg::CreateFile(form.text.clone())), msgs)
}

const STAGE_HINT: &str = "Moves the file in the index too, so the old and new paths show as one staged rename. The file's other staged and unstaged changes are kept.";

/// The rename dialog for the working-tree file `old`; returns true when it should close. A
/// tracked file gets the "Stage the rename" checkbox, starting from `stage_renames`; toggling it
/// sends a `Msg` (update saves the setting).
pub fn rename_file(
    ui: &mut egui::Ui,
    cx: Cx,
    old: &str,
    form: &mut PathForm,
    snap: Option<&Snapshot>,
    stage_renames: bool,
    msgs: &mut Vec<Msg>,
) -> bool {
    let d = Dialog::new(cx, format!("Rename {old}")).begin(ui);
    let (submit, valid) = path_field(ui, form, snap, Some(old), cx.keyboard);
    let tracked = snap.is_some_and(|s| tracked(s, old));
    if tracked {
        ui.add_space(4.0);
        let mut on = stage_renames;
        if controls::checkbox(ui, &mut on, "Stage the rename (git mv)").on_hover_text(STAGE_HINT).changed() {
            msgs.push(Msg::WorkFile(WorkFileMsg::SetRenameStages(on)));
        }
        let note = if stage_renames {
            "The rename is staged; changes to the file stay as they are."
        } else {
            "Git will show the old path deleted and the new one untracked until you stage both."
        };
        ui.label(RichText::new(note).color(color::weak()));
    }
    let msg = || Msg::WorkFile(WorkFileMsg::RenameFile { from: old.to_owned(), to: form.text.clone(), stage: tracked && stage_renames });
    footer(ui, d, form, "Rename", submit, valid, msg, msgs)
}

/// The path field with its problem line and normalized preview. Returns (Enter pressed in the
/// field, the path can be submitted).
fn path_field(ui: &mut egui::Ui, form: &mut PathForm, snap: Option<&Snapshot>, old: Option<&str>, focus: bool) -> (bool, bool) {
    let r = controls::field(ui, egui::TextEdit::singleline(&mut form.text).hint_text("folder/name.ext").desired_width(f32::INFINITY));
    if focus {
        r.request_focus();
    }
    if r.gained_focus() {
        // The field's id is reused between openings, so a cursor left from the last dialog would
        // put typing in the middle of the prefill.
        let mut state = egui::text_edit::TextEditState::load(ui.ctx(), r.id).unwrap_or_default();
        let end = egui::text::CCursor::new(form.text.chars().count());
        state.cursor.set_char_range(Some(egui::text::CCursorRange::one(end)));
        state.store(ui.ctx(), r.id);
    }
    let checked = check_new_path(&form.text);
    let unchanged = old.is_some_and(|o| checked.as_deref() == Ok(o));
    let problem = if unchanged { None } else { problem(&form.text, snap, old).or_else(|| form.current_error().map(str::to_owned)) };
    if let Some(p) = &problem {
        error_line(ui, p);
    } else if let Some(w) = checked.as_deref().ok().filter(|_| !unchanged).and_then(new_path_warning) {
        // Allowed here, but the repository won't check out on Windows.
        ui.add_space(4.0);
        ui.label(RichText::new(w).color(color::amber()));
    }
    if let Ok(path) = &checked
        && path != form.text.trim()
    {
        ui.add_space(2.0);
        ui.label(RichText::new(format!("Becomes {path}")).color(color::weak()));
    }
    let valid = checked.is_ok() && !unchanged && problem.is_none();
    (r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)), valid)
}

/// What is wrong with `text` as a new path, from the text and the snapshot alone. Empty input
/// or a bare folder ("docs/", often a prefill still to be completed) has no message; the button
/// is just off.
fn problem(text: &str, snap: Option<&Snapshot>, old: Option<&str>) -> Option<String> {
    match check_new_path(text) {
        Err(PathProblem::Empty | PathProblem::Folder) => None,
        Err(p) => Some(p.message()),
        Ok(path) => snap.filter(|s| listed(s, &path, old)).map(|_| format!("{path} already exists.")),
    }
}

fn error_line(ui: &mut egui::Ui, text: &str) {
    ui.add_space(4.0);
    ui.label(RichText::new(text).color(color::red()));
}

/// A spinner while a request runs, then Cancel and the submit button. Returns true for Cancel.
#[allow(clippy::too_many_arguments)]
fn footer(
    ui: &mut egui::Ui,
    d: Open,
    form: &PathForm,
    label: &str,
    submit: bool,
    valid: bool,
    msg: impl FnOnce() -> Msg,
    msgs: &mut Vec<Msg>,
) -> bool {
    if form.busy {
        ui.add(egui::Spinner::new().size(12.0));
    }
    let out = d.finish(ui, Footer::new().cancel().primary(buttons::primary(label).enabled(valid && !form.busy)).submit(submit));
    if out.primary {
        msgs.push(msg());
    }
    out.dismiss
}
