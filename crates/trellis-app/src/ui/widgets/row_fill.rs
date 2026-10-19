//! The shared look of a selected or hovered row: the selection fill with an accent bar along the
//! left edge, and the lifted hover (the hover fill with a soft shadow inside the bottom edge).
//! The graph, the sidebar's rows, the file lists and the file history list all use it.

use crate::ui::theme::color;
use eframe::egui::{Color32, Painter, Rect, pos2, vec2};

/// The width of a selected row's accent bar.
pub const ACCENT_BAR_W: f32 = 3.0;

/// Paints a row's selection or hover in `rect`: the selection wins; a plain row paints nothing.
pub fn paint(p: &Painter, rect: Rect, selected: bool, hovered: bool) {
    let pal = color::current();
    if selected {
        p.rect_filled(rect, 0.0, pal.selected);
        p.rect_filled(Rect::from_min_size(rect.left_top(), vec2(ACCENT_BAR_W, rect.height())), 0.0, pal.accent);
    } else if hovered {
        // Lifted: the hover fill with a soft shadow inside the bottom edge.
        p.rect_filled(rect, 0.0, pal.hover);
        let alpha = if pal.dark { 70 } else { 26 };
        let line = |from: f32, a: u8| {
            let r = Rect::from_min_max(pos2(rect.left(), rect.bottom() - from), pos2(rect.right(), rect.bottom() - from + 1.0));
            p.rect_filled(r, 0.0, Color32::from_black_alpha(a));
        };
        line(1.0, alpha);
        line(2.0, alpha / 2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{self, Id, LayerId, Order, Shape};

    const ROW: Rect = Rect { min: egui::Pos2 { x: 10.0, y: 20.0 }, max: egui::Pos2 { x: 210.0, y: 56.0 } };

    /// The filled rects `paint` draws for one row, as (rect, colour).
    fn fills(selected: bool, hovered: bool) -> Vec<(Rect, Color32)> {
        color::set_dark(true);
        let ctx = egui::Context::default();
        let out = ctx.run(egui::RawInput::default(), |ctx| {
            let p = Painter::new(ctx.clone(), LayerId::new(Order::Background, Id::new("rows")), Rect::EVERYTHING);
            paint(&p, ROW, selected, hovered);
        });
        out.shapes
            .iter()
            .filter_map(|c| match &c.shape {
                Shape::Rect(r) => Some((r.rect, r.fill)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn selected_rows_get_the_fill_and_a_left_accent_bar() {
        let pal = color::current();
        let f = fills(true, true);
        assert!(f.contains(&(ROW, pal.selected)), "the fill spans the row: {f:?}");
        let bar = f.iter().find(|(_, c)| *c == pal.accent).expect("an accent bar");
        assert_eq!(bar.0, Rect::from_min_size(ROW.left_top(), vec2(ACCENT_BAR_W, ROW.height())));
        assert!(!f.iter().any(|(_, c)| *c == pal.hover), "the selection wins over the hover");
    }

    #[test]
    fn hovered_rows_are_lifted() {
        let pal = color::current();
        let f = fills(false, true);
        assert_eq!(f[0], (ROW, pal.hover));
        let lines: Vec<_> = f[1..].iter().collect();
        assert_eq!(lines.len(), 2, "two shadow lines: {f:?}");
        for (i, (r, c)) in lines.iter().enumerate() {
            assert_eq!(r.height(), 1.0);
            assert_eq!(r.width(), ROW.width());
            assert!(r.bottom() <= ROW.bottom() && r.top() >= ROW.bottom() - 2.0, "inside the bottom edge: {r:?}");
            assert_eq!((c.r(), c.g(), c.b()), (0, 0, 0), "black alpha");
            assert!(c.a() > 0, "line {i} shows");
        }
    }

    #[test]
    fn a_plain_row_paints_nothing() {
        assert!(fills(false, false).is_empty());
    }
}
