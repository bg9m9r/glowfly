//! One line of text set as `sizes::text_pos` sets it: `paint_text`, the drop-in for egui's
//! `Painter::text` that puts vertically centred text on a whole point (`sizes::centred_on`), and
//! `line_rect`, the one-line rect an egui label is put in so it sits where painted text does,
//! and `put_line`, which puts a label there without moving the parent's cursor.

use crate::theme::sizes;
use eframe::egui::{Align, Align2, Color32, Direction, FontId, Layout, Painter, Pos2, Rect, Response, Ui, UiBuilder, Widget, vec2};

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

/// `r` cut to one line of `font`, as wide as `r`, with its top where `sizes::text_pos` puts
/// text in `r` (a whole point at or above the centre). Put a label in it
/// (`ui.put(line_rect(..), Label)`) so it keeps its accessible role and sits where painted text
/// sits: egui sets a label put in a rect one line tall at that rect's top.
pub fn line_rect(ui: &Ui, r: Rect, font: &FontId) -> Rect {
    let row_h = ui.fonts(|f| f.row_height(font));
    Rect::from_min_size(Pos2::new(r.left(), sizes::centred_top(r, row_h)), vec2(r.width(), row_h))
}

/// `widget` (a label) laid out as `ui.put` lays it out, in `line_rect(ui, r, font)`, but in a
/// child that leaves `ui`'s cursor alone: `ui.put` moves the cursor to the line's bottom, which
/// sits above `r`'s, so in a list of rows the next row would start that much higher.
pub fn put_line(ui: &mut Ui, r: Rect, font: &FontId, widget: impl Widget) -> Response {
    let line = line_rect(ui, r, font);
    ui.new_child(UiBuilder::new().max_rect(line).layout(Layout::centered_and_justified(Direction::TopDown))).add(widget)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::type_scale::{self, BODY};
    use eframe::egui::{self, Context, LayerId, pos2};

    /// Runs `f` with a painter over a fresh context's background layer.
    fn with_painter(f: impl Fn(&Painter)) {
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
                assert_eq!(
                    paint_text(p, at, anchor, "Fetch", font.clone(), Color32::WHITE),
                    p.text(at, anchor, "Fetch", font.clone(), Color32::WHITE)
                );
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

    #[test]
    fn put_line_sets_the_label_on_the_line_and_leaves_the_cursor() {
        let ctx = Context::default();
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                let font = type_scale::font(BODY);
                let (r, _) = ui.allocate_exact_size(vec2(120.0, 19.0), egui::Sense::hover());
                let before = ui.cursor().min;
                let shown = put_line(ui, r, &font, egui::Label::new(egui::RichText::new("Ours, then theirs").font(font.clone())));
                assert_eq!(shown.rect.top(), line_rect(ui, r, &font).top());
                assert_eq!(ui.cursor().min, before, "the next row starts at the band's bottom");
                assert_eq!(before.y, r.bottom());
            });
        });
    }

    #[test]
    fn line_rect_is_one_line_on_a_whole_point() {
        let ctx = Context::default();
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let font = type_scale::font(BODY);
                let row_h = ui.fonts(|f| f.row_height(&font));
                for top in [0.0, 0.5, 3.25] {
                    let r = Rect::from_min_size(pos2(7.0, top), vec2(120.0, 28.0));
                    let line = line_rect(ui, r, &font);
                    assert_eq!(line.top(), (r.center().y - row_h / 2.0).floor(), "top {top}");
                    assert_eq!(line.height(), row_h);
                    assert_eq!((line.left(), line.width()), (r.left(), r.width()));
                }
            });
        });
    }
}
