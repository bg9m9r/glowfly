//! The File view's previews: a Markdown file drawn by the shared renderer (`markdown.rs`,
//! parsed once through `MarkdownCache`) in a scroll area of its own, chosen with the toolbar's Text / Preview switch, and an image drawn at its own size or
//! scaled down to fit, over the checkerboard the binary diff uses (`image_paint.rs`).

use super::UiCache;
use super::history_file::{Note, PREVIEW, PREVIEW_TIP, TEXT, TEXT_TIP, bar_row, note_labels};
use super::image_paint;
use super::theme::color;
use super::widgets::segmented::{Segment, segmented_opts};
use crate::msg::{HistoryMsg, Msg};
use crate::state::AppState;
use crate::state::history::{FileTextData, Preview};
use crate::ui::theme::sizes::ControlSize;
use eframe::egui::{self, Align, Color32, Layout, Rect, RichText, Sense, Ui, pos2, vec2};
use git2::Oid;
use glowfly_core::lfs::pointer::size_text;
use std::sync::Arc;

/// The widest the rendered text gets, for lines that stay easy to read.
const MAX_TEXT_W: f32 = 900.0;
/// The space around an image.
const IMAGE_MARGIN: f32 = 12.0;

/// The Text / Preview switch; `toggle` is `Err(why)` when this version has no preview. `on`:
/// the preview is shown.
pub(super) fn toggle(ui: &mut Ui, toggle: Result<(), &str>, on: bool, msgs: &mut Vec<Msg>) {
    let options = [Segment::new(false, TEXT, TEXT_TIP), Segment::new(true, PREVIEW, PREVIEW_TIP).why(toggle.err())];
    if let Some(v) = segmented_opts(ui, ControlSize::Regular, on, &options) {
        msgs.push(Msg::History(HistoryMsg::SetFilePreview(v)));
    }
}

/// The rendered document, back at the top whenever the switch is used or another version is
/// shown. `id`: the version's commit and path, which keep its widget ids apart from another's.
pub(super) fn markdown(ui: &mut Ui, st: &mut AppState, cache: &mut UiCache, id: (Oid, String), source: &str, msgs: &mut Vec<Msg>) {
    let to_top = st.session.history.view.as_mut().is_some_and(|v| std::mem::take(&mut v.nav.to_top));
    ui.painter().rect_filled(ui.available_rect_before_wrap(), 0.0, color::bg());
    let mut area = egui::ScrollArea::vertical().id_salt("history-preview").auto_shrink([false, false]);
    if to_top {
        area = area.vertical_scroll_offset(0.0);
    }
    area.show(ui, |ui| {
        egui::Frame::new().inner_margin(egui::Margin::same(16)).show(ui, |ui| {
            let w = ui.available_width().min(MAX_TEXT_W);
            ui.set_max_width(w);
            ui.set_min_width(w);
            super::markdown::body(ui, ("history-preview", id), source, &mut cache.markdown, msgs);
        });
    });
}

/// The image's toolbar row (format, size, scale and the version's notes) and the image itself,
/// centred at the top of the space left, 1 pixel to 1 point unless that doesn't fit.
pub(super) fn image(ui: &mut Ui, cache: &mut UiCache, data: &Arc<FileTextData>, notes: &[Note]) {
    let Some(Preview::Image { image: img, size }) = &data.preview else { return };
    let (w, h) = (img.width.max(1) as f32, img.height.max(1) as f32);
    let row_h = ui.spacing().interact_size.y;
    let room = ui.available_size() - vec2(2.0 * IMAGE_MARGIN, row_h + 4.0 + 2.0 * IMAGE_MARGIN);
    let scale = (room.x / w).min(room.y / h).clamp(0.01, 1.0);
    ui.add_space(2.0);
    bar_row(ui, "history-file-bar", row_h, |ui| {
        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
            ui.add_space(10.0);
            let what = format!("{} image · {} × {} · {}", img.format, img.width, img.height, size_text(*size));
            ui.label(RichText::new(what).color(color::weak()));
            if scale < 1.0 {
                ui.label(RichText::new("·").color(color::faint()));
                let pct = ((scale * 100.0).floor() as u32).max(1);
                ui.label(RichText::new(format!("shown at {pct}%")).color(color::weak()));
            }
            note_labels(ui, notes);
        });
    });
    ui.add_space(2.0);
    let area = ui.available_rect_before_wrap();
    ui.painter().rect_filled(area, 0.0, color::bg());
    let shown = vec2(w, h) * scale;
    let min = pos2((area.center().x - shown.x / 2.0).max(area.left() + IMAGE_MARGIN), area.top() + IMAGE_MARGIN);
    let rect = Rect::from_min_size(min.round(), shown.round());
    let resp = ui.allocate_rect(rect, Sense::hover());
    image_paint::checkerboard(ui.painter(), rect);
    let tex = cache.history_preview.get(ui.ctx(), data, img);
    ui.painter().image(tex.id(), rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    resp.on_hover_text(format!("{} × {} pixels", img.width, img.height));
}

#[cfg(test)]
mod tests {
    use super::super::history_file::NO_PREVIEW_LONG;
    use glowfly_core::markdown::MARKDOWN_MAX_BYTES;

    #[test]
    fn the_long_reason_names_the_limit() {
        assert!(NO_PREVIEW_LONG.contains(&format!("over {} KB", MARKDOWN_MAX_BYTES / 1024)), "{NO_PREVIEW_LONG}");
    }
}
