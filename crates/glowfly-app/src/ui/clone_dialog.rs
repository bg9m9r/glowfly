//! The Clone dialog: the URL (typed, or picked from the user's repositories on a connected
//! service, drawn by `clone_service.rs`), the folder to clone into and the shallow and sparse
//! clone options, checked as the user types with the pure rules in `state::clone`.

use super::dialog_frame::{Cx, Dialog, Footer, Size};
use super::theme::color;
use super::widgets::text_field::{self, TextField};
use super::widgets::{FieldGroup, buttons};
use crate::msg::CloneMsg;
use crate::msg::PrefsMsg;
use crate::msg::{FolderPurpose, Msg};
use crate::state::clone::{CloneForm, LimitKind};
use crate::state::clone_service::CloneSource;
use crate::ui::theme::controls;
use crate::ui::widgets::form_message::{self, Message};
use chrono::NaiveDate;
use eframe::egui::{self, Id, RichText};
use glowfly_core::forge::Connection;
use std::path::Path;

pub(super) fn note(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).color(color::weak()));
}

/// The folder the clone creates, after a right arrow. The arrow is painted: the bundled fonts
/// lack the glyph.
pub(super) fn destination(ui: &mut egui::Ui, path: &str) {
    ui.horizontal(|ui| {
        let h = ui.text_style_height(&egui::TextStyle::Body);
        let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, h), egui::Sense::hover());
        let (c, s) = (rect.center(), egui::Stroke::new(1.2_f32, color::weak()));
        let tip = egui::pos2(c.x + 5.0, c.y);
        ui.painter().line_segment([egui::pos2(c.x - 5.0, c.y), tip], s);
        ui.painter().line_segment([egui::pos2(c.x + 1.0, c.y - 4.0), tip], s);
        ui.painter().line_segment([egui::pos2(c.x + 1.0, c.y + 4.0), tip], s);
        ui.label(RichText::new(path).color(color::weak()));
    });
}

/// A short single-line field in `group` (its label above, its help or problem under it).
fn short_field(ui: &mut egui::Ui, group: FieldGroup<'_>, text: &mut String, id: &str, hint: &str, width: f32) -> egui::Response {
    group.show(ui, |ui| ui.add(TextField::new(text).id(Id::new(id)).hint(hint).width(width)))
}

/// The URL tab and the Your repositories tab.
fn tabs(ui: &mut egui::Ui, form: &CloneForm, msgs: &mut Vec<Msg>) {
    ui.horizontal(|ui| {
        for (source, label) in [(CloneSource::Url, "URL"), (CloneSource::Service, "Your repositories")] {
            if ui.selectable_label(form.source == source, label).clicked() && form.source != source {
                msgs.push(Msg::Clone(CloneMsg::Source(source)));
            }
        }
    });
}

fn shallow_options(ui: &mut egui::Ui, form: &mut CloneForm, today: NaiveDate) {
    controls::checkbox(ui, &mut form.shallow, "Shallow clone (download only recent history)");
    if !form.shallow {
        return;
    }
    ui.indent("glowfly-clone-shallow", |ui| {
        ui.horizontal(|ui| {
            ui.label("History to download");
            controls::radio_value(ui, &mut form.limit, LimitKind::Depth, "Latest commits");
            controls::radio_value(ui, &mut form.limit, LimitKind::Since, "Commits since a date");
        });
        let problems = form.problems_now(today);
        let (key, r) = match form.limit {
            LimitKind::Depth => {
                let help = "How many of the newest commits on the branch to download.";
                let group = FieldGroup::new("Commits").help(help).message(&form.touched, "depth", problems.depth.map(Message::error));
                ("depth", short_field(ui, group, &mut form.depth, "glowfly-clone-depth", "", 90.0))
            }
            LimitKind::Since => {
                let help = "Commits made before this day are left out.";
                let group = FieldGroup::new("Since").help(help).message(&form.touched, "since", problems.since.map(Message::error));
                ("since", short_field(ui, group, &mut form.since, "glowfly-clone-since", "YYYY-MM-DD", 130.0))
            }
        };
        form_message::track(&mut form.touched, key, &r);
        let help = "Only this branch is downloaded. Leave it empty for the remote's default branch.";
        let group = FieldGroup::new("Branch").help(help).message(&form.touched, "branch", problems.branch.map(Message::error));
        let r = short_field(ui, group, &mut form.branch, "glowfly-clone-branch", "Remote's default branch", 240.0);
        form_message::track(&mut form.touched, "branch", &r);
    });
}

