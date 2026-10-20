//! The new branch, rename and upstream dialogs of a local branch. Names are checked live with
//! the pure helpers in `branch_ops`; `update` (or git itself, for a new branch) checks them
//! again before anything changes. A name's problem shows under its field once the field was
//! edited or a submit was tried (`form_message`).

use super::dialog_frame::{Cx, Dialog, Footer};
use super::widgets::form_message::{self, Message};
use super::widgets::text_field::{self, TextField};
use super::widgets::{FieldGroup, buttons};
use crate::msg::BranchMsg;
use crate::msg::{JobMsg, Msg};
use crate::state::touched::Touched;
use crate::ui::theme::controls;
use eframe::egui;
use trellis_core::branch_ops::{NameProblem, branch_name_warning, check_branch_name, is_valid_branch_name};
use trellis_core::gitcli::args;
use trellis_core::repo::{BranchInfo, Snapshot};

/// The name field's key in `Touched`.
const NAME: &str = "name";

fn branch<'a>(snap: Option<&'a Snapshot>, name: &str) -> Option<&'a BranchInfo> {
    snap?.locals.iter().find(|b| b.name == name)
}

fn locals(snap: Option<&Snapshot>) -> Vec<&str> {
    snap.map(|s| s.locals.iter().map(|b| b.name.as_str()).collect()).unwrap_or_default()
}

/// The error for `name`'s problem, or else the warning that Windows can't store it.
fn name_message(name: &str, problem: Option<&NameProblem>) -> Option<Message<'static>> {
    match problem {
        Some(p) => Some(Message::error(p.message(name))),
        None => branch_name_warning(name).map(Message::warning),
    }
}

/// What the New branch dialog edits: the commit it starts at (HEAD when None), the name, the
/// checkout box, and which fields were edited.
pub struct NewBranchForm<'a> {
    pub start: Option<&'a str>,
    pub name: &'a mut String,
    pub checkout: &'a mut bool,
    pub touched: &'a mut Touched,
}

/// The new branch dialog; returns true when it should close. Its field keeps the keyboard focus
/// unless `cx.keyboard` is false.
pub fn new_branch(ui: &mut egui::Ui, cx: Cx, f: NewBranchForm, snap: Option<&Snapshot>, msgs: &mut Vec<Msg>) -> bool {
    let heading = match (f.start, snap.and_then(|s| s.head_branch.as_deref())) {
        (Some(s), _) => format!("New branch at {}", &s[..7.min(s.len())]),
        (None, Some(b)) => format!("New branch from {b}"),
        (None, None) => "New branch at HEAD".to_owned(),
    };
    let d = Dialog::new(cx, heading).begin(ui);
    let locals = locals(snap);
    let shown = name_message(f.name.trim(), check_branch_name(f.name.trim(), &locals, None).err().as_ref());
    let group = FieldGroup::new("Branch name").message(f.touched, NAME, shown);
    let r = group.show(ui, |ui| ui.add(TextField::new(f.name).hint("branch-name")));
    form_message::track(f.touched, NAME, &r);
    let submit = text_field::entered(ui, &r);
    if cx.keyboard {
        r.request_focus();
    }
    let new = f.name.trim();
    let problem = check_branch_name(new, &locals, None).err();
    controls::checkbox(ui, f.checkout, "Check out after creating");
    let out = d.finish(ui, Footer::new().cancel().primary(buttons::primary("Create").enabled(problem.is_none())).submit(submit));
    if out.attempted {
        f.touched.attempt();
    }
    if out.primary {
        let mut cmd = if *f.checkout { args(&["checkout", "-b", new]) } else { args(&["branch", new]) };
        if let Some(s) = f.start {
            cmd.push(s.to_owned());
        }
        msgs.push(Msg::Job(JobMsg::Git { label: "Create branch".into(), cmds: vec![cmd] }));
    }
    out.primary || out.dismiss
}

