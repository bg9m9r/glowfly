//! The top of the rebase banner on the WIP row: "<name> in progress · step N of M", the
//! stopped commit (its short SHA a link to it in the graph, the summary, and why it stopped), a
//! failed command, and the autostash note; plus the "Edit remaining steps…" button. The
//! conflicted-files row and the other buttons are drawn by `details_banner::op_banner` (for
//! `details_wip::wip_panel`).

use crate::msg::{Msg, RebaseMsg, RepoMsg};
use crate::state::rebase_banner::{EDIT_STEPS, SELECT_COMMIT, STASH_NOTE, lines};
use crate::ui::theme::color;
use crate::ui::theme::type_scale;
use crate::ui::widgets::buttons;
use eframe::egui::{self, RichText};
use git2::RepositoryState;
use trellis_core::rebase::RebaseProgress;

pub fn progress(ui: &mut egui::Ui, p: &RebaseProgress, name: &str, conflicts_left: bool, msgs: &mut Vec<Msg>) {
    let text = lines(p, name, conflicts_left);
    ui.label(RichText::new(text.title).font(type_scale::strong(ui, type_scale::BODY)));
    if let Some(stop) = &text.stop {
        // Word by word, so the line wraps between words like plain text.
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 2.0);
            words(ui, &format!("{} ", stop.before));
            let link = egui::Link::new(RichText::new(&stop.sha).monospace());
            if ui.add(link).on_hover_text(SELECT_COMMIT).clicked() {
                msgs.push(Msg::Repo(RepoMsg::JumpTo(stop.oid)));
            }
            if !stop.summary.is_empty() {
                words(ui, &format!(" {}", stop.summary));
            }
            // `after` brings its own leading space or punctuation.
            words(ui, stop.after);
        });
    }
    if let Some(command) = &text.command {
        ui.add(egui::Label::new(RichText::new(command).monospace()).wrap());
    }
    if text.stash_note {
        ui.label(RichText::new(STASH_NOTE).color(color::weak()));
    }
}

/// `text` as one label per word (each with the spaces after it).
fn words(ui: &mut egui::Ui, text: &str) {
    for word in text.split_inclusive(' ') {
        ui.add(egui::Label::new(word).wrap_mode(egui::TextWrapMode::Extend));
    }
}

/// "Edit remaining steps…", only while an interactive rebase is in progress.
pub fn edit_steps_button(ui: &mut egui::Ui, state: RepositoryState, msgs: &mut Vec<Msg>) {
    if state == RepositoryState::RebaseInteractive && ui.add(buttons::secondary(EDIT_STEPS)).clicked() {
        msgs.push(Msg::Rebase(RebaseMsg::OpenRemaining));
    }
}
