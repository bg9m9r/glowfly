//! The rename and upstream dialogs of a local branch. Names are checked live with the pure
//! helpers in `branch_ops`; `update` checks them again before running anything.

use super::theme::color;
use crate::msg::BranchMsg;
use crate::msg::Msg;
use eframe::egui::{self, Key, RichText};
use trellis_core::branch_ops::{check_branch_name, is_valid_branch_name};
use trellis_core::repo::{BranchInfo, Snapshot};

fn branch<'a>(snap: Option<&'a Snapshot>, name: &str) -> Option<&'a BranchInfo> {
    snap?.locals.iter().find(|b| b.name == name)
}

/// The rename dialog; returns true when it should close. Its field keeps the keyboard focus
/// unless `focus` is false (a credential prompt is open above it).
pub fn rename(ui: &mut egui::Ui, old: &str, name: &mut String, snap: Option<&Snapshot>, focus: bool, msgs: &mut Vec<Msg>) -> bool {
    let mut close = false;
    ui.heading(format!("Rename branch {old}"));
    ui.add_space(6.0);
    let r = ui.add(egui::TextEdit::singleline(name).hint_text("branch-name").desired_width(f32::INFINITY));
    if focus {
        r.request_focus();
    }
    let new = name.trim();
    let unchanged = new == old;
    let existing: Vec<&str> = snap.map(|s| s.locals.iter().map(|b| b.name.as_str()).collect()).unwrap_or_default();
    let problem = if unchanged { None } else { check_branch_name(new, &existing, Some(old)).err() };
    if let Some(p) = &problem {
        ui.add_space(4.0);
        ui.label(RichText::new(p.message(new)).color(color::red()));
    }
    if let Some(t) = branch(snap, old).and_then(|b| b.tracking.as_ref()).filter(|t| t.is_remote()) {
        ui.add_space(4.0);
        ui.label(RichText::new(format!("Its upstream setting moves with it; {} is not renamed.", t.display())).color(color::weak()));
    }
    ui.add_space(10.0);
    let valid = !unchanged && problem.is_none();
    let submit = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
    ui.horizontal(|ui| {
        if ui.add_enabled(valid, egui::Button::new("Rename")).clicked() || (submit && valid) {
            msgs.push(Msg::Branch(BranchMsg::RenameBranch { old: old.to_owned(), new: new.to_owned() }));
            close = true;
        }
        if ui.button("Cancel").clicked() {
            close = true;
        }
    });
    close
}

/// The upstream dialog: which remote branch `branch` pulls from and pushes to. Returns true when
/// it should close.
pub fn upstream(
    ui: &mut egui::Ui,
    branch_name: &str,
    remote: &mut String,
    name: &mut String,
    snap: Option<&Snapshot>,
    msgs: &mut Vec<Msg>,
) -> bool {
    let mut close = false;
    ui.heading(format!("Upstream for {branch_name}"));
    ui.label(RichText::new("Pull from and push to:").color(color::weak()));
    ui.add_space(8.0);
    let remotes: &[(String, Vec<(String, git2::Oid)>)] = snap.map(|s| s.remotes.as_slice()).unwrap_or_default();
    egui::Grid::new("upstream-form").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
        ui.label("Remote");
        egui::ComboBox::from_id_salt("upstream-remote").selected_text(remote.as_str()).width(200.0).show_ui(ui, |ui| {
            for (r, _) in remotes {
                ui.selectable_value(remote, r.clone(), r);
            }
        });
        ui.end_row();
        ui.label("Branch");
        ui.add(egui::TextEdit::singleline(name).hint_text("branch-name").desired_width(300.0));
        ui.end_row();
    });
    let target = name.trim();
    let valid = is_valid_branch_name(target);
    let exists = remotes.iter().any(|(r, b)| *r == *remote && b.iter().any(|(n, _)| n == target));
    ui.add_space(6.0);
    let status = if !valid {
        RichText::new("Not a valid branch name.").color(color::red())
    } else if exists {
        RichText::new(format!("Tracks {remote}/{target}.")).color(color::weak())
    } else {
        RichText::new(format!("{remote}/{target} doesn't exist yet. Push {branch_name} to create it.")).color(color::amber())
    };
    ui.label(status);
    let current = branch(snap, branch_name).and_then(|b| b.tracking.as_ref());
    let unchanged = current.is_some_and(|t| t.remote == *remote && t.merge == target);
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        if ui.add_enabled(valid && !unchanged && !remote.is_empty(), egui::Button::new("Save")).clicked() {
            msgs.push(Msg::Branch(BranchMsg::SetUpstream {
                branch: branch_name.to_owned(),
                remote: remote.clone(),
                name: target.to_owned(),
            }));
            close = true;
        }
        if ui.button("Cancel").clicked() {
            close = true;
        }
    });
    close
}
