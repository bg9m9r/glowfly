//! The rebase's lines under the WIP panel's step heading (`op_steps`): the stopped commit (its
//! short SHA a link to it in the graph, the summary, and why it stopped), a failed command, and
//! the autostash note; plus the "Edit remaining steps…" button of step 3.

use crate::msg::{Msg, RebaseMsg, RepoMsg};
use crate::state::rebase_banner::lines;
use crate::ui::theme::color;
use crate::ui::widgets::buttons;
use eframe::egui::{self, RichText};
use git2::RepositoryState;
use glowfly_core::i18n::Lang;
use glowfly_core::rebase::RebaseProgress;

/// The lines under the title: where the rebase stopped (the SHA a link to it), a failed
/// command, and the autostash note; also under the WIP panel's step heading (`op_steps`).
pub fn details(ui: &mut egui::Ui, t: Lang, p: &RebaseProgress, conflicts_left: bool, msgs: &mut Vec<Msg>) {
    let text = lines(t, p, conflicts_left);
    if let Some(stop) = &text.stop {
        // Word by word, so the line wraps between words like plain text.
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 2.0);
            words(ui, &format!("{} ", stop.before));
            let link = egui::Link::new(RichText::new(&stop.sha).monospace());
            if ui.add(link).on_hover_text(t.tr("details.banner.select_commit.tip")).clicked() {
                msgs.push(Msg::Repo(RepoMsg::JumpTo(stop.oid)));
            }
            if !stop.summary.is_empty() {
                words(ui, &format!(" {}", stop.summary));
            }
            // `after` brings its own leading space or punctuation.
            words(ui, &stop.after);
        });
    }
    if let Some(command) = &text.command {
        ui.add(egui::Label::new(RichText::new(command).monospace()).wrap());
    }
    if text.stash_note {
        ui.label(RichText::new(t.tr("details.banner.stash_note")).color(color::weak()));
    }
}

/// `text` as one label per word (each with the spaces after it).
fn words(ui: &mut egui::Ui, text: &str) {
    for word in text.split_inclusive(' ') {
        ui.add(egui::Label::new(word).wrap_mode(egui::TextWrapMode::Extend));
    }
}

/// "Edit remaining steps…", only while an interactive rebase is in progress.
pub fn edit_steps_button(ui: &mut egui::Ui, t: Lang, state: RepositoryState, msgs: &mut Vec<Msg>) {
    if state == RepositoryState::RebaseInteractive && ui.add(buttons::secondary(t.tr("details.banner.edit_steps.button"))).clicked() {
        msgs.push(Msg::Rebase(RebaseMsg::OpenRemaining));
    }
}
