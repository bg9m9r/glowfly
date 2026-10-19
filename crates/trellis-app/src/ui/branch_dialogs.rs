//! The new branch, rename and upstream dialogs of a local branch. Names are checked live with
//! the pure helpers in `branch_ops`; `update` (or git itself, for a new branch) checks them
//! again before anything changes.

use super::dialog_frame::{Cx, Dialog, Footer};
use super::theme::color;
use super::widgets::buttons;
use crate::msg::BranchMsg;
use crate::msg::{JobMsg, Msg};
use crate::ui::theme::controls;
use eframe::egui::{self, Key, RichText};
use trellis_core::branch_ops::{NameProblem, branch_name_warning, check_branch_name, is_valid_branch_name};
use trellis_core::gitcli::args;
use trellis_core::repo::{BranchInfo, Snapshot};

fn branch<'a>(snap: Option<&'a Snapshot>, name: &str) -> Option<&'a BranchInfo> {
    snap?.locals.iter().find(|b| b.name == name)
}

fn locals(snap: Option<&Snapshot>) -> Vec<&str> {
    snap.map(|s| s.locals.iter().map(|b| b.name.as_str()).collect()).unwrap_or_default()
}

/// The red problem line for `name` (none for an empty name unless `empty_too`), or else the
/// amber warning that Windows can't store it.
fn name_lines(ui: &mut egui::Ui, name: &str, problem: Option<&NameProblem>, empty_too: bool) {
    let line = match problem {
        Some(NameProblem::Empty) if !empty_too => None,
        Some(p) => Some(RichText::new(p.message(name)).color(color::red())),
        None => branch_name_warning(name).map(|w| RichText::new(w).color(color::amber())),
    };
    if let Some(line) = line {
        ui.add_space(4.0);
        ui.label(line);
    }
}

/// The new branch dialog, at `start` (a commit) or HEAD; returns true when it should close. Its
/// field keeps the keyboard focus unless `cx.keyboard` is false.
pub fn new_branch(
    ui: &mut egui::Ui,
    cx: Cx,
    start: Option<&str>,
    name: &mut String,
    checkout: &mut bool,
    snap: Option<&Snapshot>,
    msgs: &mut Vec<Msg>,
) -> bool {
    let heading = match (start, snap.and_then(|s| s.head_branch.as_deref())) {
        (Some(s), _) => format!("New branch at {}", &s[..7.min(s.len())]),
        (None, Some(b)) => format!("New branch from {b}"),
        (None, None) => "New branch at HEAD".to_owned(),
    };
    let d = Dialog::new(cx, heading).begin(ui);
    let r = controls::field(ui, egui::TextEdit::singleline(name).hint_text("branch-name").desired_width(f32::INFINITY));
    super::access::name_field(&r, "Branch name");
    if cx.keyboard {
        r.request_focus();
    }
    let new = name.trim();
    let problem = check_branch_name(new, &locals(snap), None).err();
    name_lines(ui, new, problem.as_ref(), false);
    controls::checkbox(ui, checkout, "Check out after creating");
    let submit = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
    let out = d.finish(ui, Footer::new().cancel().primary(buttons::primary("Create").enabled(problem.is_none())).submit(submit));
    if out.primary {
        let mut cmd = if *checkout { args(&["checkout", "-b", new]) } else { args(&["branch", new]) };
        if let Some(s) = start {
            cmd.push(s.to_owned());
        }
        msgs.push(Msg::Job(JobMsg::Git { label: "Create branch".into(), cmds: vec![cmd] }));
    }
    out.primary || out.dismiss
}

/// The rename dialog; returns true when it should close. Its field keeps the keyboard focus
/// unless `cx.keyboard` is false (a credential prompt is open above it).
pub fn rename(ui: &mut egui::Ui, cx: Cx, old: &str, name: &mut String, snap: Option<&Snapshot>, msgs: &mut Vec<Msg>) -> bool {
    let d = Dialog::new(cx, format!("Rename branch {old}")).begin(ui);
    let r = controls::field(ui, egui::TextEdit::singleline(name).hint_text("branch-name").desired_width(f32::INFINITY));
    if cx.keyboard {
        r.request_focus();
    }
    let new = name.trim();
    let unchanged = new == old;
    let problem = if unchanged { None } else { check_branch_name(new, &locals(snap), Some(old)).err() };
    if !unchanged {
        name_lines(ui, new, problem.as_ref(), true);
    }
    if let Some(t) = branch(snap, old).and_then(|b| b.tracking.as_ref()).filter(|t| t.is_remote()) {
        ui.add_space(4.0);
        ui.label(RichText::new(format!("Its upstream setting moves with it; {} is not renamed.", t.display())).color(color::weak()));
    }
    let valid = !unchanged && problem.is_none();
    let submit = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
    let out = d.finish(ui, Footer::new().cancel().primary(buttons::primary("Rename").enabled(valid)).submit(submit));
    if out.primary {
        msgs.push(Msg::Branch(BranchMsg::RenameBranch { old: old.to_owned(), new: new.to_owned() }));
    }
    out.primary || out.dismiss
}

/// The upstream dialog: which remote branch `branch` pulls from and pushes to. Returns true when
/// it should close.
pub fn upstream(
    ui: &mut egui::Ui,
    cx: Cx,
    branch_name: &str,
    remote: &mut String,
    name: &mut String,
    snap: Option<&Snapshot>,
    msgs: &mut Vec<Msg>,
) -> bool {
    let d = Dialog::new(cx, format!("Upstream for {branch_name}")).subtitle("Pull from and push to:").begin(ui);
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
        controls::field(ui, egui::TextEdit::singleline(name).hint_text("branch-name").desired_width(300.0));
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
    let save = buttons::primary("Save").enabled(valid && !unchanged && !remote.is_empty());
    let out = d.finish(ui, Footer::new().cancel().primary(save));
    if out.primary {
        msgs.push(Msg::Branch(BranchMsg::SetUpstream { branch: branch_name.to_owned(), remote: remote.clone(), name: target.to_owned() }));
    }
    out.primary || out.dismiss
}
