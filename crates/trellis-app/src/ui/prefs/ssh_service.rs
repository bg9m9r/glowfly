//! Preferences > SSH: the inline Add to service form under a key's card. The view edits only
//! the Title buffer; Add key, Cancel and the fallback button are `Msg`s.

use super::{GAP, LABEL_W, ROW_W, fit, help, primary_button, text_field, warn};
use crate::msg::Msg;
use crate::ssh::SshMsg;
use crate::state::ssh_service::{UploadForm, UploadState, already_note};
use crate::ui::theme::color;
use crate::ui::theme::type_scale;
use crate::ui::widgets::buttons;
use eframe::egui::{self, CornerRadius, RichText, Stroke, Ui};
use trellis_core::forge::title_problem;
use trellis_core::i18n::Lang;

pub(super) fn upload_form(ui: &mut Ui, t: Lang, form: &mut UploadForm, file: &str, msgs: &mut Vec<Msg>) {
    // As wide as the cards, in their style.
    let w = fit(ui, LABEL_W + GAP + ROW_W);
    egui::Frame::new()
        .fill(color::panel())
        .stroke(Stroke::new(1.0_f32, color::border()))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(egui::Margin::symmetric(14, 12))
        .show(ui, |ui| {
            ui.set_width(w - 2.0 * 15.0);
            ui.spacing_mut().item_spacing.y = 6.0;
            let heading =
                t.trf("prefs.ssh.service.heading", &[("file", &file), ("service", &form.service()), ("host", &form.account.host)]);
            ui.label(type_scale::section_text(ui, heading));
            ui.horizontal(|ui| {
                ui.label(RichText::new(t.tr("prefs.ssh.service.title.label")).size(type_scale::SMALL).color(color::text()));
                let hint = form.default_title.clone();
                text_field(ui, &mut form.title, &hint);
            });
            if let Some(problem) = title_problem(&form.title) {
                warn(ui, problem);
            }
            ui.horizontal(|ui| {
                // Not the title's problem, warned about above.
                let blocked = form.send_blocked();
                let why = blocked.filter(|b| title_problem(&form.title) != Some(*b));
                if primary_button(ui, t.tr("prefs.ssh.service.add.button"), blocked.is_none(), why).clicked() {
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
                    if ui.add(buttons::secondary(t.tr("prefs.ssh.service.fallback.button"))).clicked() {
                        msgs.push(Msg::Ssh(SshMsg::OpenKeySettings { key: form.key.clone(), account: form.account.clone() }));
                    }
                }
            }
        });
    ui.add_space(8.0);
}
