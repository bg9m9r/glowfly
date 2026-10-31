//! Preferences > Sparse checkout (repository-specific): whether it is on, the mode, the folders
//! or patterns to check out, and Turn on while it is off, or Reapply and Turn off while it is on
//! (edits to the paths are saved by the Save / Revert bar). While git is held in the repository
//! (trust) and sparse checkout is on, its paths aren't read: the page says so instead of the
//! form. Every action is a `Msg`.

use super::{help, primary_button, row, warn};
use crate::msg::{Msg, SparseMsg, TrustMsg};
use crate::state::blocked::BUSY;
use crate::state::sparse::{SparsePage, summary};
use crate::ui::theme::color;
use crate::ui::theme::controls;
use crate::ui::theme::type_scale;
use crate::ui::widgets::{self, buttons};
use eframe::egui::{self, Key, Modifiers, RichText, Ui, vec2};
use glowfly_core::sparse_ops::{SparseMode, SparseStatus};
use std::path::PathBuf;

/// What the page needs from outside Preferences, cloned before the page borrows its state.
pub struct SparseContext {
    pub repo_name: Option<String>,
    /// The running job's label.
    pub job: Option<String>,
    /// Why Reapply and Turn off wait while the form has unsaved edits.
    pub save_first: &'static str,
    /// The active repository's real path while git is held there, for Review trust….
    pub held: Option<PathBuf>,
}

pub fn sparse_page(ui: &mut Ui, page: &mut SparsePage, cx: &SparseContext, msgs: &mut Vec<Msg>) {
    if cx.repo_name.is_none() {
        help(ui, "Open a repository to choose which of its folders are checked out.");
        return;
    }
    if let Some(error) = &page.error {
        warn(ui, error);
        ui.add_space(6.0);
        if ui.add(buttons::secondary("Check again")).clicked() {
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
    if status.enabled && (status.held || cx.held.is_some()) {
        held_row(ui, cx, msgs);
        return;
    }
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
                "This worktree uses a sparse index (index.sparse), which Glowfly can't fully read. Saving the paths here turns it off.",
            );
        }
    });
}

/// Sparse checkout is on and its paths weren't read: git is held in the repository. Without
/// `cx.held` the status is from before the repository was trusted, and a new one is on its way.
fn held_row(ui: &mut Ui, cx: &SparseContext, msgs: &mut Vec<Msg>) {
    row(ui, "Paths", |ui| {
        help(
            ui,
            "Glowfly lists the sparse paths with git, and git would run the programs this repository's own settings and hooks name. \
             Trust it to see and change them.",
        );
        if let Some(real) = &cx.held {
            ui.add_space(4.0);
            if ui.add(buttons::secondary("Review trust…")).clicked() {
                msgs.push(Msg::Trust(TrustMsg::Ask { real: real.clone() }));
            }
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
            if !status.enabled {
                let tip = "Check out only what this list names.";
                if primary_button(ui, "Turn on", !busy, busy.then_some(BUSY)).on_hover_text(tip).clicked() {
                    msgs.push(Msg::Sparse(SparseMsg::Apply));
                }
                return;
            }
            // Edits wait for the bar: these act on the saved paths.
            let edited = page.edited().then_some(cx.save_first);
            let enabled = !busy && edited.is_none();
            let reapply = "Write or remove files again to match the current paths, for example after committing or discarding                            changes that kept files outside them.";
            let r = ui.add(buttons::secondary("Reapply").enabled(enabled).why(edited));
            if r.on_hover_text(reapply).clicked() {
                msgs.push(Msg::Sparse(SparseMsg::Reapply));
            }
            let off = "Check out every tracked file again and stop using sparse checkout.";
            if ui.add(buttons::secondary("Turn off").enabled(enabled).why(edited)).on_hover_text(off).clicked() {
                msgs.push(Msg::Sparse(SparseMsg::Disable));
            }
            if let Some(why) = edited {
                widgets::reason::beside(ui, why, 280.0);
            }
        });
        if let Some(job) = &cx.job {
            ui.label(RichText::new(format!("{job}…")).size(type_scale::SMALL).color(color::weak()));
        }
    });
}
