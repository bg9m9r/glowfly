//! The confirmation dialog (`Modal::Confirm`), and its danger form for steps that lose work
//! (`Modal::ConfirmDanger`): the danger-filled button at the right edge with Cancel focused
//! 12 px to its left, and, for a step that can't be undone, an acknowledgement to tick first.

use crate::msg::Msg;
use crate::ui::dialog_frame::{self, Cx, Dialog, Footer, Tone};
use crate::ui::widgets::buttons;
use eframe::egui::{self, Ui};

/// Draws the dialog; returns true when it should close (Cancel). `ack` is the acknowledgement's
/// label and tick, when the step needs one.
pub(super) fn show(
    ui: &mut Ui,
    cx: Cx,
    (title, body, button): (&str, &str, &str),
    tone: Tone,
    ack: Option<(&str, &mut bool)>,
    msgs: &mut Vec<Msg>,
) -> bool {
    let mut d = Dialog::new(cx, title).begin(ui);
    d.body(ui, |ui| ui.add(egui::Label::new(body).wrap()));
    let mut ok = true;
    if let Some((label, acked)) = ack {
        dialog_frame::ack(ui, cx.t, acked, Some(label));
        ok = *acked;
    }
    let footer = match tone {
        Tone::Normal => Footer::new().primary(buttons::primary(button)),
        Tone::Danger => Footer::new().danger(buttons::danger(button).enabled(ok).why(Some(cx.t.tr("common.ack.tip")))),
    };
    let out = d.finish(ui, footer.cancel());
    if out.primary {
        msgs.push(Msg::ConfirmAccepted);
    }
    out.dismiss
}
