//! The Create pull/merge request dialog (`Modal::CreatePr`): source, target, push note, title,
//! description, linked issues, draft.

use super::common::{capitalize, issue_chip, parse_keys};
use super::theme::color;
use crate::msg::ForgeMsg;
use crate::msg::Msg;
use crate::state::ForgeState;
use crate::state::branches;
use crate::state::push_pr::{self, HostBranch};
use crate::ui::theme::controls;
use eframe::egui::{self, RichText};
use trellis_core::forge::{self, NewPullRequest, RefStyle};
use trellis_core::repo::Snapshot;

pub(super) struct PrForm<'a> {
    pub source: &'a mut String,
    pub target: &'a mut String,
    pub targets: &'a mut Vec<String>,
    pub title: &'a mut String,
    pub description: &'a mut String,
    pub draft: &'a mut bool,
    pub items: &'a mut String,
}

/// What the create dialog says about `source` not being (all) on the code host's remote, and
/// the local branch it offers to push, if any.
fn push_note(snap: &Snapshot, fs: &ForgeState, source: &str, pushed: bool) -> Option<(String, Option<String>)> {
    let remote = &fs.remote;
    match push_pr::dialog_push(snap, fs, source) {
        Some((b, HostBranch::Missing)) => Some((format!("{source} isn't on {remote} yet."), Some(b.name.clone()))),
        Some((b, HostBranch::Ahead(n))) => {
            Some((format!("{} has {} that aren't on {remote}.", b.name, branches::commits(n)), Some(b.name.clone())))
        }
        Some((_, HostBranch::Blocked(why))) => Some((why, None)),
        Some((_, HostBranch::Pushed)) => None,
        None if !pushed => Some((format!("{source} isn't on {remote}. Fetch, or push it from where it lives."), None)),
        None => None,
    }
}

/// The create pull/merge request form; returns true when the dialog should close.
pub(super) fn show(ui: &mut egui::Ui, f: PrForm, fs: &ForgeState, snap: Option<&Snapshot>, idle: bool, msgs: &mut Vec<Msg>) -> bool {
    let Some(host) = fs.host.as_ref() else { return true };
    let mut close = false;
    let noun = fs.pr_noun();
    ui.heading(format!("Create {noun}"));
    ui.label(RichText::new(format!("{} · {}", host.provider.name(), host.display())).color(color::weak()));
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new(f.source.as_str()).strong());
        ui.label("into");
        egui::ComboBox::from_id_salt("pr-target").selected_text(f.target.as_str()).width(200.0).show_ui(ui, |ui| {
            for t in f.targets.iter() {
                ui.selectable_value(f.target, t.clone(), t);
            }
        });
    });
    let pushed = snap.is_some_and(|s| s.remotes.iter().any(|(r, b)| *r == fs.remote && b.iter().any(|(n, _)| n == f.source)));
    if let Some((text, local)) = snap.and_then(|s| push_note(s, fs, f.source, pushed)) {
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(text).color(color::amber()));
            // The dialog stays open: it is the one shown again once the push lands.
            if let Some(local) = local
                && ui.add_enabled(idle, egui::Button::new(format!("Push {local}"))).clicked()
            {
                msgs.push(Msg::Forge(ForgeMsg::PushAndStartPr { branch: local, target: Some(f.target.clone()) }));
            }
        });
    }
    ui.add_space(6.0);
    controls::field(ui, egui::TextEdit::singleline(f.title).hint_text("Title").desired_width(f32::INFINITY));
    controls::field(
        ui,
        egui::TextEdit::multiline(f.description).hint_text("Description (Markdown)").desired_rows(5).desired_width(f32::INFINITY),
    );
    let keys = parse_keys(f.items);
    if fs.tracker.is_some() {
        ui.horizontal(|ui| {
            ui.label(format!("{}s", capitalize(fs.issue_noun())));
            let hint = match fs.style() {
                Some(RefStyle::Jira) => "e.g. PROJ-12, PROJ-14",
                Some(RefStyle::Trello) => "e.g. AbCd1234 or a card link",
                _ => "e.g. 123, 456",
            };
            controls::field(ui, egui::TextEdit::singleline(f.items).hint_text(hint).desired_width(220.0));
        });
        for k in &keys {
            issue_chip(ui, fs, k, msgs);
        }
    }
    controls::checkbox(ui, f.draft, "Create as draft");
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        let valid = pushed && !f.title.trim().is_empty() && !f.target.is_empty() && f.target != f.source;
        if ui.add_enabled(valid, egui::Button::new(RichText::new(format!("Create {noun}")).strong())).clicked() {
            // Trackers linked by mention get "Closes #12"-style lines; Azure Boards items are
            // linked through the API.
            let mut body = f.description.trim().to_owned();
            let links: Vec<String> = fs
                .tracker
                .as_ref()
                .map(|t| keys.iter().filter_map(|k| forge::link_line(host.provider, t.provider, k)).collect())
                .unwrap_or_default();
            if !links.is_empty() {
                if !body.is_empty() {
                    body.push_str("\n\n");
                }
                body.push_str(&links.join("\n"));
            }
            msgs.push(Msg::Forge(ForgeMsg::CreatePr(NewPullRequest {
                source: f.source.clone(),
                target: f.target.clone(),
                title: f.title.trim().to_owned(),
                description: body,
                draft: *f.draft,
                issues: keys.clone(),
            })));
            close = true;
        }
        if ui.button("Cancel").clicked() {
            close = true;
        }
    });
    close
}
