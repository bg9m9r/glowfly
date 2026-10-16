//! Preferences > SSH: the key pairs in ~/.ssh, each with Copy public key, Use for git, Add to
//! agent and Add to service, then the key git uses and the agent (`ssh_git`) and the Generate New
//! Key form (`ssh_generate`). The view edits only text buffers; everything else is a `Msg`.

use super::{help, row, warn};
use crate::msg::Msg;
use crate::ssh::SshMsg;
use crate::state::ssh::{NO_KEYGEN, SshPage};
use crate::state::ssh_use::{AGENT_ON, NOT_TRUSTED, add_blocked, chosen_at};
use crate::ui::theme::color;
use crate::ui::widgets::{self, Icon};
use eframe::egui::{self, Color32, CornerRadius, FontId, RichText, Sense, Ui, vec2};
use trellis_core::forge::Account;
use trellis_core::paths::same_spelling;
use trellis_core::ssh_keys::{SshKeyPair, SshListing};

/// What the page needs from outside Preferences.
pub struct SshContext {
    /// Settings > Use local SSH agent.
    pub use_agent: bool,
    /// Connected code-hosting accounts, with their menu labels.
    pub services: Vec<(Account, String)>,
    /// The running git job's label.
    pub job: Option<String>,
}

pub fn ssh_page(ui: &mut Ui, page: &mut SshPage, cx: &SshContext, msgs: &mut Vec<Msg>) {
    if let Some(error) = &page.error {
        row(ui, "Key Pairs", |ui| {
            warn(ui, error);
            ui.add_space(4.0);
            if ui.button("Reload").clicked() {
                msgs.push(Msg::Ssh(SshMsg::Reload));
            }
        });
        return;
    }
    let Some(listing) = page.listing.clone() else {
        ui.horizontal(|ui| {
            ui.add(egui::Spinner::new());
            ui.label(RichText::new("Reading your .ssh folder…").color(color::weak()));
        });
        return;
    };
    key_pairs(ui, page, &listing, cx, msgs);
    super::ssh_git::choice_row(ui, page, &listing, cx, msgs);
    super::ssh_git::agent_row(ui, page, cx, msgs);
    super::ssh_generate::generate_form(ui, page, &listing, msgs);
}

fn key_pairs(ui: &mut Ui, page: &mut SshPage, listing: &SshListing, cx: &SshContext, msgs: &mut Vec<Msg>) {
    row(ui, "Key Pairs", |ui| {
        ui.horizontal(|ui| {
            help(ui, "Private keys in your .ssh folder that have a matching .pub file. ssh and git use them to reach SSH remotes.");
            if widgets::icon_button(ui, Icon::Refresh, "Reload keys").clicked() {
                msgs.push(Msg::Ssh(SshMsg::Reload));
            }
        });
        ui.add_space(4.0);
        if listing.keygen.is_none() {
            warn(ui, NO_KEYGEN);
            ui.add_space(4.0);
        }
        if listing.keys.is_empty() {
            help(ui, &format!("No key pairs in {} yet. Generate one below.", listing.dir.display()));
        }
        for key in &listing.keys {
            key_row(ui, key, page, cx, msgs);
            if let Some(form) = page.upload.as_mut().filter(|f| same_spelling(&f.key, &key.private)) {
                super::ssh_service::upload_form(ui, form, &key.file_name(), msgs);
            }
        }
    });
}

/// One key pair: name and kind, comment, fingerprint and its actions, with the copy button on
/// the right.
fn key_row(ui: &mut Ui, key: &SshKeyPair, page: &SshPage, cx: &SshContext, msgs: &mut Vec<Msg>) {
    egui::Frame::new().fill(color::panel()).corner_radius(CornerRadius::same(6)).inner_margin(egui::Margin::symmetric(10, 8)).show(
        ui,
        |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 3.0;
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(key.file_name()).strong().color(color::text()));
                        ui.label(RichText::new(key.kind_label()).size(12.0).color(color::weak()));
                    });
                    let comment = if key.comment.is_empty() { "No comment" } else { key.comment.as_str() };
                    ui.label(RichText::new(comment).size(12.0).color(color::weak()));
                    match &key.fingerprint {
                        Some(fp) => ui.label(RichText::new(fp).monospace().size(11.5).color(color::faint())),
                        None => ui.label(RichText::new("Fingerprint unavailable").size(11.5).color(color::faint())),
                    };
                    ui.add_space(3.0);
                    actions(ui, key, page, cx, msgs);
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                    if ui.button("Copy public key").clicked() {
                        msgs.push(Msg::Ssh(SshMsg::CopyPublicKey(key.private.clone())));
                    }
                });
            });
        },
    );
    ui.add_space(4.0);
}

