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
use crate::state::branch_sync::{RenameForm, remote_rename};
use crate::state::touched::Touched;
use crate::ui::theme::controls;
use eframe::egui;
use glowfly_core::branch_ops::{NameProblem, branch_name_warning, check_branch_name, create_tracking_cmd, is_valid_branch_name};
use glowfly_core::gitcli::args;
use glowfly_core::repo::{BranchInfo, Snapshot};

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
/// checkout box, and which fields were edited. With `track` (remote, branch) it starts at that
/// remote branch instead and always tracks it, which the name field's note says.
pub struct NewBranchForm<'a> {
    pub start: Option<&'a str>,
    pub name: &'a mut String,
    pub checkout: &'a mut bool,
    pub track: Option<&'a (String, String)>,
    pub touched: &'a mut Touched,
}

/// The new branch dialog; returns true when it should close. Its field keeps the keyboard focus
/// unless `cx.keyboard` is false.
pub fn new_branch(ui: &mut egui::Ui, cx: Cx, f: NewBranchForm, snap: Option<&Snapshot>, msgs: &mut Vec<Msg>) -> bool {
    let t = cx.t;
    let upstream = f.track.map(|(remote, name)| format!("{remote}/{name}"));
    let heading = match (&upstream, f.start, snap.and_then(|s| s.head_branch.as_deref())) {
        (Some(u), ..) => format!("New branch from {u}"),
        (None, Some(s), _) => format!("New branch at {}", &s[..7.min(s.len())]),
        (None, None, Some(b)) => format!("New branch from {b}"),
        (None, None, None) => "New branch at HEAD".to_owned(),
    };
    let d = Dialog::new(cx, heading).begin(ui);
    let locals = locals(snap);
    let shown = name_message(f.name.trim(), check_branch_name(f.name.trim(), &locals, None).err().as_ref());
    let note = upstream.as_deref().map(|u| Message::info(t.trf("branch.new.tracks_note", &[("upstream", &u)])));
    let group = FieldGroup::new("Branch name").message(f.touched, NAME, shown).note(note);
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
        let cmd = match f.track {
            Some((remote, name)) => create_tracking_cmd(new, remote, name, *f.checkout),
            None => {
                let mut cmd = if *f.checkout { args(&["checkout", "-b", new]) } else { args(&["branch", new]) };
                cmd.extend(f.start.map(str::to_owned));
                cmd
            }
        };
        msgs.push(Msg::Job(JobMsg::Git { label: "Create branch".into(), cmds: vec![cmd] }));
    }
    out.primary || out.dismiss
}

/// The rename dialog (`form`); returns true when it should close. For a branch with an upstream
/// on a remote it offers to rename it there too, or says why it can't (`remote_rename`). Its
/// field keeps the keyboard focus unless `cx.keyboard` is false (a credential prompt is open
/// above it).
pub fn rename(ui: &mut egui::Ui, cx: Cx, form: &mut RenameForm, snap: Option<&Snapshot>, msgs: &mut Vec<Msg>) -> bool {
    let (t, old) = (cx.t, form.old.clone());
    let d = Dialog::new(cx, format!("Rename branch {old}")).begin(ui);
    let locals = locals(snap);
    let problem_of = |new: &str| if new == old { None } else { check_branch_name(new, &locals, Some(&old)).err() };
    let shown = Some(form.name.trim()).filter(|n| *n != old).and_then(|n| name_message(n, problem_of(n).as_ref()));
    let b = branch(snap, &old);
    let tracking = b.and_then(|b| b.tracking.as_ref()).filter(|t| t.is_remote());
    let offer = snap.zip(b).and_then(|(s, b)| remote_rename(t, s, b, form.name.trim()));
    let remote = matches!(offer, Some(Ok(_))) && form.remote;
    let note = tracking.filter(|_| !remote).map(|u| Message::info(t.trf("branch_sync.rename.local_note", &[("upstream", &u.display())])));
    let group = FieldGroup::new("Branch name").message(&form.touched, NAME, shown).note(note);
    let r = group.show(ui, |ui| ui.add(TextField::new(&mut form.name).hint("branch-name")));
    form_message::track(&mut form.touched, NAME, &r);
    let submit = text_field::entered(ui, &r);
    if cx.keyboard {
        r.request_focus();
    }
    if let (Some(offer), Some(u)) = (&offer, tracking) {
        ui.add_space(8.0);
        let label = t.trf("branch_sync.rename.remote.check", &[("remote", &u.remote)]);
        // A tick stays clearable after the name typed makes it impossible.
        ui.add_enabled(offer.is_ok() || form.remote, controls::Check::new(&mut form.remote, label));
        let text = match offer {
            Ok(p) if form.remote => {
                Some(Message::info(t.trf("branch_sync.rename.remote_note", &[("remote", &p.remote), ("upstream", &p.upstream())])))
            }
            Ok(_) => None,
            Err(why) if form.remote => Some(Message::warning(why.clone())),
            Err(why) => Some(Message::info(why.clone())),
        };
        if let Some(m) = text {
            ui.add_space(4.0);
            form_message::form(ui, m);
        }
    }
    let new = form.name.trim();
    let unchanged = new == old;
    let problem = problem_of(new);
    // Ticked but refused: renaming only here would not be what was asked.
    let refused = form.remote && matches!(offer, Some(Err(_)));
    let valid = !unchanged && problem.is_none() && !refused;
    let out = d.finish(ui, Footer::new().cancel().primary(buttons::primary("Rename").enabled(valid)).submit(submit));
    if out.attempted && !unchanged {
        form.touched.attempt();
    }
    if out.primary {
        msgs.push(Msg::Branch(BranchMsg::RenameBranch { old: old.clone(), new: new.to_owned(), remote }));
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
