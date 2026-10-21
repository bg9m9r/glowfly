//! Preferences > SSH, under the key cards: the `~/.ssh/config` IdentityFile entries that can't
//! be listed as cards (each with where it is and why), and the keys only the agent holds, as
//! compact cards that can only copy their public key. Every action is a `Msg`.

use super::ssh_card::{PAD_X, PAD_Y, chip};
use super::{GAP, LABEL_W, ROW_W, fit, help, warn};
use crate::msg::Msg;
use crate::ssh::SshMsg;
use crate::state::ssh::SshPage;
use crate::state::ssh_marks::agent_only;
use crate::ui::theme::tokens::radius;
use crate::ui::theme::{color, type_scale};
use crate::ui::widgets::{self, Icon, buttons};
use eframe::egui::{self, Align, CornerRadius, Layout, RichText, Sense, Stroke, Ui, vec2};
use std::path::Path;
use trellis_core::i18n::Lang;
use trellis_core::native_path;
use trellis_core::ssh_keys::{AgentKey, ConfigIdentity, ConfigStatus, KeyFileProblem, SshListing, Unresolved};

/// A config file as the page names it: relative to the .ssh folder (`dir`) when inside it.
pub(super) fn config_file_label(file: &Path, dir: &Path) -> String {
    match file.strip_prefix(dir) {
        Ok(rel) if !rel.as_os_str().is_empty() => native_path::shown(rel),
        _ => native_path::shown(file),
    }
}

/// Why an entry isn't listed, in `t`.
fn reason(t: Lang, status: &ConfigStatus) -> String {
    match status {
        ConfigStatus::Key(_) => String::new(),
        ConfigStatus::Missing(_) => t.tr("prefs.ssh.config.reason.missing").into(),
        ConfigStatus::PublicOnly(p) => {
            let file = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            t.trf("prefs.ssh.config.reason.public_only", &[("file", &file)])
        }
        ConfigStatus::NotAPair(_, KeyFileProblem::NoPublicKey) => t.tr("prefs.ssh.config.reason.no_public").into(),
        ConfigStatus::NotAPair(_, KeyFileProblem::NotAPublicKey) => t.tr("prefs.ssh.config.reason.not_a_key").into(),
        ConfigStatus::NotAPair(_, KeyFileProblem::NotAFile) => t.tr("prefs.ssh.config.reason.not_a_file").into(),
        ConfigStatus::NotAPair(_, KeyFileProblem::Unreadable(e)) => t.trf("prefs.ssh.config.reason.unreadable", &[("error", e)]),
        ConfigStatus::Unresolved(u) => match u {
            Unresolved::HostToken(c) => t.trf("prefs.ssh.config.reason.host_token", &[("token", &format!("%{c}"))]),
            Unresolved::Unsupported(c) => {
                let token = match c {
                    '$' => "${".to_owned(),
                    '%' => "%".to_owned(),
                    c => format!("%{c}"),
                };
                t.trf("prefs.ssh.config.reason.unsupported", &[("token", &token)])
            }
            Unresolved::OtherUser(_) => t.tr("prefs.ssh.config.reason.other_user").into(),
            Unresolved::Relative => t.tr("prefs.ssh.config.reason.relative").into(),
            Unresolved::Variable(name) => t.trf("prefs.ssh.config.reason.variable", &[("name", name)]),
            Unresolved::NotWindowsPath => t.tr("prefs.ssh.config.reason.not_windows").into(),
        },
    }
}

/// One entry's line: "{value} ({file}, line {n}, {section}): {reason}".
fn entry_line(t: Lang, c: &ConfigIdentity, dir: &Path) -> String {
    let file = config_file_label(&c.file, dir);
    let mut at = t.trf("prefs.ssh.config.where", &[("file", &file), ("line", &c.line)]);
    if !c.section.is_empty() {
        at = format!("{at}, {}", c.section);
    }
    t.trf("prefs.ssh.config.entry.help", &[("value", &c.raw), ("where", &at), ("reason", &reason(t, &c.status))])
}

