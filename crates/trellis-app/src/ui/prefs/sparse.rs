//! Preferences > Sparse Checkout (repository-specific): whether it is on, the mode, the folders
//! or patterns to check out, and Turn on / Save paths, Reapply and Turn off. Every action is a
//! `Msg`.

use super::{help, primary_button, row, warn};
use crate::msg::{Msg, SparseMsg};
use crate::state::sparse::{SparsePage, summary};
use crate::ui::theme::color;
use crate::ui::theme::controls;
use eframe::egui::{self, Key, Modifiers, RichText, Ui, vec2};
use trellis_core::sparse_ops::{SparseMode, SparseStatus};

/// What the page needs from outside Preferences, cloned before the page borrows its state.
pub struct SparseContext {
    pub repo_name: Option<String>,
    /// The running job's label.
    pub job: Option<String>,
}

pub fn sparse_page(ui: &mut Ui, page: &mut SparsePage, cx: &SparseContext, msgs: &mut Vec<Msg>) {
    if cx.repo_name.is_none() {
        help(ui, "Open a repository to choose which of its folders are checked out.");
        return;
    }
    if let Some(error) = &page.error {
        warn(ui, error);
        ui.add_space(6.0);
        if ui.button("Check again").clicked() {
            msgs.push(Msg::Sparse(SparseMsg::Reload));
        }
        return;
    }
    let Some(status) = page.status.clone() else {
        ui.horizontal(|ui| {
            ui.add(egui::Spinner::new());
            ui.label(RichText::new("Reading sparse checkout…").color(color::weak()));
        });
        return;
    };
    status_row(ui, &status);
    mode_row(ui, page);
    paths_row(ui, page, cx, msgs);
    actions_row(ui, page, &status, cx, msgs);
}

fn status_row(ui: &mut Ui, status: &SparseStatus) {
    row(ui, "Status", |ui| {
        ui.label(RichText::new(summary(status)).color(color::text()));
        help(
            ui,
            "Sparse checkout leaves files outside the chosen paths out of the working tree only. They stay in commits, the history \
             and the index, and the setting belongs to this worktree alone.",
        );
        if status.fell_back {
            warn(ui, "The sparse-checkout file holds patterns that cone mode can't read, so git is matching them as patterns.");
        }
        if status.sparse_index {
            warn(
                ui,
                "This worktree uses a sparse index (index.sparse), which Trellis can't fully read. Saving the paths here turns it off.",
            );
        }
    });
}

fn mode_row(ui: &mut Ui, page: &mut SparsePage) {
    row(ui, "Mode", |ui| {
        ui.horizontal(|ui| {
            controls::radio_value(ui, &mut page.mode, SparseMode::Cone, "Folders (cone mode)");
            ui.add_space(8.0);
            controls::radio_value(ui, &mut page.mode, SparseMode::Patterns, "Patterns");
        });
        help(
            ui,
            match page.mode {
                SparseMode::Cone => "Fast: whole folders are checked out, and files at the root of the repository always are.",
                SparseMode::Patterns => {
                    "Lines like a .gitignore file name what to check out, and a line starting with ! leaves it out again. Slower in \
                     big repositories."
                }
            },
        );
    });
}

fn paths_row(ui: &mut Ui, page: &mut SparsePage, cx: &SparseContext, msgs: &mut Vec<Msg>) {
    let cone = page.mode == SparseMode::Cone;
    let (label, hint) = if cone { ("Folders", "src\ndocs/guide") } else { ("Patterns", "/*\n!/assets/\n/assets/icons/") };
    row(ui, label, |ui| {
        // Always enabled: disabling it while a job (an auto-fetch, say) runs would take the
        // focus away mid-typing. Only the buttons wait for the job.
        let edit = egui::TextEdit::multiline(&mut page.paths)
            .font(egui::TextStyle::Monospace)
            .desired_rows(8)
            .desired_width(320.0)
            .margin(vec2(8.0, 5.0))
            .hint_text(hint);
        let r = controls::field(ui, edit);
        if r.changed() {
            page.paths_error = None;
        }
        if r.has_focus() && cx.job.is_none() && ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Enter)) {
            msgs.push(Msg::Sparse(SparseMsg::Apply));
        }
        if let Some(e) = &page.paths_error {
            warn(ui, e);
        }
        help(
            ui,
            if cone {
                "One folder per line, relative to the repository root. Each folder brings everything inside it. Files at the root are \
                 always checked out, so an empty list leaves only those."
            } else {
                "One pattern per line; a later line starting with ! leaves matching files out again. An empty list isn't allowed."
            },
        );
    });
}

fn actions_row(ui: &mut Ui, page: &SparsePage, status: &SparseStatus, cx: &SparseContext, msgs: &mut Vec<Msg>) {
    let busy = cx.job.is_some();
    row(ui, "Actions", |ui| {
        ui.horizontal_wrapped(|ui| {
            let (text, tip) = if status.enabled {
                ("Save paths", "Replace the paths with this list and update the working tree.")
            } else {
                ("Turn on", "Check out only what this list names.")
            };
            if primary_button(ui, text, !busy).on_hover_text(tip).on_disabled_hover_text(tip).clicked() {
                msgs.push(Msg::Sparse(SparseMsg::Apply));
            }
            if !status.enabled {
                return;
            }
            if page.edited() && ui.button("Undo edits").clicked() {
                msgs.push(Msg::Sparse(SparseMsg::ResetForm));
            }
            let reapply = "Write or remove files again to match the current paths, for example after committing or discarding \
                           changes that kept files outside them.";
            if ui.add_enabled(!busy, egui::Button::new("Reapply")).on_hover_text(reapply).on_disabled_hover_text(reapply).clicked() {
                msgs.push(Msg::Sparse(SparseMsg::Reapply));
            }
            let off = "Check out every tracked file again and stop using sparse checkout.";
            if ui.add_enabled(!busy, egui::Button::new("Turn off")).on_hover_text(off).on_disabled_hover_text(off).clicked() {
                msgs.push(Msg::Sparse(SparseMsg::Disable));
            }
        });
        if let Some(job) = &cx.job {
            ui.label(RichText::new(format!("{job}…")).size(12.0).color(color::weak()));
        }
    });
}
