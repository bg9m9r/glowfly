//! A changed file's status chip: its letter (M, A, D, R, …) centred on a small square in the
//! status colour, with the same fill and ink as the graph WIP row's change chips. A conflicted
//! file ('!') takes the coral needs-you badge instead.

use super::common::{status_color, status_label};
use super::theme::{color, contrast};
use crate::ui::theme::tokens::radius;
use crate::ui::theme::type_scale;
use eframe::egui::{Align2, CornerRadius, Response, Sense, Ui, vec2};

/// The chip's width and height, the same for every letter so the paths line up.
pub(super) const STATUS_CHIP_W: f32 = 20.0;
const RADIUS: u8 = radius::SM;

/// Draws the chip for `status`; returns its (hover) response.
pub(super) fn status_chip(ui: &mut Ui, status: char) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(STATUS_CHIP_W, STATUS_CHIP_W), Sense::hover());
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        if status == '!' {
            super::needs_you::badge(p, rect);
            return resp;
        }
        let (fill, ink) = contrast::change_chip(status_color(status), &color::current());
        p.rect_filled(rect, CornerRadius::same(RADIUS), fill);
        p.text(rect.center(), Align2::CENTER_CENTER, status_label(status), type_scale::mono(type_scale::SMALL), ink);
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
                    let r = status_chip(ui, status);
                    assert_eq!(r.rect.size(), vec2(STATUS_CHIP_W, STATUS_CHIP_W), "{status}");
                }
            });
        });
    }
}
