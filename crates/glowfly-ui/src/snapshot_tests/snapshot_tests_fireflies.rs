//! The firefly loader's sheet (`widgets/fireflies.rs`) in Graphite and Porcelain: the moving
//! loader at a fixed moment of its cycle, the still form reduce motion shows, and the moving
//! loader on a raised card. Each row has a weak caption on the right.

use crate::ui::snapshot_check::CheckSnapshot;
use crate::ui::theme::color::{self, Palette};
use crate::ui::theme::{GRAPHITE, PORCELAIN, apply_style, egui_theme};
use crate::ui::widgets::fireflies;
use eframe::egui::{Align2, CornerRadius, Rect, pos2, vec2};

/// The moment the moving rows show: dots at 0.625, 1.0 and 0.306.
pub(super) const AT: f64 = 0.85;
/// Between rows, and the sheet's size.
const ROW: f32 = 40.0;
const SIZE: (f32, f32) = (360.0, 150.0);

fn sheet(name: &str, p: &'static Palette) {
    let mut h = crate::ui::snapshot_check::builder().with_size(vec2(SIZE.0, SIZE.1)).build_ui(move |ui| {
        color::set_palette(p);
        apply_style(ui.ctx(), p);
        ui.ctx().set_theme(egui_theme(p));
        let r = ui.max_rect();
        let painter = ui.painter();
        painter.rect_filled(r, 0.0, p.panel);
        let rows: [(&str, Option<f64>); 3] = [("moving", Some(AT)), ("reduce motion", None), ("on raised", Some(AT))];
        for (i, (caption, time)) in rows.into_iter().enumerate() {
            let y = r.top() + 35.0 + i as f32 * ROW;
            if i == 2 {
                let card = Rect::from_min_max(pos2(r.left() + 16.0, y - 16.0), pos2(r.right() - 16.0, y + 16.0));
                painter.rect_filled(card, CornerRadius::same(8), p.raised);
            }
            fireflies::paint(painter, pos2(r.left() + 60.0, y), p, time);
            let font = crate::ui::theme::type_scale::font(crate::ui::theme::type_scale::SMALL);
            painter.text(pos2(r.right() - 32.0, y), Align2::RIGHT_CENTER, caption, font, p.weak);
        }
    });
    h.run();
    h.check_snapshot(name);
}

#[test]
fn loader_fireflies_dark() {
    sheet("loader_fireflies_dark", &GRAPHITE);
}

#[test]
fn loader_fireflies_light() {
    sheet("loader_fireflies_light", &PORCELAIN);
}
