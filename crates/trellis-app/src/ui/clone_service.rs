//! The Clone dialog's Your repositories tab: the connected account to list (a picker when there
//! are several), then its repositories, filtered as the user types; a click on a row picks it
//! and `update` fills the URL with its clone URL.

use super::clone_dialog::note;
use super::pane_status::{self, Failure};
use super::theme::color;
use super::widgets::buttons;
use crate::msg::{CloneMsg, Msg, PrefsMsg};
use crate::state::PrefPage;
use crate::state::clone::CloneForm;
use crate::state::clone_service::{ServicePicker, account_label, visible};
use crate::state::forks::error_text;
use crate::ui::theme::controls;
use eframe::egui::{self, Id, Layout, RichText};
use trellis_core::forge::Connection;
use trellis_core::i18n::Lang;

/// The repositories the filter keeps (only the rows in view are laid out: a list can hold 1000).
fn rows(ui: &mut egui::Ui, p: &ServicePicker, msgs: &mut Vec<Msg>) {
    let shown = visible(p);
    if shown.is_empty() {
        note(ui, "No repository matches the filter.");
        return;
    }
    let row_height = ui.spacing().interact_size.y;
    let area = egui::ScrollArea::vertical().id_salt("trellis-clone-repo-list").max_height(220.0).auto_shrink([false, true]);
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
    egui::ComboBox::from_id_salt("trellis-clone-account").selected_text(account_label(selected, connections)).width(320.0).show_ui(
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

/// The tab's contents; returns true when Open Integrations closes the dialog. The filter takes
/// the focus when nothing has it, unless `focus` is false (a credential prompt is open above).
pub fn show(ui: &mut egui::Ui, t: Lang, form: &mut CloneForm, connections: &[Connection], focus: bool, msgs: &mut Vec<Msg>) -> bool {
    let p = &mut form.service;
    if p.accounts.is_empty() {
        ui.label("Connect GitHub, GitLab or Bitbucket in Preferences > Integrations to pick from your repositories.");
        if ui.add(buttons::secondary("Open Integrations").small()).clicked() {
            msgs.push(Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Integrations)));
            return true;
        }
        return false;
    }
    account(ui, p, connections, msgs);
    ui.add_space(4.0);
    if p.loading || p.loaded.is_none() {
        pane_status::loading_line(ui, "Loading your repositories…");
    } else if let Some(e) = &p.error {
        let error = error_text(e);
        let f = Failure { title: "Couldn't list your repositories", error: &error, retry: Some(Msg::Clone(CloneMsg::RetryRepos)) };
        pane_status::failed_line(ui, t, f, msgs);
    } else if p.list.repos.is_empty() {
        ui.label("No repositories found for this account.");
    } else {
        let r = controls::field(
            ui,
            egui::TextEdit::singleline(&mut p.filter)
                .id(Id::new("trellis-clone-repo-filter"))
                .hint_text("Filter repositories")
                .desired_width(f32::INFINITY),
        );
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
