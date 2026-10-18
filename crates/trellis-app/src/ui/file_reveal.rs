//! Scrolling a file list to the file a next/previous change step landed on, once per landing
//! (`ViewState::file_reveal`), so the highlighted row is in view even when the list was scrolled
//! elsewhere or the file sits below its fold.

use eframe::egui;
use std::ops::Range;
use trellis_core::repo::DiffTarget;

/// Called inside a `show_rows` body drawing `range` of a list of `row_h`-high rows: when `want`
/// names a landing this list (`list`) hasn't revealed yet and `index` finds its file in the list,
/// scrolls the list just enough to show that row. Rows outside `range` aren't drawn, so the row
/// is placed from its index.
pub(in crate::ui) fn reveal(
    ui: &egui::Ui,
    list: &str,
    want: Option<&(DiffTarget, u64)>,
    index: impl FnOnce(&DiffTarget) -> Option<usize>,
    range: &Range<usize>,
    row_h: f32,
) {
    let Some((target, generation)) = want else { return };
    let id = egui::Id::new(("file-reveal", list));
    if ui.data(|d| d.get_temp::<u64>(id)) == Some(*generation) {
        return;
    }
    let Some(i) = index(target) else { return };
    ui.data_mut(|d| d.insert_temp(id, *generation));
    let step = row_h + ui.spacing().item_spacing.y;
    let area = ui.max_rect();
    let top = area.top() + (i as f32 - range.start as f32) * step;
    let row = egui::Rect::from_min_size(egui::pos2(area.left(), top), egui::vec2(area.width(), row_h));
    ui.scroll_to_rect(row, None);
}
