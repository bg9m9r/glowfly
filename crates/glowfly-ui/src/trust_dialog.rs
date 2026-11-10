//! The "Trust this repository?" dialog: what in the repository's own config and hooks makes git
//! run programs (and, apart, settings that change how git connects: TLS checks, proxies and the
//! cookie file, and the steps a rebase or pick in progress runs on continuing),
//! and the Trust / Don't trust choice (update/trust.rs records it). When a git
//! job the user started, or the built-in terminal's shells, wait for the decision, it says so
//! and offers Trust and run or Cancel instead. For a repository trusted earlier that gained
//! settings or hooks since (or whose hook scripts or commands file changed), it asks again and
//! lists only what is new. Hooks that come from the working tree are said to change with the
//! branch, and a job held because the commit it brings in has other versions of them lists
//! those versions.

use super::dialog_frame::{Cx, Dialog, Footer};
use super::theme::color;
use super::trust_dialog_rows::{self as rows, Mark};
use super::widgets::{self, Icon, buttons, icon};
use crate::theme::type_scale;
use eframe::egui::{self, RichText, Ui};
use glowfly_core::i18n::Lang;
use glowfly_core::text::printable;
use glowfly_core::trust::known::{self, Known};
use glowfly_core::trust::{ConfigCommand, Follow, TrustScan, is_network_setting};
use glowfly_model::msg::{Msg, TrustMsg};
use glowfly_model::state::trust::rows::row_follows;
use glowfly_model::state::trust::{TrustPrompt, hooks_label, shown_value};
use std::path::Path;

