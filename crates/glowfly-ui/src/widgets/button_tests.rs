//! The button family: names, sizes, disabled behaviour, each kind's look, the focus ring, the
//! icon-only button and the split button's halves.

use super::*;
use crate::ui::theme::color;
use crate::ui::theme::sizes::ControlSize;
use crate::ui::theme::surfaces::Tone;
use crate::ui::theme::tokens::FOCUS_GAP;
use crate::ui::widgets::button_look::{Look, Press, look, look_for};
use crate::ui::widgets::button_split::CARET_W;
use eframe::egui::{Color32, Rect, Shape, Stroke};
use egui_kittest::kittest::Queryable;

/// The Regular and Small heights.
const HEIGHT: f32 = ControlSize::Regular.metrics().h;
const SMALL_HEIGHT: f32 = ControlSize::Small.metrics().h;

/// Makes a button of one kind.
type Make = fn(&'static str) -> StyledButton<'static>;

/// Every kind with a label and a maker.
fn kinds() -> [(&'static str, Make); 5] {
    [("Primary", primary), ("Tonal", tonal), ("Secondary", secondary), ("Quiet", quiet), ("Danger", danger)]
}

#[test]
fn disabled_buttons_are_named_and_inert() {
    let mut clicks = 0;
    let mut h = crate::ui::snapshot_check::builder().build_ui(|ui| {
        if ui.add(primary("Save").enabled(false).why(Some("Nothing changed"))).clicked() {
            clicks += 1;
        }
        ui.add(secondary("Cancel"));
    });
    h.run();
    assert!(h.get_by_label("Save").is_disabled());
    assert!(!h.get_by_label("Cancel").is_disabled());
    h.get_by_label("Save").click();
    h.run();
    drop(h);
    assert_eq!(clicks, 0);
}

#[test]
fn every_kind_is_named_by_its_text() {
    let mut h = crate::ui::snapshot_check::builder().build_ui(|ui| {
        for (name, make) in kinds() {
            ui.add(make(name));
        }
    });
    h.run();
    for (name, _) in kinds() {
        assert!(!h.get_by_label(name).is_disabled(), "{name}");
    }
}

#[test]
fn disabled_tonal_and_quiet_are_inert_and_explain() {
    for make in [tonal as Make, quiet] {
        let mut h = crate::ui::snapshot_check::builder().build_ui_state(
            |ui, clicks: &mut u32| {
                if ui.add(make("Stage all").enabled(false).why(Some("Nothing to stage"))).clicked() {
                    *clicks += 1;
                }
            },
            0,
        );
        h.run();
        assert!(h.get_by_label("Stage all").is_disabled());
        h.get_by_label("Stage all").click();
        h.run();
        assert_eq!(*h.state(), 0);
        h.get_by_label("Stage all").hover();
        h.run();
        h.run();
        assert!(h.query_by_label("Nothing to stage").is_some(), "the reason shows on hover");
    }
}

#[test]
fn sizes_meet_the_minimums() {
    let mut h = crate::ui::snapshot_check::builder().build_ui(|ui| {
        crate::ui::theme::apply_theme(ui.ctx());
        ui.set_style(ui.ctx().style());
        assert_eq!(ui.style().text_styles[&TextStyle::Button].size, crate::ui::theme::type_scale::BODY);
        assert_eq!(secondary("OK").min_width(76.0).desired_width(ui), 76.0);
        for (name, make) in kinds() {
            let r = ui.add(make(name));
            assert_eq!(r.rect.height(), HEIGHT, "{name}: default height at body size");
            assert_eq!(r.rect.width(), make(name).desired_width(ui), "{name}: the width it said");
            let r = ui.add(make(name).size(ControlSize::Small));
            assert_eq!(r.rect.height(), SMALL_HEIGHT, "{name}: small height at the small step");
            assert!(make(name).size(ControlSize::Small).desired_width(ui) < make(name).desired_width(ui), "{name}: small text is narrower");
        }
        let text_h = |b: StyledButton| b.galley(ui).size().y;
        let room = |h: f32, b: StyledButton| (h - text_h(b)) / 2.0;
        assert!(room(SMALL_HEIGHT, secondary("Retry").size(ControlSize::Small)) >= 3.5, "a small button's text has room inside its edge");
        assert!(
            (room(SMALL_HEIGHT, secondary("Retry").size(ControlSize::Small)) - room(HEIGHT, secondary("Retry"))).abs() <= 1.0,
            "small and default buttons give their text about the same room"
        );
        let r = ui.add(secondary("Stage hunk").height(17.0).min_width(120.0));
        assert_eq!(r.rect.height(), 17.0, "an exact height");
        assert_eq!(r.rect.width(), 120.0, "the least width still applies");
        assert_eq!(quiet("Fetch").desired_width(ui), secondary("Fetch").desired_width(ui), "quiet and framed: 12 px beside the text");
        let small_quiet = quiet("Fetch").size(ControlSize::Compact);
        assert_eq!(small_quiet.desired_width(ui) - small_quiet.galley(ui).size().x, 16.0, "Compact quiet: 8 px each side");
        let r = ui.add(quiet("Fetch").size(ControlSize::Compact));
        assert_eq!(r.rect.height(), 22.0, "Compact is 22 px");
        let r = ui.add(secondary("Commit").size(ControlSize::Large));
        assert_eq!(r.rect.height(), 36.0, "Large is 36 px");
        let plain = secondary("Fetch").desired_width(ui);
        assert_eq!(secondary("Fetch").icon(Icon::Refresh).desired_width(ui), plain + icon::SIZE_MD + 6.0, "an icon widens it");
    });
    h.run();
}

#[test]
fn icon_only_is_square_and_named() {
    let mut h = crate::ui::snapshot_check::builder().build_ui(|ui| {
        let r = ui.add(icon_only(Icon::Refresh, "Refresh"));
        assert_eq!(r.rect.size(), vec2(HEIGHT, HEIGHT));
        assert_eq!(icon_only(Icon::Refresh, "Refresh").desired_width(ui), HEIGHT);
        let r = ui.add(icon_only(Icon::Close, "Close").size(ControlSize::Small));
        assert_eq!(r.rect.size(), vec2(SMALL_HEIGHT, SMALL_HEIGHT));
        assert_eq!(icon_only(Icon::Close, "Close").size(ControlSize::Small).icon_size(), icon::SIZE_MD, "the size's icon");
    });
    h.run();
    h.get_by_label("Refresh").hover();
    h.run();
    h.run();
    assert_eq!(h.get_all_by_label("Refresh").count(), 2, "the name is the tooltip too");
}

#[test]
fn buttons_in_a_disabled_ui_still_explain_on_hover() {
    let mut h = crate::ui::snapshot_check::builder().build_ui(|ui| {
        ui.add_enabled_ui(false, |ui| {
            crate::ui::widgets::icon_button(ui, Icon::Refresh, "Previous change");
            ui.add(secondary("Move up").why(Some("Already first")));
        });
    });
    h.run();
    assert!(h.get_by_label("Previous change").is_disabled());
    h.get_by_label("Previous change").hover();
    h.run();
    h.run();
    assert_eq!(h.get_all_by_label("Previous change").count(), 2, "the icon button's tooltip shows while disabled");
    h.get_by_label("Move up").hover();
    h.run();
    h.run();
    assert!(h.query_by_label("Already first").is_some(), "the reason shows while the ui is disabled");
}

#[test]
fn looks_follow_the_kind_and_state() {
    let p = crate::ui::theme::DARK;
    let edge = |c| Stroke::new(1.0_f32, c);
    let rest = look_for(&p, Kind::Primary, true, Press::Rest);
    assert_eq!(rest, Look { fill: p.primary, ink: p.on_primary, edge: edge(p.primary_border) });
    assert_eq!(look_for(&p, Kind::Primary, true, Press::Hover).fill, p.shade_fill(p.primary, 0.10));
    assert_eq!(look_for(&p, Kind::Quiet, true, Press::Rest).fill, Color32::TRANSPARENT);
    assert_eq!(look_for(&p, Kind::Quiet, true, Press::Hover).fill, p.hover_layer());
    assert_eq!(look_for(&p, Kind::Quiet, false, Press::Rest), Look { fill: Color32::TRANSPARENT, ink: p.faint, edge: Stroke::NONE });
    for kind in [Kind::Primary, Kind::Tonal, Kind::Secondary, Kind::Danger] {
        assert_eq!(look_for(&p, kind, false, Press::Hover), Look { fill: p.raised, ink: p.weak, edge: Stroke::NONE }, "{kind:?}");
    }
    let dq = |press| look_for(&p, Kind::DangerQuiet, true, press);
    assert_eq!(dq(Press::Rest), Look { fill: Color32::TRANSPARENT, ink: p.danger, edge: Stroke::NONE }, "quiet danger has no fill at rest");
    assert_eq!(dq(Press::Hover).fill, p.wash(p.danger, 0.12));
    assert_eq!(dq(Press::Pressed).fill, p.wash(p.danger, 0.18));
    assert_eq!(look_for(&p, Kind::DangerQuiet, false, Press::Hover).ink, p.faint);
    let tonal = look_for(&p, Kind::Tonal, true, Press::Rest);
    assert_eq!((tonal.fill, tonal.ink), (p.tonal_fill(), p.tonal_ink(p.tonal_fill())));
    color::set_palette(&p);
    assert_eq!(look(Kind::Secondary, true, Press::Rest).fill, p.panel, "the active palette");
}

#[test]
fn hover_shades_toward_the_far_end() {
    let f = Color32::from_rgb(100, 100, 100);
    assert!(crate::ui::theme::DARK.shade_fill(f, 0.1).r() > 100);
    assert!(crate::ui::theme::LIGHT.shade_fill(f, 0.1).r() < 100);
}

/// The rects painted on the last of three frames of `add`, which is focused after the first, and
/// the rect `add` returned.
fn painted(add: impl Fn(&mut Ui) -> Response) -> (Rect, Vec<egui::epaint::RectShape>) {
    color::set_dark(true);
    let ctx = egui::Context::default();
    let mut rect = Rect::NOTHING;
    let mut shapes = Vec::new();
    for frame in 0..3 {
        let out = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let r = add(ui);
                if frame == 0 {
                    r.request_focus();
                }
                rect = r.rect;
            });
        });
        shapes = out.shapes.into_iter().filter_map(|c| if let Shape::Rect(r) = c.shape { Some(r) } else { None }).collect();
    }
    (rect, shapes)
}

