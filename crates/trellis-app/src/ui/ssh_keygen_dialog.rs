//! The Generate SSH key dialog (`Modal::SshKeygen`): where to save the key, its comment and an
//! optional passphrase, checked as they are typed. The fields live in Preferences > SSH's
//! `SshPage`, so closing the dialog keeps them; `update` runs ssh-keygen, asks before replacing
//! a key, and closes the dialog when the key is made.

use super::dialog_frame::{Cx, Dialog, Footer};
use super::theme::{color, controls};
use super::widgets::buttons;
use super::widgets::form_message::{self, Message};
use crate::msg::Msg;
use crate::ssh::SshMsg;
use crate::state::ssh::SshPage;
use eframe::egui::{self, Id, Key, RichText, Ui};
use trellis_core::ssh_keys::{comment_problem, default_location};

/// A field with its label above it; returns its response and whether Enter left it.
fn field(ui: &mut Ui, label: &str, text: &mut String, id: &str, hint: &str, password: bool) -> (egui::Response, bool) {
    ui.label(label);
    let edit = egui::TextEdit::singleline(text).id(Id::new(id)).hint_text(hint).password(password).desired_width(f32::INFINITY);
    let r = controls::field(ui, edit);
    let enter = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
    (r, enter)
}

fn note(ui: &mut Ui, text: &str) {
    ui.add(egui::Label::new(RichText::new(text).color(color::weak())).wrap());
}

/// Draws the dialog; returns true when Cancel or Esc closes it. The location field takes the
/// focus when nothing has it, unless the keyboard belongs to something above (`cx.keyboard`).
pub fn show(ui: &mut Ui, cx: Cx, page: &mut SshPage, msgs: &mut Vec<Msg>) -> bool {
    let t = cx.t;
    let d = Dialog::new(cx, t.tr("prefs.ssh.generate.title")).subtitle(t.tr("prefs.ssh.generate.subtitle")).begin(ui);
    ui.add_space(6.0);

    let hint = page.listing.as_ref().map(|l| default_location(&l.home).display().to_string()).unwrap_or_default();
    let label = t.tr("prefs.ssh.generate.location.label");
    let (location_r, enter_location) = field(ui, label, &mut page.location, "trellis-ssh-keygen-location", &hint, false);
    form_message::track(&mut page.touched, "location", &location_r);
    let location = match page.location_problem() {
        Some(p) => Some(Message::error(p.message())),
        None => page.location_taken().then(|| Message::warning(t.tr("prefs.ssh.generate.location.taken.warn"))),
    };
    form_message::field(ui, &page.touched, "location", location);
    if cx.keyboard && ui.memory(|m| m.focused().is_none()) {
        location_r.request_focus();
    }
    ui.add_space(6.0);

    let label = t.tr("prefs.ssh.generate.comment.label");
    let hint = t.tr("prefs.ssh.generate.comment.hint");
    let (comment_r, enter_comment) = field(ui, label, &mut page.comment, "trellis-ssh-keygen-comment", hint, false);
    form_message::track(&mut page.touched, "comment", &comment_r);
    form_message::field(ui, &page.touched, "comment", comment_problem(&page.comment).map(Message::error));
    note(ui, t.tr("prefs.ssh.generate.comment.help"));
    ui.add_space(6.0);

    let label = t.tr("prefs.ssh.generate.passphrase.label");
    let hint = t.tr("prefs.ssh.generate.passphrase.hint");
    let (pass_r, enter_pass) = field(ui, label, &mut page.passphrase, "trellis-ssh-keygen-passphrase", hint, true);
    form_message::track(&mut page.touched, "passphrase", &pass_r);
    note(ui, t.tr("prefs.ssh.generate.passphrase.help"));
    ui.add_space(6.0);

    let label = t.tr("prefs.ssh.generate.confirm.label");
    let (confirm_r, enter_confirm) = field(ui, label, &mut page.confirm, "trellis-ssh-keygen-confirm", "", true);
    form_message::track(&mut page.touched, "confirm", &confirm_r);
    let mismatch = page.passphrase_mismatch().then(|| Message::error(t.tr("prefs.ssh.generate.mismatch.warn")));
    form_message::field(ui, &page.touched, "confirm", mismatch);

    if let Some(e) = &page.form_error {
        ui.add_space(10.0);
        form_message::form(ui, Message::error(e.as_str()));
    }

    // Only a reason that isn't a field's: the fields say theirs under them. While ssh-keygen
    // runs, the note says so instead.
    let wait = page.form_wait().filter(|_| !page.generating);
    let generate = buttons::primary(t.tr("prefs.ssh.generate.create.button")).enabled(page.can_generate()).why(wait.as_deref());
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
