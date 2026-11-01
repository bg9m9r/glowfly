//! The segment bar: flush segments, a fill only on the hovered and enabled segment, over its
//! own shape, and a framed single segment lighting its whole frame.

use super::*;
use crate::ui::theme::tokens::radius;
use eframe::egui::{self, Event, Pos2, RawInput, Shape, epaint::RectShape};

/// The rects painted on the second of two frames of a bar of `segs` in `style`, with the
/// pointer at `at`, and the segments' rects.
fn painted(style: BarStyle, segs: &[Seg], at: Pos2) -> (Vec<Rect>, Vec<RectShape>) {
    color::set_dark(true);
    let ctx = egui::Context::default();
    let mut rects = Vec::new();
    let mut shapes = Vec::new();
    for _ in 0..2 {
        let input = RawInput { events: vec![Event::PointerMoved(at)], ..Default::default() };
        let out = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let bar = segment_bar(ui, ControlSize::Regular, 40.0, radius::MD, style, segs);
                bar.paint(ui, &[]);
                rects = bar.responses.iter().map(|r| r.rect).collect();
            });
        });
        // The panel's own fill aside.
        shapes = out
            .shapes
            .into_iter()
            .filter_map(|c| if let Shape::Rect(r) = c.shape { Some(r) } else { None })
            .filter(|r| r.rect.width() < 1000.0)
            .collect();
    }
    (rects, shapes)
}

const QUIET: BarStyle = BarStyle::Quiet { divider: 24.0 };

#[test]
fn segments_are_flush_and_only_the_hovered_one_lights() {
    let segs = [Seg { w: 56.0, enabled: true }, Seg { w: 24.0, enabled: true }];
    let (rects, _) = painted(QUIET, &segs, Pos2::ZERO);
    assert_eq!(rects[0].right(), rects[1].left(), "flush");
    assert_eq!((rects[0].width(), rects[1].width()), (56.0, 24.0));
    let (rects, shapes) = painted(QUIET, &segs, rects[1].center());
    let lit: Vec<&RectShape> = shapes.iter().filter(|s| s.fill != Color32::TRANSPARENT).collect();
    assert_eq!(lit.len(), 1, "{shapes:?}");
    assert_eq!(lit[0].rect, rects[1], "the hovered half lights its own whole shape");
    assert_eq!(lit[0].corner_radius, radius::segment(radius::MD, false, true));
}

#[test]
fn a_disabled_segment_never_lights() {
    let segs = [Seg { w: 56.0, enabled: false }, Seg { w: 24.0, enabled: true }];
    let (rects, _) = painted(QUIET, &segs, Pos2::ZERO);
    let (_, shapes) = painted(QUIET, &segs, rects[0].center());
    assert!(shapes.iter().all(|s| s.fill == Color32::TRANSPARENT), "{shapes:?}");
}

#[test]
fn a_framed_bar_lights_each_segment_inside_its_border() {
    let fill = Color32::from_gray(40);
    let style = BarStyle::Framed { fill };
    let segs = [Seg { w: 100.0, enabled: true }, Seg { w: 120.0, enabled: true }];
    let (rects, _) = painted(style, &segs, Pos2::ZERO);
    assert_eq!(rects[0].height(), 38.0, "inside the 1 px border");
    let (rects, shapes) = painted(style, &segs, rects[0].center());
    let lit: Vec<&RectShape> = shapes.iter().filter(|s| s.fill != Color32::TRANSPARENT && s.fill != fill).collect();
    assert_eq!(lit.len(), 1, "{shapes:?}");
    assert_eq!(lit[0].rect, rects[0]);
    assert_eq!(lit[0].corner_radius, radius::segment(radius::inside(radius::MD, 1), true, false));
}

#[test]
fn a_framed_single_segment_lights_its_whole_frame() {
    let fill = Color32::from_gray(40);
    let style = BarStyle::Framed { fill };
    let segs = [Seg { w: 100.0, enabled: true }];
    let (rects, _) = painted(style, &segs, Pos2::ZERO);
    let (_, shapes) = painted(style, &segs, rects[0].center());
    let frame = rects[0].expand(BORDER_W);
    let lit = shapes.iter().find(|s| s.fill != Color32::TRANSPARENT && s.fill != fill).expect("a hover layer");
    assert_eq!(lit.rect, frame, "the whole frame lights");
    let edge = shapes.iter().find(|s| s.stroke.width > 0.0).expect("its border");
    assert_eq!(edge.stroke.color, color::weak(), "the border goes weak while lit");
}
