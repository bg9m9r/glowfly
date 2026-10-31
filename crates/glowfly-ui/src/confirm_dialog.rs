//! The confirmation dialog (`Modal::Confirm`), its danger form for steps that lose work
//! (`Modal::ConfirmDanger`): the danger-filled button at the right edge with Cancel focused
//! 12 px to its left, and, for a step that can't be undone, an acknowledgement to tick first;
//! and its form with an alternative answer between Cancel and the primary
//! (`Modal::ConfirmChoice`), which pushes `Msg::ConfirmAlt`. `Modal::ConfirmDetails` adds a
//! collapsed Details under the body.

use crate::msg::Msg;
use crate::ui::dialog_frame::{self, Cx, Dialog, Footer, Size, Tone};
use crate::ui::widgets::{buttons, details::disclosure};
use eframe::egui::{self, Ui};

/// The dialog's texts: `details`, when given, sits collapsed under the body.
pub(super) struct Texts<'a> {
    pub title: &'a str,
    pub body: &'a str,
    pub button: &'a str,
    pub details: Option<&'a str>,
}

/// Draws the dialog; returns true when it should close (Cancel). `ack` is the acknowledgement's
/// label and tick, when the step needs one; `alt` the alternative button's label.
pub(super) fn show(
    ui: &mut Ui,
    cx: Cx,
    Texts { title, body, button, details }: Texts,
    tone: Tone,
    ack: Option<(&str, &mut bool)>,
    alt: Option<&str>,
    msgs: &mut Vec<Msg>,
) -> bool {
    let mut d = Dialog::new(cx, title).size(Size::Small).begin(ui);
    d.body(ui, |ui| {
        ui.add(egui::Label::new(body).wrap());
        if let Some(text) = details {
            ui.add_space(6.0);
            disclosure(ui, "confirm-details", cx.t.tr("common.details.button"), text);
        }
    });
    let mut ok = true;
    if let Some((label, acked)) = ack {
        dialog_frame::ack(ui, cx.t, acked, Some(label));
        ok = *acked;
    }
    let footer = match alt {
        Some(alt) => Footer::new().alt(buttons::secondary(alt)),
        None => Footer::new(),
    };
    let footer = match tone {
        Tone::Normal => footer.primary(buttons::primary(button)),
        Tone::Danger => footer.danger(buttons::danger(button).enabled(ok).why(Some(cx.t.tr("common.ack.tip")))),
    };
    let out = d.finish(ui, footer.cancel());
    if out.primary {
        msgs.push(Msg::ConfirmAccepted);
    } else if out.alt == Some(0) {
        msgs.push(Msg::ConfirmAlt);
    }
    out.dismiss
}
