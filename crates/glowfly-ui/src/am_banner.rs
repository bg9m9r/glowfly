//! The Applying patches lines under the WIP panel's step heading while a `git am` series is
//! stopped (`state::am_banner`).

use super::theme::color;
use crate::state::am_banner;
use eframe::egui::{self, RichText};
use glowfly_core::i18n::Lang;
use glowfly_core::repo::Snapshot;

/// A stopped patch series' lines (none for other operations), also under the WIP panel's
/// step heading (`op_steps`).
pub fn lines(ui: &mut egui::Ui, t: Lang, snap: &Snapshot, conflicts_left: bool) {
    let Some(p) = &snap.am else { return };
    let text = am_banner::lines(t, p, conflicts_left, !snap.staged.is_empty());
    if let Some(stop) = text.stop {
        ui.label(stop);
    }
    ui.add(egui::Label::new(RichText::new(text.hint).color(color::weak())).wrap());
}
