//! Preferences > SSH: the inline Add to service form under a key's row. The view edits only the
//! Title buffer; Add key, Cancel and the fallback button are `Msg`s.

use super::{help, primary_button, text_field, warn};
use crate::msg::Msg;
use crate::ssh::SshMsg;
use crate::state::ssh_service::{UploadForm, UploadState, already_note};
use crate::ui::theme::color;
use crate::ui::theme::type_scale;
use crate::ui::widgets::buttons;
use eframe::egui::{self, CornerRadius, RichText, Ui};
use trellis_core::forge::title_problem;
use trellis_core::i18n::Lang;

pub(super) fn upload_form(ui: &mut Ui, t: Lang, form: &mut UploadForm, file: &str, msgs: &mut Vec<Msg>) {
    egui::Frame::new().fill(color::panel()).corner_radius(CornerRadius::same(6)).inner_margin(egui::Margin::symmetric(12, 10)).show(
        ui,
        |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 6.0;
            let heading = format!("Add {file} to {} ({})", form.service(), form.account.host);
            ui.label(type_scale::section_text(ui, heading));
            ui.horizontal(|ui| {
                ui.label(RichText::new("Title").size(type_scale::SMALL).color(color::text()));
                let hint = form.default_title.clone();
                text_field(ui, &mut form.title, &hint);
            });
            if let Some(problem) = title_problem(&form.title) {
                warn(ui, problem);
            }
            ui.horizontal(|ui| {
                if primary_button(ui, "Add key", form.send_blocked().is_none()).clicked() {
                    msgs.push(Msg::Ssh(SshMsg::SendToService));
                }
                if ui.add(buttons::secondary(t.tr("common.cancel.button"))).clicked() {
                    msgs.push(Msg::Ssh(SshMsg::CancelService));
                }
                if form.state == UploadState::Sending {
                    ui.add(egui::Spinner::new().size(14.0));
                }
            });
            match &form.state {
                UploadState::Editing | UploadState::Sending => {}
                UploadState::AlreadyThere(title) => help(ui, &already_note(form.service(), title)),
                UploadState::Failed { message, .. } => {
                    warn(ui, message);
                    if ui.add(buttons::secondary("Copy key and open settings")).clicked() {
                        msgs.push(Msg::Ssh(SshMsg::OpenKeySettings { key: form.key.clone(), account: form.account.clone() }));
                    }
                }
            }
        },
    );
    ui.add_space(6.0);
}
