//! The size tables (`ControlSize`, `ChipSize`), `text_pos` and `fits`.

use super::*;
use eframe::egui::{Color32, Context, FontId, vec2};

#[test]
fn the_control_sizes_follow_the_table() {
    let row = |s: ControlSize| {
        let m = s.metrics();
        (m.h, m.pad_x, m.quiet_pad_x, m.text, m.icon, m.icon_gap, m.radius, m.hit)
    };
    assert_eq!(row(ControlSize::Compact), (22.0, 0.0, 8.0, SMALL, 14.0, 4.0, radius::MD, 28.0));
    assert_eq!(row(ControlSize::Small), (24.0, 10.0, 10.0, SMALL, 16.0, 6.0, radius::MD, 28.0));
    assert_eq!(row(ControlSize::Regular), (28.0, 12.0, 12.0, BODY, 16.0, 6.0, radius::MD, 28.0));
    assert_eq!(row(ControlSize::Large), (36.0, 16.0, 16.0, BODY, 16.0, 8.0, radius::LG, 36.0));
    assert_eq!(ControlSize::default(), ControlSize::Regular);
    for s in [ControlSize::Small, ControlSize::Regular, ControlSize::Large] {
        assert_eq!(s.metrics().quiet_pad_x, s.metrics().pad_x, "{s:?}: quiet padding matches its framed sibling's");
    }
    assert!(!ControlSize::Compact.allows_frame());
    assert!(ControlSize::Small.allows_frame() && ControlSize::Large.allows_frame());
}

#[test]
fn the_chip_sizes_follow_the_table() {
    let b = ChipSize::Badge.metrics();
    assert_eq!((b.h, b.pad_x, b.text, b.glyph, b.gap, b.min_w), (18.0, 6.0, CAPTION, 10.0, 4.0, 18.0));
    assert_eq!(b.radius, radius::pill(b.h), "a badge is a pill");
    let c = ChipSize::Chip.metrics();
    assert_eq!((c.h, c.pad_x, c.text, c.glyph, c.gap, c.radius), (22.0, 8.0, SMALL, 12.0, 4.0, radius::SM));
}

#[test]
fn rows_host_the_sizes_the_table_lists() {
    // 28 px menu rows: Compact quiet controls, not Small bordered ones.
    assert!(fits(28.0, ControlSize::Compact, false));
    assert!(!fits(28.0, ControlSize::Small, true));
    // 32 px rows host Small; 36 px rows Regular.
    assert!(fits(32.0, ControlSize::Small, true));
    assert!(!fits(32.0, ControlSize::Regular, true));
    assert!(fits(36.0, ControlSize::Regular, true));
    // 44 px rows host Large.
    assert!(fits(44.0, ControlSize::Large, true));
    assert!(!fits(43.0, ControlSize::Large, true));
}

#[test]
fn text_sits_on_a_whole_point_never_low() {
    let ctx = Context::default();
    let mut galley = None;
    let _ = ctx.run(Default::default(), |ctx| {
        galley = Some(ctx.fonts(|f| f.layout_no_wrap("Stage hunk".into(), FontId::proportional(BODY), Color32::WHITE)));
    });
    let g = galley.unwrap();
    let rect = Rect::from_min_size(pos2(10.0, 3.0), vec2(80.0, 24.0));
    let p = text_pos(rect, &g);
    assert_eq!(p.x, 10.0);
    assert_eq!(p.y, p.y.floor());
    let above = p.y - rect.top();
    let below = rect.bottom() - (p.y + g.size().y);
    assert!(above <= below && below - above <= 1.0, "above {above}, below {below}");
}
