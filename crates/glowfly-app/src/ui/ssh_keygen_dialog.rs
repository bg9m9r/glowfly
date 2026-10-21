//! The Generate SSH key dialog (`Modal::SshKeygen`): where to save the key, its comment and an
//! optional passphrase, checked as they are typed. The fields live in Preferences > SSH's
//! `SshPage`, so closing the dialog keeps them; `update` runs ssh-keygen, asks before replacing
//! a key, and closes the dialog when the key is made.

use super::dialog_frame::{Cx, Dialog, Footer};
use super::widgets::FieldGroup;
use super::widgets::buttons;
use super::widgets::form_message::{self, Message};
use super::widgets::text_field::{self, TextField};
use crate::msg::Msg;
use crate::ssh::SshMsg;
use crate::state::ssh::{SshPage, location_text};
use eframe::egui::{self, Id, Ui};
use glowfly_core::ssh_keys::default_location;

/// A field in its label-field-help `group`; returns its response and whether Enter left it.
fn field(ui: &mut Ui, group: FieldGroup<'_>, text: &mut String, id: &str, hint: &str, password: bool) -> (egui::Response, bool) {
    let r = group.show(ui, |ui| ui.add(TextField::new(text).id(Id::new(id)).hint(hint).password(password)));
    let enter = text_field::entered(ui, &r);
    (r, enter)
}

/// Draws the dialog; returns true when Cancel or Esc closes it. The location field takes the
/// focus when nothing has it, unless the keyboard belongs to something above (`cx.keyboard`).
pub fn show(ui: &mut Ui, cx: Cx, page: &mut SshPage, msgs: &mut Vec<Msg>) -> bool {
    let t = cx.t;
    let d = Dialog::new(cx, t.tr("prefs.ssh.generate.title")).subtitle(t.tr("prefs.ssh.generate.subtitle")).begin(ui);
    ui.add_space(6.0);

    let hint = page.listing.as_ref().map(|l| default_location(&l.home).display().to_string()).unwrap_or_default();
    let location = match page.location_problem() {
        Some(p) => Some(Message::error(location_text(t, p))),
        None => page.location_taken().then(|| Message::warning(t.tr("prefs.ssh.generate.location.taken.warn"))),
    };
    let group = FieldGroup::new(t.tr("prefs.ssh.generate.location.label")).message(&page.touched, "location", location);
    let (location_r, enter_location) = field(ui, group, &mut page.location, "glowfly-ssh-keygen-location", &hint, false);
    form_message::track(&mut page.touched, "location", &location_r);
    if cx.keyboard && ui.memory(|m| m.focused().is_none()) {
        location_r.request_focus();
    }

    let group = FieldGroup::new(t.tr("prefs.ssh.generate.comment.label")).help(t.tr("prefs.ssh.generate.comment.help")).message(
        &page.touched,
        "comment",
        page.comment_problem(t).map(Message::error),
    );
    let hint = t.tr("prefs.ssh.generate.comment.hint");
    let (comment_r, enter_comment) = field(ui, group, &mut page.comment, "glowfly-ssh-keygen-comment", hint, false);
    form_message::track(&mut page.touched, "comment", &comment_r);

    let group = FieldGroup::new(t.tr("prefs.ssh.generate.passphrase.label"))
        .optional(t.tr("dialog.field.optional"))
        .help(t.tr("prefs.ssh.generate.passphrase.help"));
    // The group's Optional marker says what a hint would.
    let (pass_r, enter_pass) = field(ui, group, &mut page.passphrase, "glowfly-ssh-keygen-passphrase", "", true);
    form_message::track(&mut page.touched, "passphrase", &pass_r);

    let mismatch = page.passphrase_mismatch().then(|| Message::error(t.tr("prefs.ssh.generate.mismatch.warn")));
    let group = FieldGroup::new(t.tr("prefs.ssh.generate.confirm.label")).message(&page.touched, "confirm", mismatch);
    let (confirm_r, enter_confirm) = field(ui, group, &mut page.confirm, "glowfly-ssh-keygen-confirm", "", true);
    form_message::track(&mut page.touched, "confirm", &confirm_r);

    if let Some(e) = &page.form_error {
        form_message::form(ui, Message::error(e.as_str()));
    }

    // Only a reason that isn't a field's: the fields say theirs under them. While ssh-keygen
    // runs, the note says so instead.
    let wait = page.form_wait(t).filter(|_| !page.generating);
    let generate = buttons::primary(t.tr("prefs.ssh.generate.create.button")).enabled(page.can_generate()).why(wait);
    let enter = enter_location || enter_comment || enter_pass || enter_confirm;
    let mut footer = Footer::new().cancel().primary(generate).submit(enter);
    if page.generating {
        footer = footer.note(t.tr("prefs.ssh.generate.generating"));
    }
    let out = d.finish(ui, footer);
    if out.attempted {
        page.touched.attempt();
    }
    if out.primary {
        msgs.push(Msg::Ssh(SshMsg::Generate { overwrite: false }));
    }
    out.dismiss
}
