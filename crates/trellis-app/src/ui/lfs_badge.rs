//! The small "LFS" badge after a file row's path when the row's stored content is a Git LFS
//! pointer (`FileEntry::lfs`).

use super::theme::color;
use crate::state::lfs_notes::{BADGE, BADGE_TIP};
use eframe::egui::{self, RichText, Stroke};
use trellis_core::repo::FileEntry;

pub(super) fn badge(ui: &mut egui::Ui, f: &FileEntry) {
    if !f.lfs {
        return;
    }
    egui::Frame::new()
        .stroke(Stroke::new(1.0_f32, color::accent()))
        .corner_radius(3)
        .inner_margin(egui::Margin::symmetric(3, 0))
        .show(ui, |ui| ui.label(RichText::new(BADGE).small().strong().color(color::accent())))
        .response
        .on_hover_text(BADGE_TIP);
}
