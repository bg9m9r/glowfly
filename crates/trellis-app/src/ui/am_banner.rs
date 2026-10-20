//! The title lines of the WIP row's operation banner: the title, then the Applying patches
//! lines while a `git am` series is stopped (`state::am_banner`).

use super::theme::color;
use crate::state::am_banner::lines;
use crate::ui::theme::type_scale;
use eframe::egui::{self, RichText};
use trellis_core::i18n::Lang;
use trellis_core::repo::Snapshot;

/// Draws the banner's `title`, then for a patch series its lines, with `conflicts_left`
/// whether conflicted files remain.
pub fn title(ui: &mut egui::Ui, t: Lang, snap: &Snapshot, title: &str, conflicts_left: bool) {
    ui.label(RichText::new(title).font(type_scale::strong(ui, type_scale::BODY)));
    let Some(p) = &snap.am else { return };
    let text = lines(t, p, conflicts_left, !snap.staged.is_empty());
    if let Some(stop) = text.stop {
        ui.label(stop);
    }
    ui.add(egui::Label::new(RichText::new(text.hint).color(color::weak())).wrap());
}
