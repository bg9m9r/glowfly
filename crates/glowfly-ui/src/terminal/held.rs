//! The terminal panel's body while the active tab's shell is held (`SessionStatus::Held`): git
//! is held in the repository until the user trusts it, and a shell prompt often runs git, so no
//! shell starts until then. Trust and run… brings the trust prompt up on the shells' behalf.

use crate::empty_state::{EmptyIcon, PaneButton, empty_state};
use eframe::egui::Ui;
use glowfly_model::msg::{Msg, TrustMsg};
use glowfly_model::state::AppState;
use glowfly_model::state::trust::Standing;

pub(super) fn show(ui: &mut Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let Some(snap) = &st.snap else { return };
    let t = st.settings.lang();
    let hint =
        if st.trust_standing() == Some(Standing::NotTrusted) { t.tr("terminal.held.hint_refused") } else { t.tr("terminal.held.hint") };
    let button = PaneButton { text: t.tr("terminal.held.button"), primary: true };
    if empty_state(ui, EmptyIcon::Shield, t.tr("terminal.held.title"), hint, &[button]) == Some(0) {
        msgs.push(Msg::Trust(TrustMsg::AskForTerminal { real: snap.real_workdir.clone() }));
    }
}
