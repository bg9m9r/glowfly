//! The "Trust this repository?" dialog: what in the repository's own config and hooks makes git
//! run programs, and the Trust / Don't trust choice (update/trust.rs records it). When a git
//! job the user started, or the built-in terminal's shells, wait for the decision, it says so
//! and offers Trust and run or Cancel instead. For a repository trusted earlier that gained
//! settings or hooks since (or whose hook scripts or commands file changed), it asks again and
//! lists only what is new.

use super::dialog_frame::{Cx, Dialog, Footer};
use super::theme::color;
use super::widgets::{self, Icon, buttons, icon};
use crate::msg::{Msg, TrustMsg};
use crate::state::trust::{TrustPrompt, hooks_label, shown_value};
use crate::ui::theme::type_scale;
use eframe::egui::{self, RichText, Ui};
use glowfly_core::i18n::Lang;
use glowfly_core::text::printable;
use glowfly_core::trust::TrustScan;
use std::path::Path;

/// Draws the prompt; returns true when Cancel was clicked (the dialog closes without a
/// decision). The other buttons send a decision and update closes it. Enter never trusts: the
/// footer's tone makes Cancel the default (and with no Cancel there is no default).
pub fn show(ui: &mut Ui, cx: Cx, p: &TrustPrompt, msgs: &mut Vec<Msg>) -> bool {
    let title = if p.gained.is_some() { "Trust this repository again?" } else { "Trust this repository?" };
    let mut d = Dialog::new(cx, title).begin(ui);
    d.body(ui, |ui| body(ui, cx.t, p));
    let footer = if p.held.is_some() || p.terminal {
        Footer::new().cancel().danger(buttons::primary("Trust and run"))
    } else {
        Footer::new().alt(buttons::secondary("Don't trust")).danger(buttons::primary("Trust"))
    };
    let out = d.finish(ui, footer);
    if out.primary || out.alt.is_some() {
        msgs.push(Msg::Trust(TrustMsg::Decide { real: p.real.clone(), trusted: out.primary }));
    }
    out.dismiss
}

/// Where the repository is, the job waiting, and what in it runs programs.
fn body(ui: &mut Ui, t: Lang, p: &TrustPrompt) {
    ui.label(RichText::new(printable(&p.name)).font(type_scale::strong(ui, type_scale::BODY)));
    ui.add(egui::Label::new(RichText::new(glowfly_core::native_path::shown(&p.workdir)).color(color::weak())).wrap());
    if p.held.is_some() || p.terminal || p.refused {
        ui.add_space(8.0);
    }
    if let Some(label) = &p.held {
        let what = format!("{} needs git to run this repository's settings and hooks.", printable(label));
        ui.add(egui::Label::new(RichText::new(what).font(type_scale::strong(ui, type_scale::BODY))).wrap());
    }
    if p.terminal {
        let what =
            "The terminal waits for this decision: shell prompts often run git, which runs what this repository's settings and hooks name.";
        ui.add(egui::Label::new(RichText::new(what).font(type_scale::strong(ui, type_scale::BODY))).wrap());
    }
    if p.refused {
        ui.label(RichText::new("You chose not to trust it before.").color(color::weak()));
    }
    ui.add_space(8.0);
    let shown = p.gained.as_ref().unwrap_or(&p.scan);
    // A commands file alone runs nothing by itself: listed below, after "nothing runs", unless
    // it is what changed since the repository was trusted.
    if shown.is_empty() && (p.gained.is_none() || shown.commands.is_none()) {
        ui.add(egui::Label::new("Nothing in its settings or hooks runs programs right now.").wrap());
    } else {
        let intro = if p.gained.is_some() {
            t.tr("trust.gained.intro")
        } else {
            "This repository's own settings or hooks make git run programs. So far Glowfly has only read its files."
        };
        ui.add(egui::Label::new(intro).wrap());
        ui.add_space(8.0);
        egui::ScrollArea::vertical().id_salt("trust-findings").max_height(180.0).auto_shrink([false, true]).show(ui, |ui| {
            findings(ui, shown, &p.workdir);
            for m in &shown.submodules {
                ui.add_space(4.0);
                ui.label(RichText::new(format!("In submodule {}", printable(&m.path))).font(type_scale::strong(ui, type_scale::BODY)));
                ui.indent(("trust-submodule", &m.path), |ui| findings(ui, &m.scan, &p.workdir));
            }
        });
    }
    if shown.commands.is_some() {
        commands(ui, t);
    }
    ui.add_space(8.0);
    ui.add(
        egui::Label::new(
            "Trust it only if you know where it came from. Until then, Glowfly won't fetch in the background, \
             ask its credential helper for sign-ins or let git read its status, and it asks before running git \
             commands you start.",
        )
        .wrap(),
    );
}

/// The repository's own commands file, new or changed: its commands run only when picked.
fn commands(ui: &mut Ui, t: Lang) {
    ui.add_space(6.0);
    ui.label(type_scale::section_text(ui, t.tr("trust.commands.title")));
    let file = format!("{}/{}", glowfly_core::custom_commands::REPO_DIR, glowfly_core::custom_commands::FILE_NAME);
    ui.label(RichText::new(file).monospace());
    ui.add(egui::Label::new(RichText::new(t.tr("trust.commands.note")).color(color::weak())).wrap());
}

/// The settings and hooks of one repository (the top one or a submodule).
fn findings(ui: &mut Ui, scan: &TrustScan, workdir: &Path) {
    if !scan.config.is_empty() {
        ui.label(type_scale::section_text(ui, "Settings that run commands"));
        for c in &scan.config {
            let row = format!("{} = {}", printable(&c.key), shown_value(&c.value));
            ui.add(egui::Label::new(RichText::new(row).monospace()).wrap());
        }
        ui.add_space(6.0);
    }
    if !scan.hooks.is_empty() {
        ui.horizontal(|ui| {
            // The icon starts at the cursor, which is the text margin.
            ui.spacing_mut().item_spacing.x = 6.0;
            let (rect, _) = ui.allocate_exact_size(egui::vec2(icon::SIZE_MD, icon::SIZE_MD), egui::Sense::hover());
            widgets::paint_icon_sized(ui.painter(), rect.center(), Icon::Hook, color::text(), icon::SIZE_MD);
            ui.label(type_scale::section_text(ui, "Hooks"));
            if let Some(dir) = &scan.hooks_dir {
                ui.label(RichText::new(format!("in {}", hooks_label(dir, workdir))).color(color::weak()));
            }
        });
        for h in &scan.hooks {
            ui.label(RichText::new(printable(h)).monospace());
        }
    }
}
