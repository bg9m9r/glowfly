//! The Clone dialog's Your repositories tab: the connected account to list (a picker when there
//! are several), then its repositories, filtered as the user types; a click on a row picks it
//! and `update` fills the URL with its clone URL.

use super::clone_dialog::note;
use super::pane_status::{self, Failure};
use super::theme::color;
use super::widgets::TextField;
use crate::msg::{CloneMsg, Msg};
use crate::state::clone::CloneForm;
use crate::state::clone_service::{CloneSource, ServicePicker, account_label, visible};
use crate::state::error_words;
use crate::ui::theme::controls;
use eframe::egui::{self, Id, Layout, RichText};
use glowfly_core::error::advice::Ask;
use glowfly_core::forge::{Connection, Provider};
use glowfly_core::i18n::Lang;

/// The repositories the filter keeps (only the rows in view are laid out: a list can hold 1000).
fn rows(ui: &mut egui::Ui, p: &ServicePicker, msgs: &mut Vec<Msg>) {
    let shown = visible(p);
    if shown.is_empty() {
        note(ui, "No repository matches the filter.");
        return;
    }
    let row_height = ui.spacing().interact_size.y;
    let area = egui::ScrollArea::vertical().id_salt("glowfly-clone-repo-list").max_height(220.0).auto_shrink([false, true]);
    area.show_rows(ui, row_height, shown.len(), |ui, range| {
        for &i in &shown[range] {
            let repo = &p.list.repos[i];
            ui.horizontal(|ui| {
                if ui.selectable_label(p.picked == Some(i), repo.full_name.as_str()).clicked() {
                    msgs.push(Msg::Clone(CloneMsg::PickRepo(i)));
                }
                if repo.private {
                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new("private").color(color::weak()));
                    });
                }
            });
        }
    });
}

/// The account picker, or the one account's name.
fn account(ui: &mut egui::Ui, p: &ServicePicker, connections: &[Connection], msgs: &mut Vec<Msg>) {
    let Some(selected) = p.selected() else { return };
    if p.accounts.len() == 1 {
        note(ui, &account_label(selected, connections));
        return;
    }
    egui::ComboBox::from_id_salt("glowfly-clone-account").selected_text(account_label(selected, connections)).width(320.0).show_ui(
        ui,
        |ui| {
            for (i, a) in p.accounts.iter().enumerate() {
                if ui.selectable_label(i == p.account, account_label(a, connections)).clicked() && i != p.account {
                    msgs.push(Msg::Clone(CloneMsg::ServiceAccount(i)));
                }
            }
        },
    );
}

/// No listing service is connected: what to connect, and a link back to the URL tab. The
/// dialog's footer holds Open integrations.
pub(super) fn connect_prompt(ui: &mut egui::Ui, t: Lang, msgs: &mut Vec<Msg>) {
    ui.add(egui::Label::new(t.tr("clone.service.connect")).wrap());
    ui.add_space(6.0);
    if ui.link(t.tr("clone.service.use_url.link")).clicked() {
        msgs.push(Msg::Clone(CloneMsg::Source(CloneSource::Url)));
    }
}

/// The tab's contents; returns true when the dialog should close. The filter takes the focus
/// when nothing has it, unless `focus` is false (a credential prompt is open above).
pub fn show(ui: &mut egui::Ui, t: Lang, form: &mut CloneForm, connections: &[Connection], focus: bool, msgs: &mut Vec<Msg>) -> bool {
    let p = &mut form.service;
    if p.accounts.is_empty() {
        connect_prompt(ui, t, msgs);
        return false;
    }
    account(ui, p, connections, msgs);
    ui.add_space(4.0);
    if p.loading || p.loaded.is_none() {
        pane_status::loading_line(ui, "Loading your repositories…");
    } else if let Some(e) = &p.error {
        let provider = p.loaded.as_ref().map_or(Provider::GitHub, |a| a.provider);
        let error = error_words::hosting(t, provider, Ask::Read, e);
        let f = Failure { title: "Couldn't list your repositories", error: &error, retry: Some(Msg::Clone(CloneMsg::RetryRepos)) };
        pane_status::failed_line(ui, t, f, msgs);
    } else if p.list.repos.is_empty() {
        ui.label("No repositories found for this account.");
    } else {
        let r = ui.add(TextField::new(&mut p.filter).id(Id::new("glowfly-clone-repo-filter")).hint("Filter repositories"));
        // Not on the Enter that left it: the dialog's frame reads that as running Clone.
        if focus && !r.lost_focus() && ui.memory(|m| m.focused().is_none()) {
            r.request_focus();
        }
        ui.add_space(4.0);
        rows(ui, p, msgs);
        if p.list.truncated {
            note(ui, "Showing the first 1000 repositories. For another one, paste its URL in the URL tab.");
        }
    }
    ui.add_space(4.0);
    let mut ssh = p.ssh;
    if controls::checkbox(ui, &mut ssh, "Use the SSH URL").changed() {
        msgs.push(Msg::Clone(CloneMsg::UseSsh(ssh)));
    }
    if p.picked_repo(&form.url).is_some() {
        note(ui, &format!("Clones {}", form.url));
    }
    false
}
