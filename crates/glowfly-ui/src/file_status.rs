//! A changed file's status badge: its letter (M, A, D, R, …) centred on a one-letter badge
//! (`widgets::chip`, an 18 px circle) in the status colour, with the same fill and ink as the
//! graph WIP row's change badges. A conflicted file ('!') takes the coral needs-you badge of the
//! same size instead. Screen readers read the status as a word (`status_word`), never the letter.

use super::common::{status_color, status_label};
use super::theme::{color, contrast};
use super::widgets::chip::{Chip, ChipSize, Look};
use eframe::egui::{Response, Sense, Ui, WidgetInfo, WidgetType, vec2};
use glowfly_core::i18n::Lang;

/// The badge's width and height, the same for every letter so the paths line up.
pub(super) const STATUS_CHIP_W: f32 = ChipSize::Badge.metrics().min_w;

/// The word a screen reader says for the status letter `status` ("Modified" for 'M').
pub(super) fn status_word(t: Lang, status: char) -> &'static str {
    match status {
        'M' => t.tr("files.status.modified"),
        'A' | '?' => t.tr("files.status.added"),
        'D' => t.tr("files.status.deleted"),
        'R' => t.tr("files.status.renamed"),
        'C' => t.tr("files.status.copied"),
        'T' => t.tr("files.status.type_changed"),
        '!' | 'U' => t.tr("files.status.conflicted"),
        _ => t.tr("files.status.changed"),
    }
}

/// Names `resp` (a status letter or badge) by its status's word.
pub(super) fn name_status(resp: &Response, t: Lang, status: char) {
    let word = status_word(t, status);
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, word));
}

/// Draws the badge for `status`; returns its (hover) response, named by the status's word.
pub(super) fn status_chip(ui: &mut Ui, t: Lang, status: char) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(STATUS_CHIP_W, STATUS_CHIP_W), Sense::hover());
    name_status(&resp, t, status);
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        if status == '!' {
            super::needs_you::status_badge(p, rect);
            return resp;
        }
        let (fill, ink) = contrast::change_chip(status_color(status), &color::current());
        Chip::new(ChipSize::Badge, status_label(status).to_string()).look(Look::new(fill, ink)).paint_in_rect(p, rect);
    }
    resp
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui;

    #[test]
    fn every_status_takes_the_same_square() {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                for status in ['M', 'A', 'D', 'R', 'T', '?'] {
                    let r = status_chip(ui, Lang::En, status);
                    assert_eq!(r.rect.size(), vec2(STATUS_CHIP_W, STATUS_CHIP_W), "{status}");
                }
            });
        });
    }
}
