//! The card over the WIP panel for an operation that has no steps (`state::op_steps`): a bisect,
//! or git's ambiguous "apply mailbox or rebase" leftover, with nothing conflicted. Neither has
//! conflicts to resolve or a commit to make, so the card keeps to its title, its notes and its
//! actions in one wrapping row with the standard gap (Reset bisect, or Abort, asking first).

use super::common::state_actions;
use super::theme::color;
use super::widgets::banner::{self, BannerKind};
use super::widgets::buttons;
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::op_actions::{action_text, banner_title};
use crate::state::op_confirm::banner_msg;
use eframe::egui::{self, RichText};
use glowfly_core::repo::Snapshot;

/// The gap between the card's buttons, across and between wrapped rows.
const BUTTON_GAP: egui::Vec2 = egui::vec2(8.0, 8.0);

/// The card; nothing when no operation is in progress.
pub(super) fn op_banner(ui: &mut egui::Ui, st: &AppState, snap: &Snapshot, msgs: &mut Vec<Msg>) {
    let (Some(actions), Some(title)) = (state_actions(snap.state), banner_title(st.settings.lang(), snap)) else { return };
    let t = st.settings.lang();
    let kind = if st.job.is_some() { BannerKind::Progress } else { BannerKind::Info };
    banner::frame(ui, kind, None, |ui| {
        ui.label(RichText::new(title).font(super::theme::type_scale::strong(ui, super::theme::type_scale::BODY)));
        if let Some(note) = crate::state::auto_stash::banner_note(st) {
            ui.add(egui::Label::new(RichText::new(note).color(color::weak())).wrap());
        }
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = BUTTON_GAP;
            for (label, a) in actions {
                if ui.add(buttons::secondary(action_text(t, label))).clicked() {
                    msgs.push(banner_msg(st, label, a));
                }
            }
        });
    });
    ui.add_space(8.0);
}