#[test]
fn a_focused_button_gets_the_ring_outside_its_edge() {
    let (rect, shapes) = painted(|ui| ui.add(secondary("Fetch")));
    let accent = color::accent();
    assert!(shapes.iter().any(|s| s.stroke.color == accent && s.rect == rect.expand(FOCUS_GAP)), "{shapes:?}");
}

#[test]
fn a_toned_caret_takes_the_tint() {
    let (rect, shapes) = painted(|ui| split(primary("Commit"), "More").menu_tone(Some(Tone::Warning)).show(ui).1);
    let tint = color::current().tone_tint(Tone::Warning).0;
    assert_eq!(rect.width(), CARET_W);
    assert!(shapes.iter().any(|s| s.rect == rect && s.fill == tint), "{shapes:?}");
}

#[test]
fn the_split_halves_click_apart() {
    #[derive(Default)]
    struct Clicks {
        main: u32,
        caret: u32,
    }
    for main_on in [true, false] {
        let mut h = crate::ui::snapshot_check::builder().build_ui_state(
            move |ui, c: &mut Clicks| {
                let (main, caret) = split(primary("Commit").enabled(main_on), "More").show(ui);
                c.main += u32::from(main.clicked());
                c.caret += u32::from(caret.clicked());
            },
            Clicks::default(),
        );
        h.run();
        h.get_by_label("More").click();
        h.run();
        assert_eq!((h.state().main, h.state().caret), (0, 1), "the caret alone, the action enabled: {main_on}");
        h.get_by_label("Commit").click();
        h.run();
        assert_eq!((h.state().main, h.state().caret), (u32::from(main_on), 1), "the action alone");
    }
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "can't be Compact")]
fn a_framed_compact_button_panics_in_debug_builds() {
    let _ = secondary("Fetch").size(ControlSize::Compact);
}