/// Draws the prompt; returns true when Cancel was clicked (the dialog closes without a
/// decision). The other buttons send a decision and update closes it. Enter never trusts: the
/// footer's tone makes Cancel the default (and with no Cancel there is no default). Neither
/// answer is filled: this is a safety decision, so no button is styled as the expected one.
pub fn show(ui: &mut Ui, cx: Cx, p: &TrustPrompt, msgs: &mut Vec<Msg>) -> bool {
    let title = match (&p.incoming, &p.gained) {
        _ if p.unchecked.is_some() => cx.t.tr("trust.incoming.title"),
        (Some(_), _) => cx.t.tr("trust.incoming.title"),
        (None, Some(_)) => "Trust this repository again?",
        (None, None) => "Trust this repository?",
    };
    let mut d = Dialog::new(cx, title).begin(ui);
    d.body(ui, |ui| body(ui, cx.t, p));
    let footer = if p.held.is_some() || p.terminal {
        Footer::new().cancel().danger(buttons::secondary("Trust and run"))
    } else {
        Footer::new().alt(buttons::secondary("Don't trust")).danger(buttons::secondary("Trust"))
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
    rows::path_line(ui, &glowfly_core::native_path::shown(&p.workdir));
    if p.held.is_some() || p.terminal || p.refused {
        ui.add_space(8.0);
    }
    if let Some(label) = &p.held {
        let what = format!("{} needs git to run this repository's settings and hooks.", printable(label));
        ui.add(egui::Label::new(RichText::new(what).font(type_scale::strong(ui, type_scale::BODY))).wrap());
    }
    if let Some(u) = &p.unchecked {
        ui.add(
            egui::Label::new(
                RichText::new(glowfly_model::state::trust::unchecked_text(t, &u.reason)).font(type_scale::strong(ui, type_scale::BODY)),
            )
            .wrap(),
        );
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
    let shown = p.incoming.as_deref().or(p.gained.as_ref()).unwrap_or(&p.scan);
    // A commands file alone runs nothing by itself: listed below, after "nothing runs", unless
    // it is what changed since the repository was trusted.
    if shown.is_empty() && p.incoming.is_none() && (p.gained.is_none() || shown.commands.is_none()) {
        ui.add(egui::Label::new("Nothing in its settings or hooks runs programs right now.").wrap());
    } else {
        let intro = if p.incoming.is_some() {
            t.tr("trust.incoming.intro")
        } else if p.gained.is_some() {
            t.tr("trust.gained.intro")
        } else {
            "This repository's own settings or hooks make git run programs. So far Glowfly has only read its files."
        };
        ui.add(egui::Label::new(intro).wrap());
        ui.add_space(8.0);
        egui::ScrollArea::vertical().id_salt("trust-findings").max_height(180.0).auto_shrink([false, true]).show(ui, |ui| {
            findings(ui, t, shown, &p.workdir, &row_follows(p, None));
            for m in &shown.submodules {
                ui.add_space(4.0);
                ui.label(RichText::new(format!("In submodule {}", printable(&m.path))).font(type_scale::strong(ui, type_scale::BODY)));
                let follows = row_follows(p, Some(&m.path));
                ui.indent(("trust-submodule", &m.path), |ui| findings(ui, t, &m.scan, &p.workdir, &follows));
            }
        });
    }
    if shown.commands.is_some() {
        commands(ui, t);
    }
    ui.add_space(8.0);
    // Two labels with no gap: the advice in strong text, then what holds until then.
    let gap = std::mem::replace(&mut ui.spacing_mut().item_spacing.y, 0.0);
    let advice = RichText::new("Trust it only if you know where it came from.").font(type_scale::strong(ui, type_scale::BODY));
    ui.add(egui::Label::new(advice.color(color::text())).wrap());
    let until = "Until then, Glowfly won't fetch in the background, ask its credential helper for sign-ins or let git read \
                 its status, and it asks before running git commands you start.";
    ui.add(egui::Label::new(RichText::new(until).color(color::weak())).wrap());
    ui.spacing_mut().item_spacing.y = gap;
}

/// The repository's own commands file, new or changed: its commands run only when picked.
fn commands(ui: &mut Ui, t: Lang) {
    ui.add_space(6.0);
    ui.label(type_scale::section_text(ui, t.tr("trust.commands.title")));
    let file = format!("{}/{}", glowfly_core::custom_commands::REPO_DIR, glowfly_core::custom_commands::FILE_NAME);
    ui.label(RichText::new(file).monospace());
    ui.add(egui::Label::new(RichText::new(t.tr("trust.commands.note")).color(color::weak())).wrap());
}

/// The settings and hooks of one repository (the top one or a submodule); `places` are the
/// places in its working tree its settings may name, which Git LFS's must not.
fn findings(ui: &mut Ui, t: Lang, scan: &TrustScan, workdir: &Path, places: &[Follow]) {
    let (network, commands): (Vec<_>, Vec<_>) = scan.config.iter().partition(|c| is_network_setting(&c.key));
    settings(ui, t, "Settings that run commands", &commands, |c| match known::setting(c, places) {
        Some(Known::GitLfs) => Mark::GitLfs,
        None => Mark::Unknown { tip: t.tr("trust.unknown.tip") },
    });
    if !network.is_empty() {
        settings(ui, t, t.tr("trust.network.title"), &network, |_| Mark::Unknown { tip: t.tr("trust.network.note") });
        ui.add(egui::Label::new(RichText::new(t.tr("trust.network.note")).color(color::weak())).wrap());
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
            let mark = match scan.known_hooks.get(h) {
                Some(Known::GitLfs) => Mark::GitLfs,
                None => Mark::Unknown { tip: t.tr("trust.unknown.tip") },
            };
            rows::hook(ui, t, &printable(h), mark);
        }
    }
    steps(ui, t, scan);
    follows(ui, t, scan);
}

/// The most characters of a step the dialog shows.
const STEP_CHARS: usize = 400;

/// The steps a rebase, cherry-pick or revert in progress runs when it continues, one row each.
fn steps(ui: &mut Ui, t: Lang, scan: &TrustScan) {
    if scan.steps.is_empty() {
        return;
    }
    if !scan.hooks.is_empty() {
        ui.add_space(6.0);
    }
    ui.label(type_scale::section_text(ui, t.tr("trust.steps.title")));
    for step in &scan.steps {
        let mut shown = printable(step);
        if let Some((at, _)) = shown.char_indices().nth(STEP_CHARS) {
            shown.truncate(at);
            shown.push('…');
        }
        rows::hook(ui, t, &shown, Mark::Unknown { tip: t.tr("trust.unknown.tip") });
    }
    ui.add(egui::Label::new(RichText::new(t.tr("trust.steps.note")).color(color::weak())).wrap());
    ui.add_space(6.0);
}

/// One group of settings under `title`, one `key = value` row each with its `mark`; nothing
/// when empty.
fn settings(ui: &mut Ui, t: Lang, title: &str, list: &[&ConfigCommand], mark: impl Fn(&ConfigCommand) -> Mark) {
    if list.is_empty() {
        return;
    }
    ui.label(type_scale::section_text(ui, title));
    for c in list {
        rows::setting(ui, t, &printable(&c.key), &shown_value(&c.value), mark(c));
    }
    ui.add_space(6.0);
}

/// Where hooks come from the working tree: they change with the branch.
fn follows(ui: &mut Ui, t: Lang, scan: &TrustScan) {
    if scan.follows.is_empty() {
        return;
    }
    let paths: Vec<String> = scan.follows.iter().map(|f| printable(&f.path)).collect();
    ui.add_space(4.0);
    let note = t.trf("trust.follows.note", &[("path", &paths.join(", "))]);
    ui.add(egui::Label::new(RichText::new(note).color(color::weak())).wrap());
}
