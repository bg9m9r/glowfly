//! The toolbar's "Untrusted" chip: shown while the open repository's git is held; its tooltip
//! says what is paused and why, and a click brings the trust prompt back (update/trust.rs).
//! It is a Small control: while Glowfly waits on your decision (Undecided, or Changed: trusted
//! before but it gained settings or hooks since) the coral needs-you control with the warning
//! mark; after you chose not to trust the repository a quiet button with the shield.

use super::widgets::{Icon, buttons};
use crate::theme::sizes::ControlSize;
use crate::theme::type_scale;
use eframe::egui::{self, Sense};
use glowfly_model::msg::{Msg, TrustMsg};
use glowfly_model::state::AppState;
use glowfly_model::state::trust::{self, Standing};

/// The chip's label.
const LABEL: &str = "Untrusted";

/// Draws the chip (in a right-to-left layout) while the active repository is Undecided,
/// NotTrusted or Changed; nothing otherwise.
pub fn show(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let Some(snap) = &st.snap else { return };
    let standing = match st.trust_standing() {
        Some(s @ (Standing::Undecided | Standing::NotTrusted | Standing::Changed)) => s,
        _ => return,
    };
    let waiting = matches!(standing, Standing::Undecided | Standing::Changed);
    let tip = trust::paused_note(standing);
    let resp = if waiting {
        let galley = ui.painter().layout_no_wrap(LABEL.into(), type_scale::font(type_scale::SMALL), super::needs_you::ink());
        let (rect, resp) = ui.allocate_exact_size(super::needs_you::control_size(galley.size().x), Sense::click());
        super::needs_you::paint_control(ui.painter(), rect, resp.hovered(), galley);
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, LABEL));
        resp.on_hover_text(tip).on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        ui.add(buttons::quiet(LABEL).icon(Icon::Shield).size(ControlSize::Small).tip(tip))
    };
    if resp.clicked() {
        msgs.push(Msg::Trust(TrustMsg::Ask { real: snap.real_workdir.clone() }));
    }
}
