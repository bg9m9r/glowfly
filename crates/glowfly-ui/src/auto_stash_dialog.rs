//! The Uncommitted changes dialog (`Modal::AutoStash`): stash the changes, run the operation
//! and put them back afterwards. A hard reset offers Discard and reset instead of Always stash.

use super::dialog_frame::{Cx, Dialog, Footer};
use super::widgets::buttons;
use crate::msg::{AutoStashMsg, Msg};
use crate::state::auto_stash::{self, AutoStashAsk};
use crate::ui::theme::{color, controls};
use eframe::egui::{self, RichText};

/// Draws the dialog; returns true when it should close (Cancel). The answers go to `update`,
/// which takes the dialog and runs the operation, as a confirmation does.
pub fn show(ui: &mut egui::Ui, cx: Cx, ask: &mut AutoStashAsk, msgs: &mut Vec<Msg>) -> bool {
    let t = cx.t;
    let hard = ask.op.is_hard_reset();
    let mut d = Dialog::new(cx, auto_stash::title(t)).begin(ui);
    let body = auto_stash::body(t, &ask.op, ask.files);
    d.body(ui, |ui| ui.add(egui::Label::new(body).wrap()));
    if !hard {
        ui.add_space(10.0);
        controls::checkbox(ui, &mut ask.always, t.tr("auto_stash.always.check"));
        ui.add(egui::Label::new(RichText::new(t.tr("auto_stash.always.help")).color(color::weak())).wrap());
    }
    let footer = Footer::new().cancel();
    let footer = if hard { footer.alt(buttons::danger(auto_stash::discard_button(t))) } else { footer };
    let out = d.finish(ui, footer.primary(buttons::primary(auto_stash::button(t, &ask.op))));
    if out.primary {
        msgs.push(Msg::AutoStash(AutoStashMsg::Stash));
    } else if out.alt == Some(0) {
        msgs.push(Msg::AutoStash(AutoStashMsg::Discard));
    }
    out.dismiss
}
