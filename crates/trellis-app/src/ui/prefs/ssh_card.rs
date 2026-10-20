//! Preferences > SSH: one key pair's card. Its name, kind and marks (`state::ssh_marks`) with
//! Copy public key at the right, then its comment (and where it is, for a key outside the .ssh
//! folder), its fingerprint, and its actions: Add to agent, Add to service… and, for a key added
//! with Use existing, Remove from list. Every action is a `Msg`.

use super::ssh::SshContext;
use super::{GAP, LABEL_W, ROW_W, fit};
use crate::msg::Msg;
use crate::ssh::SshMsg;
use crate::state::ssh::SshPage;
use crate::state::ssh_marks::{KeyMark, key_marks, outside_folder};
use crate::state::ssh_use::add_blocked;
use crate::ui::common::menu_item;
use crate::ui::theme::{color, type_scale};
use crate::ui::widgets::{self, Icon, buttons};
use eframe::egui::{self, Align, Color32, CornerRadius, Layout, RichText, Sense, Stroke, Ui, WidgetInfo, WidgetType, vec2};
use trellis_core::i18n::Lang;
use trellis_core::native_path;
use trellis_core::paths::same_spelling;
use trellis_core::ssh_keys::{SshKeyPair, SshListing};

/// The card's inner margin, sides and top/bottom.
const PAD_X: i8 = 14;
const PAD_Y: i8 = 12;

pub(super) fn card(ui: &mut Ui, t: Lang, key: &SshKeyPair, listing: &SshListing, page: &SshPage, cx: &SshContext, msgs: &mut Vec<Msg>) {
    let w = fit(ui, LABEL_W + GAP + ROW_W);
    egui::Frame::new()
        .fill(color::panel())
        .stroke(Stroke::new(1.0_f32, color::border()))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(egui::Margin::symmetric(PAD_X, PAD_Y))
        .show(ui, |ui| {
            ui.set_width(w - 2.0 * (f32::from(PAD_X) + 1.0));
            ui.spacing_mut().item_spacing.y = 4.0;
            title_row(ui, t, key, listing, page, msgs);
            ui.horizontal(|ui| {
                let comment = if key.comment.is_empty() { t.tr("prefs.ssh.card.no_comment") } else { key.comment.as_str() };
                ui.label(RichText::new(comment).size(type_scale::SMALL).color(color::weak()));
                if outside_folder(key, listing) {
                    ui.label(RichText::new(native_path::shown(&key.private)).size(type_scale::SMALL).color(color::faint()));
                }
            });
            match &key.fingerprint {
                Some(fp) => ui.label(RichText::new(fp).monospace().size(type_scale::SMALL).color(color::faint())),
                None => ui.label(RichText::new(t.tr("prefs.ssh.card.no_fingerprint")).size(type_scale::SMALL).color(color::faint())),
            };
            ui.add_space(4.0);
            actions(ui, t, key, listing, page, cx, msgs);
        });
    ui.add_space(8.0);
}

/// The key icon, file name, kind and marks, with Copy public key at the right end.
fn title_row(ui: &mut Ui, t: Lang, key: &SshKeyPair, listing: &SshListing, page: &SshPage, msgs: &mut Vec<Msg>) {
    ui.horizontal(|ui| {
        let (icon, _) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::hover());
        widgets::paint_icon(ui.painter(), icon.center(), Icon::Key, color::weak());
        ui.label(RichText::new(key.file_name()).font(type_scale::strong(ui, type_scale::BODY)).color(color::text()));
        ui.label(RichText::new(key.kind_label()).size(type_scale::SMALL).color(color::weak()));
        for mark in key_marks(page, listing, key) {
            let (text, tip, tint) = mark_parts(t, &mark);
            chip(ui, &text, &tip, tint);
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.add(buttons::secondary(t.tr("prefs.ssh.card.copy.button"))).clicked() {
                msgs.push(Msg::Ssh(SshMsg::CopyPublicKey(key.private.clone())));
            }
        });
    });
}

/// A mark's text, hover tip and tint.
fn mark_parts(t: Lang, mark: &KeyMark) -> (String, String, Color32) {
    match mark {
        KeyMark::Default => (t.tr("prefs.ssh.mark.default").into(), t.tr("prefs.ssh.mark.default.tip").into(), color::primary()),
        KeyMark::Repo(repo) => {
            (t.trf("prefs.ssh.mark.repo", &[("repo", repo)]), t.trf("prefs.ssh.mark.repo.tip", &[("repo", repo)]), color::primary())
        }
        KeyMark::TriedByDefault => (t.tr("prefs.ssh.mark.tried").into(), t.tr("prefs.ssh.mark.tried.tip").into(), color::weak()),
        KeyMark::InAgent => (t.tr("prefs.ssh.mark.agent").into(), t.tr("prefs.ssh.mark.agent.tip").into(), color::added()),
    }
}

/// One painted chip in a rect of its own, with its hover tip.
fn chip(ui: &mut Ui, text: &str, tip: &str, tint: Color32) {
    let font = type_scale::font(type_scale::CAPTION);
    let w = ui.painter().layout_no_wrap(text.to_owned(), font, Color32::WHITE).size().x + 12.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 20.0), Sense::hover());
    widgets::chip(ui.painter(), rect.left(), rect.center().y, text, tint.gamma_multiply(0.25), color::text());
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, text));
    resp.on_hover_text(tip);
}

/// The small action buttons under the fingerprint.
fn actions(ui: &mut Ui, t: Lang, key: &SshKeyPair, listing: &SshListing, page: &SshPage, cx: &SshContext, msgs: &mut Vec<Msg>) {
    ui.horizontal(|ui| {
        ui.spacing_mut().button_padding = vec2(6.0, 2.0);
        let blocked = add_blocked(page, cx.job.as_deref());
        let add = buttons::secondary(t.tr("prefs.ssh.card.add_agent.button")).small().enabled(blocked.is_none()).why(blocked.as_deref());
        if ui.add(add).clicked() {
            msgs.push(Msg::Ssh(SshMsg::AddToAgent(key.private.clone())));
        }
        if page.agent_adding.as_ref().is_some_and(|p| same_spelling(p, &key.private)) {
            ui.add(egui::Spinner::new().size(12.0));
        }
        ui.menu_button(t.tr("prefs.ssh.card.add_service.button"), |ui| {
            if cx.services.is_empty() {
                ui.label(RichText::new(t.tr("prefs.ssh.card.no_services.help")).color(color::weak()));
            }
            for (account, label) in &cx.services {
                menu_item(msgs, ui, label, Msg::Ssh(SshMsg::ChooseService { key: key.private.clone(), account: account.clone() }));
            }
        });
        if outside_folder(key, listing) {
            let forget = buttons::secondary(t.tr("prefs.ssh.card.forget.button")).small().tip(t.tr("prefs.ssh.card.forget.tip"));
            if ui.add(forget).clicked() {
                msgs.push(Msg::Ssh(SshMsg::ForgetKey(key.private.clone())));
            }
        }
    });
}