fn small(text: &str) -> RichText {
    RichText::new(text).size(12.0)
}

/// The key's action buttons and chips.
fn actions(ui: &mut Ui, key: &SshKeyPair, page: &SshPage, cx: &SshContext, msgs: &mut Vec<Msg>) {
    ui.horizontal(|ui| {
        ui.spacing_mut().button_padding = vec2(6.0, 2.0);
        if cx.use_agent {
            ui.add_enabled(false, egui::Button::new(small("Use for git…"))).on_disabled_hover_text(AGENT_ON);
        } else {
            ui.menu_button(small("Use for git…"), |ui| {
                if ui.button("For all repositories").clicked() {
                    msgs.push(Msg::Ssh(SshMsg::UseKey { key: key.private.clone(), repo: false, confirmed: false }));
                    ui.close_menu();
                }
                if let Some(repo) = &page.repo {
                    let item = ui.add_enabled(repo.trusted, egui::Button::new(format!("For {} only", repo.name)));
                    if item.on_disabled_hover_text(NOT_TRUSTED).clicked() {
                        msgs.push(Msg::Ssh(SshMsg::UseKey { key: key.private.clone(), repo: true, confirmed: false }));
                        ui.close_menu();
                    }
                }
            });
        }
        let blocked = add_blocked(page, cx.job.as_deref());
        let add = ui.add_enabled(blocked.is_none(), egui::Button::new(small("Add to agent")));
        if add.on_disabled_hover_text(blocked.unwrap_or_default()).clicked() {
            msgs.push(Msg::Ssh(SshMsg::AddToAgent(key.private.clone())));
        }
        if page.agent_adding.as_ref().is_some_and(|p| same_spelling(p, &key.private)) {
            ui.add(egui::Spinner::new().size(12.0));
        }
        ui.menu_button(small("Add to service…"), |ui| {
            if cx.services.is_empty() {
                ui.add_enabled(false, egui::Button::new("Connect a service in Integrations first"));
            }
            for (account, label) in &cx.services {
                if ui.button(label).clicked() {
                    msgs.push(Msg::Ssh(SshMsg::ChooseService { key: key.private.clone(), account: account.clone() }));
                    ui.close_menu();
                }
            }
        });
        let mut chips: Vec<(String, Color32)> = chosen_at(page.choice.as_ref(), &key.private)
            .into_iter()
            .map(|repo| {
                let label = match (repo, &page.repo) {
                    (true, Some(r)) => format!("git: {}", r.name),
                    _ => "git: all repositories".to_owned(),
                };
                (label, color::primary())
            })
            .collect();
        if key.fingerprint.as_deref().is_some_and(|fp| page.agent.as_ref().is_some_and(|a| a.holds(fp))) {
            chips.push(("In agent".into(), color::green()));
        }
        chip_row(ui, &chips);
    });
}

/// Painted chips, side by side, with room allocated for them.
fn chip_row(ui: &mut Ui, chips: &[(String, Color32)]) {
    if chips.is_empty() {
        return;
    }
    let font = FontId::proportional(11.5);
    let width: f32 = chips.iter().map(|(t, _)| ui.painter().layout_no_wrap(t.clone(), font.clone(), Color32::WHITE).size().x + 18.0).sum();
    let (rect, _) = ui.allocate_exact_size(vec2(width, 20.0), Sense::hover());
    let mut x = rect.left() + 4.0;
    for (text, tint) in chips {
        x += widgets::chip(ui.painter(), x, rect.center().y, text, tint.gamma_multiply(0.25), color::text()) + 6.0;
    }
}
