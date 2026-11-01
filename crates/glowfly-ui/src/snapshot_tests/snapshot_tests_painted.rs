//! What a frame painted where, for tests whose point is a look the text snapshot can't see (a
//! hover fill, buttons shown only on hover): the shapes and fills inside a widget's bounds.

use eframe::egui::{Color32, Pos2, Rect, Shape, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Node;

/// `n`'s bounds, in points.
pub(crate) fn bounds(n: &Node<'_>) -> Rect {
    let b = n.bounding_box().expect("the widget has bounds");
    Rect::from_min_max(pos2(b.x0 as f32, b.y0 as f32), pos2(b.x1 as f32, b.y1 as f32))
}

/// The shapes `h`'s last frame painted wholly inside `area` (text left out), in paint order.
fn inside<S>(h: &Harness<'_, S>, area: Rect) -> Vec<Shape> {
    fn walk(s: &Shape, clip: Rect, area: Rect, out: &mut Vec<Shape>) {
        match s {
            Shape::Vec(v) => v.iter().for_each(|s| walk(s, clip, area, out)),
            Shape::Text(_) | Shape::Noop => {}
            s => {
                let r = s.visual_bounding_rect();
                if r.is_positive() && area.expand(1.0).contains_rect(r) && clip.intersects(r) {
                    out.push(s.clone());
                }
            }
        }
    }
    let mut out = Vec::new();
    for c in &h.output().shapes {
        walk(&c.shape, c.clip_rect, area, &mut out);
    }
    out
}

/// How many shapes `h`'s last frame painted inside `area`.
pub(crate) fn shapes_inside<S>(h: &Harness<'_, S>, area: Rect) -> usize {
    inside(h, area).len()
}

/// The fills of the rectangles `h`'s last frame painted inside `area`, in paint order.
pub(crate) fn fills_inside<S>(h: &Harness<'_, S>, area: Rect) -> Vec<Color32> {
    inside(h, area).into_iter().filter_map(|s| if let Shape::Rect(r) = s { Some(r.fill) } else { None }).filter(|c| c.a() > 0).collect()
}

/// The fills of the rectangles `h`'s last frame painted under `at` (a row's background, its
/// hover or selection), in paint order.
pub(crate) fn fills_at<S>(h: &Harness<'_, S>, at: Pos2) -> Vec<Color32> {
    fn walk(s: &Shape, clip: Rect, at: Pos2, out: &mut Vec<Color32>) {
        match s {
            Shape::Vec(v) => v.iter().for_each(|s| walk(s, clip, at, out)),
            Shape::Rect(r) if r.fill.a() > 0 && r.rect.contains(at) && clip.contains(at) => out.push(r.fill),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for c in &h.output().shapes {
        walk(&c.shape, c.clip_rect, at, &mut out);
    }
    out
}

/// The pieces of text `h`'s last frame painted underlined (a menu title's mnemonic), in paint
/// order.
pub(crate) fn underlined<S>(h: &Harness<'_, S>) -> Vec<String> {
    fn walk(s: &Shape, out: &mut Vec<String>) {
        match s {
            Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            Shape::Text(t) => {
                let job = &t.galley.job;
                for section in job.sections.iter().filter(|s| s.format.underline.width > 0.0) {
                    out.push(job.text[section.byte_range.clone()].to_owned());
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    for c in &h.output().shapes {
        walk(&c.shape, &mut out);
    }
    out
}
