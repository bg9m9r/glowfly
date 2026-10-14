//! The Clone dialog: the URL, the folder to clone into and the shallow clone options, checked as
//! the user types with the pure rules in `state::clone`.

use super::theme::color;
use crate::msg::{FolderPurpose, Msg};
use crate::state::clone::{CloneForm, LimitKind};
use chrono::NaiveDate;
use eframe::egui::{self, Id, RichText};
use std::path::Path;

/// A field's problem, in red under it.
fn problem(ui: &mut egui::Ui, text: Option<&String>) {
    if let Some(t) = text {
        ui.label(RichText::new(t.as_str()).color(color::red()));
    }
}

fn note(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).color(color::weak()));
}

/// The folder the clone creates, after a right arrow. The arrow is painted: the bundled fonts
/// lack the glyph.
fn destination(ui: &mut egui::Ui, path: &str) {
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

/// A label and a single-line text field on one row.
fn row(ui: &mut egui::Ui, label: &str, text: &mut String, id: &str, hint: &str, width: f32) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::TextEdit::singleline(text).id(Id::new(id)).hint_text(hint).desired_width(width));
    });
}

/// The Clone dialog; returns true when it should close. `today` is the local date, the latest
/// start date a shallow clone accepts.
pub fn show(ui: &mut egui::Ui, form: &mut CloneForm, today: NaiveDate, msgs: &mut Vec<Msg>) -> bool {
    let mut close = false;
    ui.heading("Clone a repository");
    ui.add_space(6.0);
    ui.add(
        egui::TextEdit::singleline(&mut form.url)
            .id(Id::new("trellis-clone-url"))
            .hint_text("https://github.com/owner/repo.git")
            .desired_width(f32::INFINITY),
    );
    // Each line is checked right after its field is drawn, so it follows this frame's typing.
    // The folder check doesn't affect the URL's line.
    problem(ui, form.problems(today, true).url.as_ref());
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut form.parent)
                .id(Id::new("trellis-clone-parent"))
                .hint_text("Clone into folder")
                .desired_width(340.0),
        );
        if ui.button("Browse…").clicked() {
            msgs.push(Msg::PickFolder(FolderPurpose::CloneParent));
        }
    });
    let parent_is_dir = Path::new(form.parent.trim()).is_dir();
    problem(ui, form.problems(today, parent_is_dir).folder.as_ref());
    let name = form.name();
    if !name.is_empty() && !form.parent.trim().is_empty() {
        destination(ui, &Path::new(form.parent.trim()).join(&name).display().to_string());
    }
    ui.add_space(6.0);
    ui.checkbox(&mut form.shallow, "Shallow clone (download only recent history)");
    if form.shallow {
        ui.indent("trellis-clone-shallow", |ui| {
            ui.horizontal(|ui| {
                ui.label("History to download");
                ui.radio_value(&mut form.limit, LimitKind::Depth, "Latest commits");
                ui.radio_value(&mut form.limit, LimitKind::Since, "Commits since a date");
            });
            match form.limit {
                LimitKind::Depth => {
                    row(ui, "Commits", &mut form.depth, "trellis-clone-depth", "", 70.0);
                    problem(ui, form.problems(today, parent_is_dir).depth.as_ref());
                    note(ui, "How many of the newest commits on the branch to download.");
                }
                LimitKind::Since => {
                    row(ui, "Since", &mut form.since, "trellis-clone-since", "YYYY-MM-DD", 110.0);
                    problem(ui, form.problems(today, parent_is_dir).since.as_ref());
                    note(ui, "Commits made before this day are left out.");
                }
            }
            ui.add_space(4.0);
            row(ui, "Branch", &mut form.branch, "trellis-clone-branch", "Remote's default branch", 220.0);
            problem(ui, form.problems(today, parent_is_dir).branch.as_ref());
            note(ui, "Only this branch is downloaded. Leave it empty for the remote's default branch.");
        });
    }
    ui.add_space(8.0);
    let p = form.problems(today, parent_is_dir);
    ui.horizontal(|ui| {
        if ui.add_enabled(!p.blocked, egui::Button::new("Clone")).clicked()
            && let Some(req) = form.request(today)
        {
            msgs.push(Msg::CloneRepo(req));
            close = true;
        }
        if ui.button("Cancel").clicked() {
            close = true;
        }
    });
    close
}