/// The rename dialog for `old`, editing `(name, touched)`; returns true when it should close.
/// Its field keeps the keyboard focus unless `cx.keyboard` is false (a credential prompt is open
/// above it).
pub fn rename(
    ui: &mut egui::Ui,
    cx: Cx,
    old: &str,
    (name, touched): (&mut String, &mut Touched),
    snap: Option<&Snapshot>,
    msgs: &mut Vec<Msg>,
) -> bool {
    let d = Dialog::new(cx, format!("Rename branch {old}")).begin(ui);
    let locals = locals(snap);
    let problem_of = |new: &str| if new == old { None } else { check_branch_name(new, &locals, Some(old)).err() };
    let shown = Some(name.trim()).filter(|n| *n != old).and_then(|n| name_message(n, problem_of(n).as_ref()));
    let tracking = branch(snap, old).and_then(|b| b.tracking.as_ref()).filter(|t| t.is_remote());
    let note = tracking.map(|t| Message::info(format!("Its upstream setting moves with it; {} is not renamed.", t.display())));
    let group = FieldGroup::new("Branch name").message(touched, NAME, shown).note(note);
    let r = group.show(ui, |ui| ui.add(TextField::new(name).hint("branch-name")));
    form_message::track(touched, NAME, &r);
    let submit = text_field::entered(ui, &r);
    if cx.keyboard {
        r.request_focus();
    }
    let new = name.trim();
    let unchanged = new == old;
    let problem = problem_of(new);
    let valid = !unchanged && problem.is_none();
    let out = d.finish(ui, Footer::new().cancel().primary(buttons::primary("Rename").enabled(valid)).submit(submit));
    if out.attempted && !unchanged {
        touched.attempt();
    }
    if out.primary {
        msgs.push(Msg::Branch(BranchMsg::RenameBranch { old: old.to_owned(), new: new.to_owned() }));
    }
    out.primary || out.dismiss
}

/// The upstream dialog: which remote branch `branch` pulls from and pushes to, editing
/// `(remote, name, touched)`. Returns true when it should close.
pub fn upstream(
    ui: &mut egui::Ui,
    cx: Cx,
    branch_name: &str,
    (remote, name, touched): (&mut String, &mut String, &mut Touched),
    snap: Option<&Snapshot>,
    msgs: &mut Vec<Msg>,
) -> bool {
    let d = Dialog::new(cx, format!("Upstream for {branch_name}")).subtitle("Pull from and push to:").begin(ui);
    let remotes: &[(String, Vec<(String, git2::Oid)>)] = snap.map(|s| s.remotes.as_slice()).unwrap_or_default();
    FieldGroup::new("Remote").show(ui, |ui| {
        let combo = egui::ComboBox::from_id_salt("upstream-remote").selected_text(remote.as_str()).width(ui.available_width());
        combo
            .show_ui(ui, |ui| {
                for (r, _) in remotes {
                    ui.selectable_value(remote, r.clone(), r);
                }
            })
            .response
    });
    let status = {
        let target = name.trim();
        let exists = remotes.iter().any(|(r, b)| *r == *remote && b.iter().any(|(n, _)| n == target));
        if !is_valid_branch_name(target) {
            Message::error("Not a valid branch name.")
        } else if exists {
            Message::info(format!("Tracks {remote}/{target}."))
        } else {
            Message::info(format!("{remote}/{target} doesn't exist yet. Push {branch_name} to create it."))
        }
    };
    let r = FieldGroup::new("Branch").message(touched, NAME, Some(status)).show(ui, |ui| ui.add(TextField::new(name).hint("branch-name")));
    form_message::track(touched, NAME, &r);
    let target = name.trim();
    let valid = is_valid_branch_name(target);
    let current = branch(snap, branch_name).and_then(|b| b.tracking.as_ref());
    let unchanged = current.is_some_and(|t| t.remote == *remote && t.merge == target);
    let save = buttons::primary("Save").enabled(valid && !unchanged && !remote.is_empty());
    let out = d.finish(ui, Footer::new().cancel().primary(save));
    if out.attempted {
        touched.attempt();
    }
    if out.primary {
        msgs.push(Msg::Branch(BranchMsg::SetUpstream { branch: branch_name.to_owned(), remote: remote.clone(), name: target.to_owned() }));
    }
    out.primary || out.dismiss
}
