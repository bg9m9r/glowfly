//! A menu's header when the title wraps: plain left-aligned lines, not stretched to the width
//! the way the menu's justified items are.

use super::common::commit_menu_header;
use crate::ui::snapshot_check::CheckSnapshot;
use crate::ui::theme::{self, color};
use eframe::egui;
use git2::Oid;

#[test]
fn commit_menu_header_wraps_without_stretching() {
    let summary = "fix(sensors): gust 7 ignores negative readings from the anemometer after a reset";
    let oid = Oid::from_str("0b124860a1b2c3d4e5f60718293a4b5c6d7e8f90").unwrap();
    let mut h = crate::ui::snapshot_check::builder().with_size(egui::vec2(420.0, 120.0)).build_ui(move |ui| {
        color::set_palette(&theme::DUSK);
        theme::apply_style(ui.ctx(), &theme::DUSK);
        ui.ctx().set_theme(theme::egui_theme(&theme::DUSK));
        ui.painter().rect_filled(ui.max_rect(), 0.0, color::raised());
        // A menu's own layout: items fill the width.
        ui.with_layout(egui::Layout::top_down_justified(egui::Align::Min), |ui| {
            commit_menu_header(ui, summary, oid);
            let _ = ui.button("Checkout (detached)");
        });
    });
    h.run();
    h.check_snapshot("commit_menu_header_wraps");
}
