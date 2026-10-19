//! Loading more of a truncated history from the graph: the near-end check and the row shown
//! while it loads.

use crate::state::AppState;
use crate::state::refresh::can_load_more;
use crate::ui::theme::color;
use crate::ui::theme::type_scale;
use eframe::egui::{self, Align2, Rect, Sense, WidgetInfo, WidgetType, pos2, vec2};
use trellis_core::i18n::Lang;

/// How close to the last loaded row the graph's last visible row gets before more is loaded.
pub(super) const NEAR_END_ROWS: usize = 40;

/// Whether the graph, showing rows up to `last_visible` (exclusive) of `total`, should ask for
/// more history. update checks the same conditions again; this keeps the view from asking every
/// frame while a load is pending.
pub(super) fn wants_more(st: &AppState, last_visible: usize, total: usize, filtered: bool) -> bool {
    st.settings.load_more_on_scroll
        && !filtered
        && st.snap.as_ref().is_some_and(can_load_more)
        && !st.loading_more()
        && last_visible + NEAR_END_ROWS >= total
}

/// The row below the last commit while more history loads: a spinner and a weak note, starting
/// at the message column's left edge `text_x`. It can't be clicked or selected.
pub(super) fn loading_row(ui: &mut egui::Ui, t: Lang, row_h: f32, text_x: f32) {
    let text = t.tr("graph.loading_more");
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), row_h), Sense::hover());
    let mid = rect.center().y;
    // Painted rather than added, so the row's layout stays one allocation.
    egui::Spinner::new().size(14.0).color(color::weak()).paint_at(ui, Rect::from_min_size(pos2(text_x, mid - 7.0), vec2(14.0, 14.0)));
    ui.painter_at(rect).text(pos2(text_x + 22.0, mid), Align2::LEFT_CENTER, text, type_scale::font(type_scale::BODY), color::weak());
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, text));
}
