//! The Add a fork dialog: the forks of a remote's repository on its hosting service, filtered as
//! the user types, with the ones already added as remotes marked; the picked fork is added as a
//! remote under a name Trellis suggests. `update` checks the request again and closes the dialog
//! once the job starts.

use super::super::dialog_frame::{Cx, Dialog, Footer};
use super::super::pane_status::{self, Failure};
use super::super::theme::color;
use super::super::widgets::form_message;
use super::super::widgets::{FieldGroup, TextField, buttons};
use super::dialog::{field, note, problem};
use crate::msg::Msg;
use crate::remote::RemoteMsg;
use crate::state::blocked::BUSY;
use crate::state::forks::{ForkPicker, added_as, error_text, visible};
use crate::ui::theme::controls;
use eframe::egui::{self, Id, Layout, RichText};
use glowfly_core::forge::Connection;
use glowfly_core::remote_ops::{RemoteNameProblem, check_remote_name};
use glowfly_core::repo::Snapshot;

/// Why Add remote is disabled before a fork is picked.
pub(crate) const PICK_FIRST: &str = "Pick a fork from the list";

/// The list of forks (only the rows in view are laid out: a list can hold 1000); a click on a
/// row that isn't a remote yet picks it.
fn rows(ui: &mut egui::Ui, p: &ForkPicker, snap: Option<&Snapshot>, connections: &[Connection], msgs: &mut Vec<Msg>) {
    let shown = visible(p);
    if shown.is_empty() {
        note(ui, "No fork matches the filter.");
        return;
    }
    let row_height = ui.spacing().interact_size.y;
    let area = egui::ScrollArea::vertical().id_salt("trellis-fork-list").max_height(220.0).auto_shrink([false, true]);
    area.show_rows(ui, row_height, shown.len(), |ui, range| {
        for &i in &shown[range] {
            let fork = &p.list.forks[i];
            let added = snap.and_then(|s| added_as(s, fork, connections));
            ui.horizontal(|ui| {
                let label = egui::SelectableLabel::new(p.picked == Some(i), fork.full_name.as_str());
                if ui.add_enabled(added.is_none(), label).clicked() {
                    msgs.push(Msg::Remote(RemoteMsg::PickFork(i)));
                }
                let tag = match (&added, fork.private) {
                    (Some(r), _) => format!("added as {r}"),
                    (None, true) => "private".to_owned(),
                    (None, false) => String::new(),
                };
                if !tag.is_empty() {
                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(tag).color(color::weak()));
                    });
                }
            });
        }
    });
}

/// The Add a fork dialog; returns true when Cancel closes it. The filter takes the focus when
/// nothing has it, unless `focus` is false (a credential prompt is open above it). Add remote
/// is disabled while a job runs (`idle` false).
pub fn show(
    ui: &mut egui::Ui,
    cx: Cx,
    p: &mut ForkPicker,
    snap: Option<&Snapshot>,
    connections: &[Connection],
    idle: bool,
    msgs: &mut Vec<Msg>,
) -> bool {
    let focus = cx.keyboard;
    let repo = p.target.display();
    let service = p.target.provider.name();
    let subtitle = format!("Forks on {service}. The one you pick is added as a remote.");
    let d = Dialog::new(cx, format!("Add a fork of {repo}")).subtitle(subtitle).begin(ui);
    let mut submit = false;
    let mut listed = false;
    if p.loading {
        pane_status::loading_line(ui, "Loading forks…");
    } else if let Some(e) = &p.error {
        let error = error_text(e);
        let f = Failure { title: "Couldn't list the forks", error: &error, retry: Some(Msg::Remote(RemoteMsg::RetryForks)) };
        pane_status::failed_line(ui, cx.t, f, msgs);
    } else if p.list.forks.is_empty() {
        ui.label(format!("Nobody has forked {repo} yet."));
        if p.target.provider.creates_forks() && ui.add(buttons::secondary(format!("Fork it on {service}…")).small()).clicked() {
            msgs.push(Msg::Remote(RemoteMsg::ForkOnService(p.remote.clone())));
        }
    } else {
        listed = true;
        let r = ui.add(TextField::new(&mut p.filter).id(Id::new("trellis-fork-filter")).hint("Filter forks"));
        if focus && ui.memory(|m| m.focused().is_none()) {
            r.request_focus();
        }
        ui.add_space(4.0);
        rows(ui, p, snap, connections, msgs);
        if p.list.truncated {
            note(ui, "Showing the newest 1000 forks. For another fork, paste its URL in Add remote.");
        }
        ui.add_space(12.0);
        let existing: Vec<&str> = snap.map(|s| s.remote_info.iter().map(|r| r.name.as_str()).collect()).unwrap_or_default();
        let shown = check_remote_name(p.name.trim(), &existing, None)
            .err()
            .filter(|n| *n != RemoteNameProblem::Empty)
            .and_then(|n| problem(n.message()));
        let group = FieldGroup::new("Remote name").message(&p.touched, "name", shown);
        let (r, enter) = field(ui, group, &mut p.name, "trellis-fork-name", "teammate");
        form_message::track(&mut p.touched, "name", &r);
        submit = enter;
    }

    let existing: Vec<&str> = snap.map(|s| s.remote_info.iter().map(|r| r.name.as_str()).collect()).unwrap_or_default();
    let name_p = check_remote_name(p.name.trim(), &existing, None).err();
    let ready = listed && p.picked.is_some() && name_p.is_none() && idle;
    if listed {
        controls::checkbox(ui, &mut p.fetch_after, "Fetch its branches after adding");
    }
    let why = if !idle {
        Some(BUSY.to_owned())
    } else if p.picked.is_none() {
        Some(PICK_FIRST.to_owned())
    } else {
        super::fork_create::name_why(name_p.as_ref(), &p.touched)
    };
    let mut footer = Footer::new().cancel().submit(submit);
    if listed {
        footer = footer.primary(buttons::primary("Add remote").enabled(ready).why(why.as_deref()));
    }
    let out = d.finish(ui, footer);
    if out.attempted {
        p.touched.attempt();
    }
    if out.primary {
        msgs.push(Msg::Remote(RemoteMsg::AddFork));
    }
    out.dismiss
}