/// The config's entries that aren't cards, and why the config couldn't be read.
pub(super) fn config_notes(ui: &mut Ui, t: Lang, listing: &SshListing) {
    let unlisted: Vec<&ConfigIdentity> = listing.config.iter().filter(|c| !matches!(c.status, ConfigStatus::Key(_))).collect();
    if unlisted.is_empty() && listing.config_error.is_none() {
        return;
    }
    ui.vertical(|ui| {
        ui.set_max_width(fit(ui, LABEL_W + GAP + ROW_W));
        ui.spacing_mut().item_spacing.y = 2.0;
        let config = native_path::shown(&listing.config_file);
        if let Some(e) = &listing.config_error {
            warn(ui, &t.trf("prefs.ssh.config.error.warn", &[("file", &config), ("error", e)]));
        }
        if !unlisted.is_empty() {
            help(ui, &t.trf("prefs.ssh.config.unlisted.help", &[("config", &config)]));
            for c in unlisted {
                let line = entry_line(t, c, &listing.dir);
                ui.add(egui::Label::new(RichText::new(line).size(type_scale::SMALL).color(color::faint())).wrap());
            }
        }
    });
    ui.add_space(10.0);
}

/// The keys the agent holds that no card is for, under their heading.
pub(super) fn agent_only_rows(ui: &mut Ui, t: Lang, page: &SshPage, msgs: &mut Vec<Msg>) {
    let keys = agent_only(page);
    if keys.is_empty() {
        return;
    }
    ui.add_space(4.0);
    ui.label(RichText::new(t.tr("prefs.ssh.agent_only.title")).font(type_scale::strong(ui, type_scale::SMALL)).color(color::weak()));
    ui.add_space(6.0);
    for key in keys {
        agent_card(ui, t, key, msgs);
    }
    ui.add_space(4.0);
}

/// One agent-only key: icon, comment, kind and chip with Copy public key at the right, then its
/// fingerprint.
fn agent_card(ui: &mut Ui, t: Lang, key: &AgentKey, msgs: &mut Vec<Msg>) {
    let w = fit(ui, LABEL_W + GAP + ROW_W);
    egui::Frame::new()
        .fill(color::panel())
        .stroke(Stroke::new(1.0_f32, color::border()))
        .corner_radius(CornerRadius::same(radius::LG))
        .inner_margin(egui::Margin::symmetric(PAD_X, PAD_Y))
        .show(ui, |ui| {
            ui.set_width(w - 2.0 * (f32::from(PAD_X) + 1.0));
            ui.spacing_mut().item_spacing.y = 4.0;
            ui.horizontal(|ui| {
                let (icon, _) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::hover());
                widgets::paint_icon(ui.painter(), icon.center(), Icon::Key, color::weak());
                let (comment, tint) = match key.comment.as_str() {
                    "" => (t.tr("prefs.ssh.card.no_comment"), color::weak()),
                    c => (c, color::text()),
                };
                ui.label(RichText::new(comment).font(type_scale::strong(ui, type_scale::BODY)).color(tint));
                ui.label(RichText::new(kind_label(key)).size(type_scale::SMALL).color(color::weak()));
                chip(ui, t.tr("prefs.ssh.mark.agent_only"), t.tr("prefs.ssh.mark.agent_only.tip"), color::weak());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let missing = key.public_key.is_none().then(|| t.tr("prefs.ssh.agent_only.no_public.tip"));
                    let copy = buttons::secondary(t.tr("prefs.ssh.card.copy.button")).enabled(missing.is_none()).why(missing);
                    if ui.add(copy).clicked() {
                        msgs.push(Msg::Ssh(SshMsg::CopyAgentKey(key.fingerprint.clone())));
                    }
                });
            });
            ui.label(RichText::new(&key.fingerprint).monospace().size(type_scale::SMALL).color(color::faint()));
        });
    ui.add_space(8.0);
}

/// The kind, with the size for key types that come in several sizes ("RSA 3072").
fn kind_label(key: &AgentKey) -> String {
    match key.bits {
        Some(bits) if matches!(key.kind.as_str(), "RSA" | "ECDSA" | "DSA") => format!("{} {bits}", key.kind),
        _ => key.kind.clone(),
    }
}
