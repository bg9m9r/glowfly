//! The Add a fork dialog: the forks of a remote's repository on its hosting service, filtered as
//! the user types, with the ones already added as remotes marked; the picked fork is added as a
//! remote under a name Trellis suggests. `update` checks the request again and closes the dialog
//! once the job starts.

use super::super::theme::color;
use super::dialog::{field, note, problem_line};
use crate::msg::Msg;
use crate::remote::RemoteMsg;
use crate::state::forks::{ForkPicker, added_as, error_text, visible};
use crate::ui::theme::controls;
use eframe::egui::{self, Id, Layout, RichText};
use trellis_core::forge::Connection;
use trellis_core::remote_ops::{RemoteNameProblem, check_remote_name};
use trellis_core::repo::Snapshot;

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
    p: &mut ForkPicker,
    snap: Option<&Snapshot>,
    connections: &[Connection],
    focus: bool,
    idle: bool,
    msgs: &mut Vec<Msg>,
) -> bool {
    let repo = p.target.display();
    let service = p.target.provider.name();
    ui.heading(format!("Add a fork of {repo}"));
    note(ui, format!("Forks on {service}. The one you pick is added as a remote."));
    ui.add_space(8.0);
    let mut submit = false;
    let mut listed = false;
    if p.loading {
        ui.horizontal(|ui| {
            ui.add(egui::Spinner::new());
            ui.label("Loading forks…");
        });
    } else if let Some(e) = &p.error {
        ui.label(RichText::new(error_text(e)).color(color::red()));
        if ui.button("Retry").clicked() {
            msgs.push(Msg::Remote(RemoteMsg::RetryForks));
        }
    } else if p.list.forks.is_empty() {
        ui.label(format!("Nobody has forked {repo} yet."));
        if p.target.provider.creates_forks() && ui.button(format!("Fork it on {service}…")).clicked() {
            msgs.push(Msg::Remote(RemoteMsg::ForkOnService(p.remote.clone())));
        }
    } else {
        listed = true;
        let r = controls::field(
            ui,
            egui::TextEdit::singleline(&mut p.filter)
                .id(Id::new("trellis-fork-filter"))
                .hint_text("Filter forks")
                .desired_width(f32::INFINITY),
        );
        if focus && ui.memory(|m| m.focused().is_none()) {
            r.request_focus();
        }
        ui.add_space(4.0);
        rows(ui, p, snap, connections, msgs);
        if p.list.truncated {
            note(ui, "Showing the newest 1000 forks. For another fork, paste its URL in Add remote.");
        }
        ui.add_space(6.0);
        let (_, enter) = field(ui, "Remote name", &mut p.name, "trellis-fork-name", "teammate");
        submit = enter;
    }

    let existing: Vec<&str> = snap.map(|s| s.remote_info.iter().map(|r| r.name.as_str()).collect()).unwrap_or_default();
    let name_p = check_remote_name(p.name.trim(), &existing, None).err();
    let ready = listed && p.picked.is_some() && name_p.is_none() && idle;
    if listed {
        problem_line(ui, name_p.as_ref().filter(|n| **n != RemoteNameProblem::Empty).map(|n| n.message()).unwrap_or_default());
        ui.add_space(4.0);
        controls::checkbox(ui, &mut p.fetch_after, "Fetch its branches after adding");
    }
    let mut close = false;
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        if listed && (ui.add_enabled(ready, egui::Button::new("Add remote")).clicked() || (submit && ready)) {
            msgs.push(Msg::Remote(RemoteMsg::AddFork));
        }
        if ui.button("Cancel").clicked() {
            close = true;
        }
    });
    close
}
