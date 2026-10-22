//! The create dialog's From / Into rows, shown with two or more PR repositories (pull requests
//! across forks): each picks a repository ("owner/repo (remote)"); From shows the source
//! branch, Into holds the target branch picker. A repository pick is sent as `PickPrRepo`, so
//! `update` renames the source and reloads the targets; the target branch is edited in place.

use super::create_pr_dialog::{PrForm, target_combo};
use crate::msg::{ForgeMsg, Msg};
use crate::state::ForgeState;
use crate::state::pr_repos::{self, RepoSide};
use crate::ui::theme::type_scale;
use eframe::egui::{self, RichText};

/// The From and Into rows for remotes `from` and `into`.
pub(super) fn rows(ui: &mut egui::Ui, f: &mut PrForm, fs: &ForgeState, (from, into): (&str, &str), msgs: &mut Vec<Msg>) {
    // One cell per picker: nested horizontal layouts would centre the second picker lower.
    egui::Grid::new("pr-repos").num_columns(3).spacing([12.0, 8.0]).show(ui, |ui| {
        let label = ui.label("From");
        repo_combo(ui, "pr-from", label.id, fs, from, RepoSide::From, msgs);
        ui.label(RichText::new(f.source.as_str()).font(type_scale::strong(ui, type_scale::BODY)));
        ui.end_row();
        let label = ui.label("Into");
        repo_combo(ui, "pr-into", label.id, fs, into, RepoSide::Into, msgs);
        target_combo(ui, f);
        ui.end_row();
    });
}

/// A repository picker named by the row's label (`labelled`) for screen readers.
fn repo_combo(ui: &mut egui::Ui, id: &str, labelled: egui::Id, fs: &ForgeState, current: &str, side: RepoSide, msgs: &mut Vec<Msg>) {
    let selected = pr_repos::repo(fs, current).map(pr_repos::label).unwrap_or_else(|| current.to_owned());
    let combo = egui::ComboBox::from_id_salt(id).selected_text(selected).width(220.0).show_ui(ui, |ui| {
        for r in &fs.pr_repos {
            let picked = r.remote == current;
            if ui.selectable_label(picked, pr_repos::label(r)).clicked() && !picked {
                msgs.push(Msg::Forge(ForgeMsg::PickPrRepo { side, remote: r.remote.clone() }));
            }
        }
    });
    combo.response.labelled_by(labelled);
}
