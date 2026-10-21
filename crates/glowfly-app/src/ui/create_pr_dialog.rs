//! The Create pull/merge request dialog (`Modal::CreatePr`): source, target, push note, title,
//! description, linked issues, draft. As a step of a stack's run (`ForgeState::stack_step`) it
//! says which layer it is and adds Skip; its Cancel stops the run.

use super::common::{capitalize, issue_chip, parse_keys};
use super::dialog_frame::{Cx, Dialog, Footer, Size};
use super::widgets::{FieldGroup, TextField, buttons};
use crate::msg::ForgeMsg;
use crate::msg::Msg;
use crate::msg::StackMsg;
use crate::state::ForgeState;
use crate::state::branches;
use crate::state::push_pr::{self, HostBranch};
use crate::ui::theme::controls;
use crate::ui::theme::type_scale;
use crate::ui::widgets::form_message::{self, Message};
use eframe::egui::{self, RichText};
use glowfly_core::forge::{self, NewPullRequest, RefStyle};
use glowfly_core::i18n::Lang;
use glowfly_core::repo::Snapshot;

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

/// The create pull/merge request form; returns true when the dialog should close. Ctrl/Cmd+Enter
/// in the description creates it.
pub(super) fn show(
    ui: &mut egui::Ui,
    cx: Cx,
    mut f: PrForm,
    fs: &ForgeState,
    snap: Option<&Snapshot>,
    idle: bool,
    msgs: &mut Vec<Msg>,
) -> bool {
    let Some(host) = fs.host.as_ref() else { return true };
    let noun = fs.pr_noun();
    let subtitle = format!("{} · {}", host.provider.name(), host.display());
    let step = fs.stack_step(f.source);
    let mut d = Dialog::new(cx, format!("Create {noun}")).subtitle(subtitle).size(Size::Wide).begin(ui);
    let (pushed, keys) = d.body(ui, |ui| {
        if let Some((index, total)) = step {
            form_message::form(ui, Message::info(cx.t.trf("stack.prs.step", &[("index", &index), ("total", &total)])));
            ui.add_space(8.0);
        }
        body(ui, cx.t, &mut f, fs, snap, idle, msgs)
    });
    let valid = pushed && !f.title.trim().is_empty() && !f.target.is_empty() && f.target != f.source;
    let mut footer = Footer::new().cancel();
    if step.is_some() {
        footer = footer.alt(buttons::secondary(cx.t.tr("stack.prs.skip.button")));
    }
    let out = d.finish(ui, footer.primary(buttons::primary(format!("Create {noun}")).enabled(valid)));
    if step.is_some() {
        if out.alt == Some(0) {
            msgs.push(Msg::Stack(StackMsg::SkipPr));
            return true;
        }
        if out.dismiss && !out.primary {
            msgs.push(Msg::Stack(StackMsg::StopPrs));
        }
    }
    if out.primary {
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
            issues: keys,
        })));
    }
    out.primary || out.dismiss
}

/// The form's fields; returns whether the source is on the host's remote, and the linked keys.
fn body(
    ui: &mut egui::Ui,
    t: Lang,
    f: &mut PrForm,
    fs: &ForgeState,
    snap: Option<&Snapshot>,
    idle: bool,
    msgs: &mut Vec<Msg>,
) -> (bool, Vec<String>) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(f.source.as_str()).font(type_scale::strong(ui, type_scale::BODY)));
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
        form_message::form(ui, Message::warning(text));
        // The dialog stays open: it is the one shown again once the push lands.
        if let Some(local) = local
            && ui.add(buttons::secondary(format!("Push {local}")).small().enabled(idle)).clicked()
        {
            msgs.push(Msg::Forge(ForgeMsg::PushAndStartPr { branch: local, target: Some(f.target.clone()) }));
        }
    }
    ui.add_space(12.0);
    FieldGroup::new("Title").show(ui, |ui| ui.add(TextField::new(f.title)));
    let description = FieldGroup::new("Description").optional(t.tr("dialog.field.optional")).help("Markdown is supported.");
    description.show(ui, |ui| ui.add(TextField::multiline(f.description, 5)));
    let keys = parse_keys(f.items);
    if fs.tracker.is_some() {
        let hint = match fs.style() {
            Some(RefStyle::Jira) => "e.g. PROJ-12, PROJ-14",
            Some(RefStyle::Trello) => "e.g. AbCd1234 or a card link",
            _ => "e.g. 123, 456",
        };
        // The linked items show under the field, inside its group.
        let linked = FieldGroup::new(format!("{}s", capitalize(fs.issue_noun()))).optional(t.tr("dialog.field.optional"));
        linked.show(ui, |ui| {
            let r = ui.add(TextField::new(f.items).hint(hint));
            for k in &keys {
                issue_chip(ui, fs, k, msgs);
            }
            r
        });
    }
    controls::checkbox(ui, f.draft, "Create as draft");
    (pushed, keys)
}
