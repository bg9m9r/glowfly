//! The Add remote and Edit remote dialog: a name, a fetch URL and an optional push URL, checked
//! as they are typed. `update` checks the request again and closes the dialog once the job
//! starts. When adding, a link opens the Add a fork dialog for a remote whose service lists forks.

use super::super::dialog_frame::{Cx, Dialog, Footer};
use super::super::theme::color;
use super::super::widgets::buttons;
use crate::msg::Msg;
use crate::remote::RemoteMsg;
use crate::state::remotes::{RemoteForm, extra_push_urls, name_problem, push_problem, push_same_as_fetch, url_problem};
use crate::ui::theme::controls;
use eframe::egui::{self, Id, Key, RichText};
use trellis_core::remote_ops::{RemoteNameProblem, UrlProblem};
use trellis_core::repo::Snapshot;

/// A field with its label above it; returns whether Enter left it.
pub(super) fn field(ui: &mut egui::Ui, label: &str, text: &mut String, id: &str, hint: &str) -> (egui::Response, bool) {
    ui.label(label);
    let r = controls::field(ui, egui::TextEdit::singleline(text).id(Id::new(id)).hint_text(hint).desired_width(f32::INFINITY));
    let enter = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
    (r, enter)
}

pub(super) fn problem_line(ui: &mut egui::Ui, text: String) {
    if !text.is_empty() {
        ui.label(RichText::new(text).color(color::red()));
    }
}

pub(super) fn note(ui: &mut egui::Ui, text: impl Into<String>) {
    ui.label(RichText::new(text.into()).color(color::weak()));
}

/// The Add/Edit remote dialog; returns true when Cancel closes it (update closes it once the job
/// starts). The name field takes the focus when nothing has it, unless `focus` is false (a
/// credential prompt is open above it). The button is disabled while a job runs (`idle` false).
/// `fork_source` is the remote and service the fork link offers (add mode only).
pub fn show(
    ui: &mut egui::Ui,
    cx: Cx,
    form: &mut RemoteForm,
    snap: Option<&Snapshot>,
    fork_source: Option<(String, String)>,
    idle: bool,
    msgs: &mut Vec<Msg>,
) -> bool {
    let focus = cx.keyboard;
    let title = match &form.original {
        Some(o) => format!("Edit remote {}", o.name),
        None => "Add remote".to_owned(),
    };
    let d = Dialog::new(cx, title).begin(ui);
    if let (None, Some((remote, service))) = (&form.original, fork_source)
        && ui.link(format!("Pick a fork of {remote} on {service}…")).clicked()
    {
        msgs.push(Msg::Remote(RemoteMsg::ForkDialog(remote)));
    }
    ui.add_space(6.0);

    let (name_r, enter_name) = field(ui, "Name", &mut form.name, "trellis-remote-name", "upstream");
    let name_p = name_problem(snap, form);
    problem_line(ui, name_p.as_ref().filter(|p| **p != RemoteNameProblem::Empty).map(|p| p.message()).unwrap_or_default());
    if focus && ui.memory(|m| m.focused().is_none()) {
        name_r.request_focus();
    }
    ui.add_space(6.0);

    let (_, enter_url) = field(ui, "Fetch URL", &mut form.url, "trellis-remote-url", "https://example.com/owner/repo.git or a folder path");
    let url_p = url_problem(form);
    problem_line(ui, url_p.as_ref().filter(|p| **p != UrlProblem::Empty).map(|p| p.message(false)).unwrap_or_default());
    ui.add_space(6.0);

    let (_, enter_push) = field(ui, "Push URL (optional)", &mut form.push_url, "trellis-remote-push", "Same as the fetch URL");
    let push_p = push_problem(form);
    note(ui, "Leave empty to push to the fetch URL.");
    if let Some(p) = &push_p {
        problem_line(ui, p.message(true));
    } else if push_same_as_fetch(form) {
        note(ui, "Same as the fetch URL.");
    }

    if form.is_edit() {
        if form.renaming() {
            ui.add_space(4.0);
            note(ui, "Renaming also renames its remote-tracking branches and updates the branches that track it.");
        }
        let extra = extra_push_urls(form);
        if extra > 0 {
            ui.add_space(4.0);
            let s = if extra == 1 { "" } else { "s" };
            note(ui, format!("This remote has {extra} more push URL{s} in its config; they are kept."));
        }
    } else {
        ui.add_space(6.0);
        controls::checkbox(ui, &mut form.fetch_after, "Fetch its branches after adding");
    }

    let valid = name_p.is_none() && url_p.is_none() && push_p.is_none();
    let ready = valid && idle && form.changed();
    let enter = enter_name || enter_url || enter_push;
    let label = if form.is_edit() { "Save" } else { "Add remote" };
    let out = d.finish(ui, Footer::new().cancel().primary(buttons::primary(label).enabled(ready)).submit(enter));
    if out.primary {
        msgs.push(Msg::Remote(RemoteMsg::Submit));
    }
    out.dismiss
}