#[test]
fn quiet_kinds_take_compact() {
    let _ = quiet("Fetch").size(ControlSize::Compact);
    let _ = danger_quiet("Discard").size(ControlSize::Compact);
    let _ = icon_only(Icon::Close, "Close").size(ControlSize::Compact);
    assert!(!Kind::Quiet.framed() && !Kind::DangerQuiet.framed());
    assert!(Kind::Primary.framed() && Kind::Secondary.framed() && Kind::Danger.framed() && Kind::Tonal.framed());
}

/// What a split of a primary "Commit" and its "More" caret paints on the last of three frames,
/// with the pointer at `at` and the half `focus` (0 the action, 1 the caret) focused after the
/// first: the halves' rects, the rects painted and the lines painted.
fn painted_split(at: egui::Pos2, focus: Option<usize>) -> ([Rect; 2], Vec<egui::epaint::RectShape>, Vec<[egui::Pos2; 2]>) {
    color::set_dark(true);
    let ctx = egui::Context::default();
    let mut halves = [Rect::NOTHING; 2];
    let (mut rects, mut lines) = (Vec::new(), Vec::new());
    for frame in 0..3 {
        let input = egui::RawInput { events: vec![egui::Event::PointerMoved(at)], ..Default::default() };
        let out = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let (main, caret) = split(primary("Commit"), "More").show(ui);
                if frame == 0
                    && let Some(i) = focus
                {
                    [&main, &caret][i].request_focus();
                }
                halves = [main.rect, caret.rect];
            });
        });
        rects.clear();
        lines.clear();
        for c in out.shapes {
            match c.shape {
                Shape::Rect(r) if r.rect.width() < 1000.0 => rects.push(r),
                Shape::LineSegment { points, .. } => lines.push(points),
                _ => {}
            }
        }
    }
    (halves, rects, lines)
}

