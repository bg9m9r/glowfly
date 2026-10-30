//! The New group and Rename group dialog (`Modal::RepoGroup`): one Name field, focused when it
//! opens. Create or Rename stays off while the name is empty or another group has it (which a
//! message under the field says). From a row's Move to group > New group…, the subtitle names
//! the repository that moves into the new group. `update` cleans the name again.

use super::dialog_frame::{Cx, Dialog, Footer, Size};
use super::widgets::form_message::Message;
use super::widgets::text_field::{self, TextField};
use super::widgets::{FieldGroup, buttons};
use crate::msg::{GroupMsg, Msg};
use crate::state::repo_groups::GroupForm;
use eframe::egui;
use glowfly_core::settings::MAX_GROUP_NAME;

/// Draws the dialog; returns true when it should close.
pub fn show(ui: &mut egui::Ui, cx: Cx, form: &mut GroupForm, msgs: &mut Vec<Msg>) -> bool {
    let t = cx.t;
    let title = if form.group.is_some() { t.tr("repo_group.rename.title") } else { t.tr("repo_group.new.title") };
    let mut d = Dialog::new(cx, title).size(Size::Small);
    if let Some(p) = &form.repo {
        d = d.subtitle(glowfly_core::native_path::shown(p));
    }
    let d = d.begin(ui);
    let problem = form.problem(t);
    let taken = problem.as_deref().filter(|p| !p.is_empty()).map(Message::error);
    let field = TextField::new(&mut form.text).hint(t.tr("repo_group.name.hint")).char_limit(MAX_GROUP_NAME);
    let r = FieldGroup::new(t.tr("repo_group.name.label")).note(taken).show(ui, |ui| ui.add(field));
    // Read before focus is taken back, or the Enter that just left the field is missed.
    let submit = text_field::entered(ui, &r) && problem.is_none();
    if cx.keyboard && ui.memory(|m| m.focused().is_none()) {
        r.request_focus();
    }
    let button = if form.group.is_some() { t.tr("repo_group.rename.button") } else { t.tr("repo_group.create.button") };
    let footer = Footer::new().cancel().primary(buttons::primary(button).enabled(problem.is_none())).submit(submit);
    let out = d.finish(ui, footer);
    if out.primary {
        let name = form.name();
        msgs.push(Msg::Groups(match form.group {
            Some(group) => GroupMsg::Rename { group, name },
            None => GroupMsg::Create { name, repo: form.repo.clone() },
        }));
    }
    out.dismiss
}
