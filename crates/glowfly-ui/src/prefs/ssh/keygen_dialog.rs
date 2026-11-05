//! The Generate SSH key dialog (`Modal::SshKeygen`): where to save the key, its comment and an
//! optional passphrase, checked as they are typed. The fields live in Preferences > SSH's
//! `SshPage` (drawn from a copy, sent back as `EditMsg::SshKeygen`) and the passphrases in
//! `ViewState::secrets`, so closing the dialog keeps them; `update` runs ssh-keygen, asks
//! before replacing a key, and closes the dialog when the key is made.

use crate::dialog_frame::{Cx, Dialog, Footer};
use crate::widgets::FieldGroup;
use crate::widgets::buttons;
use crate::widgets::form_message::{self, Message};
use crate::widgets::text_field::{self, TextField};
use eframe::egui::{self, Id, Ui};
use glowfly_core::ssh_keys::default_location;
use glowfly_model::msg::Msg;
use glowfly_model::ssh::SshMsg;
use glowfly_model::state::secret_fields::SecretFields;
use glowfly_model::state::ssh::{KeygenFields, SshPage, location_text};

/// A field in its label-field-help `group`; returns its response and whether Enter left it.
fn field(ui: &mut Ui, group: FieldGroup<'_>, text: &mut String, id: &str, hint: &str, masked: bool) -> (egui::Response, bool) {
    let r = group.show(ui, |ui| ui.add(TextField::new(text).id(Id::new(id)).hint(hint).masked(masked)));
    let enter = text_field::entered(ui, &r);
    (r, enter)
}

/// Draws the dialog; returns true when Cancel or Esc closes it. The location field takes the
/// focus when nothing has it, unless the keyboard belongs to something above (`cx.keyboard`).
/// `f` is the frame's copy of the location, comment and touched state.
pub fn show(ui: &mut Ui, cx: Cx, page: &SshPage, f: &mut KeygenFields, secrets: &mut SecretFields, msgs: &mut Vec<Msg>) -> bool {
    let t = cx.t;
    let d = Dialog::new(cx, t.tr("prefs.ssh.generate.title")).subtitle(t.tr("prefs.ssh.generate.subtitle")).begin(ui);
    ui.add_space(6.0);

    let hint = page.listing.as_ref().map(|l| default_location(&l.home).display().to_string()).unwrap_or_default();
    let location = match page.location_problem() {
        Some(p) => Some(Message::error(location_text(t, p))),
        None => page.location_taken().then(|| Message::warning(t.tr("prefs.ssh.generate.location.taken.warn"))),
    };
    let group = FieldGroup::new(t.tr("prefs.ssh.generate.location.label")).message(&f.touched, "location", location);
    let (location_r, enter_location) = field(ui, group, &mut f.location, "glowfly-ssh-keygen-location", &hint, false);
    form_message::track(&mut f.touched, "location", &location_r);
    if cx.keyboard && ui.memory(|m| m.focused().is_none()) {
        location_r.request_focus();
    }

    let group = FieldGroup::new(t.tr("prefs.ssh.generate.comment.label")).help(t.tr("prefs.ssh.generate.comment.help")).message(
        &f.touched,
        "comment",
        page.comment_problem(t).map(Message::error),
    );
    let hint = t.tr("prefs.ssh.generate.comment.hint");
    let (comment_r, enter_comment) = field(ui, group, &mut f.comment, "glowfly-ssh-keygen-comment", hint, false);
    form_message::track(&mut f.touched, "comment", &comment_r);

    let group = FieldGroup::new(t.tr("prefs.ssh.generate.passphrase.label"))
        .optional(t.tr("dialog.field.optional"))
        .help(t.tr("prefs.ssh.generate.passphrase.help"));
    // The group's Optional marker says what a hint would.
    let (pass_r, enter_pass) = field(ui, group, &mut secrets.ssh_passphrase, "glowfly-ssh-keygen-passphrase", "", true);
    form_message::track(&mut f.touched, "passphrase", &pass_r);

    let mismatch = secrets.ssh_mismatch().then(|| Message::error(t.tr("prefs.ssh.generate.mismatch.warn")));
    let group = FieldGroup::new(t.tr("prefs.ssh.generate.confirm.label")).message(&f.touched, "confirm", mismatch);
    let (confirm_r, enter_confirm) = field(ui, group, &mut secrets.ssh_confirm, "glowfly-ssh-keygen-confirm", "", true);
    form_message::track(&mut f.touched, "confirm", &confirm_r);

    if let Some(e) = &page.form_error {
        form_message::form(ui, Message::error(e.as_str()));
    }

    // Only a reason that isn't a field's: the fields say theirs under them. While ssh-keygen
    // runs, the note says so instead.
    let wait = page.form_wait(t).filter(|_| !page.generating);
    let generate = buttons::primary(t.tr("prefs.ssh.generate.create.button")).enabled(page.can_generate(secrets)).why(wait);
    let enter = enter_location || enter_comment || enter_pass || enter_confirm;
    let mut footer = Footer::new().cancel().primary(generate).submit(enter);
    if page.generating {
        footer = footer.note(t.tr("prefs.ssh.generate.generating"));
    }
    let out = d.finish(ui, footer);
    if out.attempted {
        f.touched.attempt();
    }
    if out.primary {
        msgs.push(Msg::Ssh(SshMsg::Generate { overwrite: false }));
    }
    out.dismiss
}