#[test]
fn a_split_has_one_edge_and_one_divider() {
    let (halves, rects, lines) = painted_split(egui::Pos2::ZERO, None);
    let whole = halves[0].union(halves[1]);
    assert_eq!(halves[0].right(), halves[1].left(), "flush");
    let edges: Vec<_> = rects.iter().filter(|s| s.stroke.width > 0.0).collect();
    assert_eq!(edges.len(), 1, "only the whole shape has an edge: {edges:?}");
    assert_eq!(edges[0].rect, whole);
    let dividers: Vec<_> = lines.iter().filter(|l| l[0].x == l[1].x).collect();
    assert_eq!(dividers.len(), 1, "one divider: {lines:?}");
    assert_eq!(dividers[0][0].x, halves[1].left().round() + 0.5, "on the seam, 1 px wide");
}

#[test]
fn only_the_hovered_half_of_a_split_lights() {
    let fill_at =
        |rects: &[egui::epaint::RectShape], r: Rect| rects.iter().find(|s| s.rect == r && s.fill != Color32::TRANSPARENT).map(|s| s.fill);
    let (halves, rects, _) = painted_split(egui::Pos2::ZERO, None);
    let rest = fill_at(&rects, halves[0]).expect("the primary fill");
    assert_eq!(fill_at(&rects, halves[1]), Some(rest), "both halves at rest");
    let (_, rects, _) = painted_split(halves[0].center(), None);
    let (main, caret) = (fill_at(&rects, halves[0]), fill_at(&rects, halves[1]));
    assert!(main.is_some_and(|f| f != rest) && caret == Some(rest), "the action alone: {main:?} {caret:?}");
    let (_, rects, _) = painted_split(halves[1].center(), None);
    let (main, caret) = (fill_at(&rects, halves[0]), fill_at(&rects, halves[1]));
    assert!(main == Some(rest) && caret.is_some_and(|f| f != rest), "the caret alone: {main:?} {caret:?}");
}

#[test]
fn a_split_half_rings_its_own_shape() {
    let r = ControlSize::Regular.metrics().radius;
    for (i, corners) in
        [(0, crate::ui::theme::tokens::radius::segment(r, true, false)), (1, crate::ui::theme::tokens::radius::segment(r, false, true))]
    {
        let (halves, rects, _) = painted_split(egui::Pos2::ZERO, Some(i));
        let on_primary = color::current().on_primary;
        let rings: Vec<_> = rects.iter().filter(|s| s.stroke.color == on_primary && s.fill == Color32::TRANSPARENT).collect();
        assert_eq!(rings.len(), 1, "half {i}: {rects:?}");
        assert_eq!((rings[0].rect, rings[0].corner_radius), (halves[i], corners), "half {i}: inset, on its own corners");
        assert_eq!(rings[0].stroke_kind, egui::StrokeKind::Inside);
    }
}