/// The sparse checkout option; `blocked` says why this git can't do it.
fn sparse_options(ui: &mut egui::Ui, form: &mut CloneForm, today: NaiveDate, blocked: Option<&str>) {
    let check = ui.add_enabled(blocked.is_none(), controls::Check::new(&mut form.sparse, "Sparse checkout (check out only some folders)"));
    if let Some(why) = blocked {
        check.on_disabled_hover_text(why);
    }
    if !form.sparse {
        return;
    }
    ui.indent("glowfly-clone-sparse-options", |ui| {
        let shown = form.problems_now(today).sparse.map(Message::error);
        let group = FieldGroup::new("Folders to check out, one per line")
            .help("Files at the top of the repository are always checked out. Leave the list empty to check out only those.")
            .message(&form.touched, "sparse", shown);
        let r = group.show(ui, |ui| ui.add(TextField::multiline(&mut form.sparse_folders, 4).id(Id::new("glowfly-clone-sparse"))));
        form_message::track(&mut form.touched, "sparse", &r);
        controls::checkbox(ui, &mut form.partial, "Download file contents only for these folders (partial clone)");
        note(ui, "Faster for large repositories. Glowfly can't show changes to files that weren't downloaded.");
    });
}

/// The Clone dialog; returns true when it should close. `today` is the local date, the latest
/// start date a shallow clone accepts; `sparse_blocked` says why sparse checkout is off, if it
/// is. The repository filter takes the focus only when `cx.keyboard` (no credential prompt is
/// open).
pub fn show(
    ui: &mut egui::Ui,
    cx: Cx,
    form: &mut CloneForm,
    today: NaiveDate,
    sparse_blocked: Option<&str>,
    connections: &[Connection],
    msgs: &mut Vec<Msg>,
) -> bool {
    let mut d = Dialog::new(cx, "Clone a repository").size(Size::Wide).begin(ui);
    let mut close = d.body(ui, |ui| body(ui, cx, form, today, sparse_blocked, connections, msgs));
    let p = form.problems_now(today);
    let out = d.finish(ui, Footer::new().cancel().primary(buttons::primary("Clone").enabled(!p.blocked)));
    if out.attempted {
        form.touched.attempt();
    }
    if out.primary
        && let Some(req) = form.request(today)
    {
        msgs.push(Msg::Clone(CloneMsg::Start(req)));
        close = true;
    }
    close || out.dismiss
}

/// The tabs, the URL or the repository list, the folder and the options; returns true when
/// Open Integrations closes the dialog.
fn body(
    ui: &mut egui::Ui,
    cx: Cx,
    form: &mut CloneForm,
    today: NaiveDate,
    sparse_blocked: Option<&str>,
    connections: &[Connection],
    msgs: &mut Vec<Msg>,
) -> bool {
    let mut close = false;
    tabs(ui, form, msgs);
    ui.add_space(4.0);
    ui.add_space(8.0);
    match form.source {
        CloneSource::Url => {
            // The folder check doesn't affect the URL's line.
            let shown = form.problems(today, true).url.map(Message::error);
            let group = FieldGroup::new("Repository URL").message(&form.touched, "url", shown);
            let hint = "https://github.com/owner/repo.git";
            let url = group.show(ui, |ui| ui.add(TextField::new(&mut form.url).id(Id::new("glowfly-clone-url")).hint(hint)));
            form_message::track(&mut form.touched, "url", &url);
        }
        CloneSource::Service => close |= super::clone_service::show(ui, cx.t, form, connections, cx.keyboard, msgs),
    }
    let shown = form.problems_now(today).folder.map(Message::error);
    let mut browse = false;
    let edit = FieldGroup::new("Clone into folder").message(&form.touched, "folder", shown).show(ui, |ui| {
        let field = TextField::new(&mut form.parent).id(Id::new("glowfly-clone-parent")).hint("Folder the clone goes in");
        let (r, b) = text_field::with_button(ui, field, buttons::secondary("Browse…"));
        browse = b.clicked();
        r
    });
    form_message::track(&mut form.touched, "folder", &edit);
    if edit.changed() {
        msgs.push(Msg::Clone(CloneMsg::ParentEdited));
    }
    if browse {
        form.touched.mark("folder");
        msgs.push(Msg::Prefs(PrefsMsg::PickFolder(FolderPurpose::CloneParent)));
    }
    let name = form.name();
    if !name.is_empty() && !form.parent.trim().is_empty() {
        destination(ui, &Path::new(form.parent.trim()).join(&name).display().to_string());
    }
    ui.add_space(6.0);
    shallow_options(ui, form, today);
    sparse_options(ui, form, today, sparse_blocked);
    close
}
