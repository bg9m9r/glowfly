//! One line of text painted at a point: `paint_text`, the drop-in for egui's `Painter::text`
//! that sets vertically centred text on a whole point (`sizes::centred_on`), as
//! `sizes::text_pos` does.

use crate::theme::sizes;
use eframe::egui::{Align, Align2, Color32, FontId, Painter, Pos2, Rect, vec2};

/// `Painter::text` with the same arguments and result, except that text centred on `at.y`
/// (`Align2::*_CENTER`) has its top on a whole point at or above the exact centre, so it never
/// sits low. Top and bottom anchors place it exactly as `Painter::text` does.
pub fn paint_text(p: &Painter, at: Pos2, anchor: Align2, text: impl ToString, font: FontId, color: Color32) -> Rect {
    let galley = p.layout_no_wrap(text.to_string(), font, color);
    let mut rect = anchor.anchor_size(at, galley.size());
    if anchor.y() == Align::Center {
        rect = rect.translate(vec2(0.0, sizes::centred_on(at.y, rect.height()) - rect.top()));
    }
    p.galley(rect.min, galley, color);
    rect
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::type_scale::{self, BODY};
    use eframe::egui::{Context, LayerId, pos2};

    /// Runs `f` with a painter over a fresh context's background layer.
    fn with_painter(f: impl FnOnce(&Painter)) {
        let ctx = Context::default();
        let _ = ctx.run(Default::default(), |ctx| f(&Painter::new(ctx.clone(), LayerId::background(), Rect::EVERYTHING)));
    }

    #[test]
    fn centred_text_sits_on_a_whole_point() {
        with_painter(|p| {
            let font = type_scale::font(BODY);
            let h = p.layout_no_wrap("Stage hunk".into(), font.clone(), Color32::WHITE).size().y;
            let r = paint_text(p, pos2(10.0, 14.0), Align2::LEFT_CENTER, "Stage hunk", font.clone(), Color32::WHITE);
            assert_eq!(r.left(), 10.0);
            assert_eq!(r.top(), (14.0 - h / 2.0).floor());
            assert_eq!(r.height(), h);
            let r = paint_text(p, pos2(90.0, 14.5), Align2::RIGHT_CENTER, "Stage hunk", font, Color32::WHITE);
            assert_eq!(r.right(), 90.0);
        });
    }

    #[test]
    fn top_and_bottom_anchors_match_egui() {
        with_painter(|p| {
            let font = type_scale::font(BODY);
            for anchor in [Align2::LEFT_TOP, Align2::LEFT_BOTTOM] {
                let at = pos2(3.0, 14.5);
                assert_eq!(paint_text(p, at, anchor, "Fetch", font.clone(), Color32::WHITE), p.text(at, anchor, "Fetch", font.clone(), Color32::WHITE));
            }
        });
    }

    #[test]
    fn centred_text_moves_up_less_than_a_point() {
        with_painter(|p| {
            let font = type_scale::font(BODY);
            for anchor in [Align2::LEFT_CENTER, Align2::CENTER_CENTER, Align2::RIGHT_CENTER] {
                for y in [14.0, 14.25, 14.5, 14.75] {
                    let at = pos2(20.0, y);
                    let ours = paint_text(p, at, anchor, "Push", font.clone(), Color32::WHITE);
                    let egui = p.text(at, anchor, "Push", font.clone(), Color32::WHITE);
                    let up = egui.top() - ours.top();
                    assert!((0.0..1.0).contains(&up), "{anchor:?} at {y}: up {up}");
                    assert_eq!((ours.left(), ours.size()), (egui.left(), egui.size()));
                }
            }
        });
    }
}
