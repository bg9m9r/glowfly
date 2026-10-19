//! The title lines of the WIP row's operation banner: the Applying patches lines while a
//! `git am` series is stopped (`state::am_banner`), else "<operation> in progress".

use super::theme::color;
use crate::state::am_banner::lines;
use crate::ui::theme::type_scale;
use eframe::egui::{self, RichText};
use trellis_core::repo::Snapshot;

/// Draws the banner's title for the operation `name`, with `conflicts_left` whether conflicted
/// files remain.
pub fn title(ui: &mut egui::Ui, snap: &Snapshot, name: &str, conflicts_left: bool) {
    let Some(p) = &snap.am else {
        ui.label(RichText::new(format!("{name} in progress")).font(type_scale::strong(ui, type_scale::BODY)));
        return;
    };
    let text = lines(p, conflicts_left, !snap.staged.is_empty());
    ui.label(RichText::new(text.title).font(type_scale::strong(ui, type_scale::BODY)));
    if let Some(stop) = text.stop {
        ui.label(stop);
    }
    ui.add(egui::Label::new(RichText::new(text.hint).color(color::weak())).wrap());
}
