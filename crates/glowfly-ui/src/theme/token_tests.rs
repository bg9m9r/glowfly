//! The design tokens: the colour tokens worked out from every built-in palette and two custom
//! ones (their contrast floors and relationships), the corner scale, the elevations, the motion
//! curve and Reduce motion, the focus rings in both styles (and the Strong ring's contrast on
//! every built-in), and what `apply_style` installs.

use super::color::Palette;
use super::contrast::{lightness, mix, ratio};
use super::surfaces::{Tone, over};
use super::tests::ocean;
use super::tokens::{self, Elevation, motion, radius, shadow};
use super::*;
use eframe::egui::{self, Color32, CornerRadius, Rect, Shadow, StrokeKind};
use glowfly_core::settings::{BuiltinTheme, ColorValue, FocusStyle};

/// (name, palette, high contrast) for every built-in, the Garden pair and two custom palettes
/// with odd colours.
fn palettes() -> Vec<(String, Palette, bool)> {
    let mut all: Vec<(String, Palette, bool)> =
        super::tests::palettes().into_iter().map(|(name, p)| (name, *p, super::tests::high_contrast(p))).collect();
    let magenta = ColorValue::Rgb([0xff, 0x00, 0xff]);
    let dark = ocean(BuiltinTheme::Dark, &[("primary", magenta), ("accent", magenta), ("text", ColorValue::Rgb([0xc0, 0xc0, 0xc0]))]);
    let light = ocean(BuiltinTheme::Light, &[("accent", ColorValue::Rgb([0x00, 0xa0, 0xff]))]);
    all.push(("custom dark".into(), custom::build(&dark), false));
    all.push(("custom light".into(), custom::build(&light), false));
    all
}

/// The text floor: 7:1 in the high-contrast palettes, 4.5:1 elsewhere.
fn floor(hc: bool) -> f64 {
    if hc { 7.0 } else { 4.5 }
}

/// How far apart two colours are, summed over the channels.
fn distance(a: Color32, b: Color32) -> i32 {
    (i32::from(a.r()) - i32::from(b.r())).abs() + (i32::from(a.g()) - i32::from(b.g())).abs() + (i32::from(a.b()) - i32::from(b.b())).abs()
}

#[test]
fn the_overlay_floats_a_step_off_raised() {
    for (name, p, hc) in palettes() {
        if p.dark {
            assert!(lightness(p.raised) < lightness(p.overlay()), "{name}: lighter than raised");
            assert!(ratio(p.overlay(), p.raised) >= 1.08, "{name}: {:.3}", ratio(p.overlay(), p.raised));
        } else {
            assert_eq!(p.overlay(), p.panel, "{name}: the shadow does the lifting");
        }
        for (s, what) in [(p.overlay(), "overlay"), (p.inset(), "inset")] {
            assert!(ratio(p.text, s) >= floor(hc), "{name}: text on {what} {:.2}", ratio(p.text, s));
            assert!(ratio(p.weak, s) >= 4.5, "{name}: weak on {what} {:.2}", ratio(p.weak, s));
        }
    }
}

#[test]
fn destructive_menu_items_read_on_the_overlay() {
    for (name, p, hc) in palettes() {
        let (text, hover) = (p.menu_danger(), p.danger_hover());
        for (s, what) in [(p.overlay(), "overlay"), (hover, "danger hover")] {
            assert!(ratio(text, s) >= floor(hc), "{name}: danger on {what} {:.2}", ratio(text, s));
        }
        assert!(ratio(p.overlay(), hover) > 1.03, "{name}: the danger hover shows ({:.3})", ratio(p.overlay(), hover));
        assert_ne!(hover, p.hover_on(p.overlay()), "{name}: the danger hover is tinted");
    }
}

#[test]
fn quiet_danger_text_reads_at_rest_and_lit() {
    use super::tokens::{DANGER_QUIET_HOVER, DANGER_QUIET_PRESSED};
    for (name, p, hc) in palettes() {
        let ink = p.danger_ink();
        for (s, what) in [(p.panel, "panel"), (p.hunk_bg, "hunk band")] {
            let hover = over(p.wash(p.danger, DANGER_QUIET_HOVER), s);
            let pressed = over(p.wash(p.danger, DANGER_QUIET_PRESSED), s);
            for (c, state) in [(s, "rest"), (hover, "hover"), (pressed, "pressed")] {
                assert!(ratio(ink, c) >= floor(hc), "{name}: quiet danger on {what} at {state} {:.2}", ratio(ink, c));
            }
        }
    }
}

#[test]
fn state_layers_keep_text_readable_and_step_up() {
    for (name, p, hc) in palettes() {
        for (s, what) in [(p.panel, "panel"), (p.raised, "raised"), (p.overlay(), "overlay"), (p.tool_backing(), "tool backing")] {
            let (h, pr, l) = (p.hover_on(s), p.pressed_on(s), p.latched_on(s));
            for (c, state) in [(h, "hover"), (pr, "pressed"), (l, "latched")] {
                assert!(ratio(p.text, c) >= floor(hc), "{name}: text on {state} {what} {:.2}", ratio(p.text, c));
            }
            assert!(ratio(s, h) > 1.03, "{name}: hover on {what} shows ({:.3})", ratio(s, h));
            assert!(distance(s, h) < distance(s, pr) && distance(s, pr) < distance(s, l), "{name}: {what} layers step up");
        }
        assert_eq!(p.tool_hover(), p.hover_on(p.tool_backing()), "{name}");
        assert_eq!(p.tool_pressed(), p.latched_on(p.tool_backing()), "{name}");
    }
    // Today's toolbar fills, pinned.
    for p in [DARK, LIGHT] {
        assert_eq!(p.tool_hover(), mix(p.tool_backing(), p.text, 0.08));
        assert_eq!(p.tool_pressed(), mix(p.tool_backing(), p.text, 0.16));
    }
}

#[test]
fn translucent_layers_match_their_opaque_mix() {
    for (name, p, _) in palettes() {
        for (layer, opaque) in [(p.hover_layer(), p.hover_on(p.panel)), (p.pressed_layer(), p.pressed_on(p.panel))] {
            let seen = over(layer, p.panel);
            for (a, b) in [(seen.r(), opaque.r()), (seen.g(), opaque.g()), (seen.b(), opaque.b())] {
                assert!(a.abs_diff(b) <= 2, "{name}: {seen:?} vs {opaque:?}");
            }
        }
    }
}

#[test]
fn tonal_ink_reads_in_every_state() {
    for (name, p, hc) in palettes() {
        for fill in [p.tonal_fill(), p.tonal_state(false), p.tonal_state(true)] {
            let ink = p.tonal_ink(fill);
            assert!(ratio(ink, fill) >= floor(hc), "{name}: {:.2}", ratio(ink, fill));
        }
        assert!(ratio(p.tonal_fill(), p.panel) >= 1.1, "{name}: {:.3}", ratio(p.tonal_fill(), p.panel));
        assert_ne!(p.tonal_fill(), p.primary, "{name}");
    }
}

/// The diff's draft marker (`diff/comment_add.rs`): the tonal ink on the accent tint, at rest and
/// under the hover and pressed layers, reads as a mark (3:1).
#[test]
fn the_draft_markers_icon_reads_on_its_tint_in_every_state() {
    for (name, p, _) in palettes() {
        let tint = p.tonal_fill();
        let ink = p.tonal_ink(tint);
        for fill in [tint, p.hover_on(tint), p.pressed_on(tint)] {
            assert!(ratio(ink, fill) >= 3.0, "{name}: {:.2}", ratio(ink, fill));
        }
    }
}

#[test]
fn tones_map_to_the_palette_and_their_tints_read() {
    for (name, p, _) in palettes() {
        for t in Tone::ALL {
            let (fill, ink) = p.tone_tint(t);
            assert!(ratio(ink, fill) >= 4.5, "{name} {t:?}: {:.2}", ratio(ink, fill));
        }
        assert_eq!(p.tone(Tone::Success), p.status.added);
        assert_eq!(p.tone(Tone::Danger), p.status.removed);
        assert_eq!(p.tone(Tone::Warning), p.attention.warn);
        assert_eq!(p.tone(Tone::Info), p.status.info);
        assert_eq!(p.tone(Tone::Accent), p.accent);
    }
}

#[test]
fn the_focus_ring_shows_on_every_surface() {
    // Built-ins only: a theme file picks its own accent.
    for (t, p) in super::tests::palettes() {
        for (s, what) in [(p.bg, "bg"), (p.panel, "panel"), (p.input, "input"), (p.raised, "raised"), (p.overlay(), "overlay")] {
            assert!(ratio(p.accent, s) >= 3.0, "{t}: accent on {what} {:.2}", ratio(p.accent, s));
        }
        assert!(ratio(p.accent, p.tool_backing()) >= 3.0, "{t}: accent on the tool backing");
    }
}

/// Strong focus (Preferences > Accessibility > Focus indicator), in every built-in: the accent
/// ring reads against the `bg` outline around it, so it always meets a colour it stands out on
/// whatever is drawn beside it (the ring against each surface is
/// `the_focus_ring_shows_on_every_surface`).
#[test]
fn the_strong_ring_reads_on_its_bg_outline() {
    for (t, p) in super::tests::palettes() {
        assert!(ratio(p.accent, p.bg) >= 3.0, "{t}: accent on its bg outline {:.2}", ratio(p.accent, p.bg));
    }
}

/// The inset ring on a row (`row_fill`) reads on the selection, the inactive selection and the
/// hover fill, in either style; the Strong one's inner `bg` line sits against the accent.
#[test]
fn the_inset_ring_reads_on_every_row_fill() {
    for (t, p) in super::tests::palettes() {
        for (fill, what) in [(p.selected, "selected"), (tokens::selected_inactive(p), "selected inactive"), (p.hover, "hover")] {
            assert!(ratio(p.accent, fill) >= 3.0, "{t}: accent on {what} {:.2}", ratio(p.accent, fill));
        }
    }
}

/// A primary half's inset ring (`focus_ring_inset_in`) is the primary ink edged by the half's
/// own fill, so it only has to read on that fill: at rest, hovered and pressed.
#[test]
fn the_primary_inset_ring_reads_on_its_fill_in_every_state() {
    for (t, p) in super::tests::palettes() {
        for fill in [p.primary, p.shade_fill(p.primary, tokens::FILL_HOVER), p.shade_fill(p.primary, tokens::FILL_PRESSED)] {
            assert!(ratio(p.on_primary, fill) >= 3.0, "{t}: primary ink on {fill:?} {:.2}", ratio(p.on_primary, fill));
        }
    }
}

/// The rect strokes `draw` paints with `style` on, as (rect, width, colour, kind).
fn strokes(style: FocusStyle, draw: impl Fn(&egui::Painter)) -> Vec<(Rect, f32, Color32, StrokeKind)> {
    color::set_palette(&DARK);
    let ctx = egui::Context::default();
    tokens::set_focus_style(&ctx, style);
    let out = ctx.run(egui::RawInput::default(), |ctx| {
        draw(&egui::Painter::new(ctx.clone(), egui::LayerId::background(), Rect::EVERYTHING));
    });
    out.shapes
        .iter()
        .filter_map(|c| match &c.shape {
            egui::Shape::Rect(r) => Some((r.rect, r.stroke.width, r.stroke.color, r.stroke_kind)),
            _ => None,
        })
        .collect()
}

const FOCUSED: Rect = Rect { min: egui::Pos2 { x: 10.0, y: 10.0 }, max: egui::Pos2 { x: 110.0, y: 38.0 } };

#[test]
fn strong_rings_are_thicker_and_outlined_in_bg() {
    let ring = |style| strokes(style, |p| tokens::focus_ring(p, FOCUSED, radius::MD));
    let gap = FOCUSED.expand(tokens::FOCUS_GAP);
    assert_eq!(ring(FocusStyle::Standard), [(gap, 2.0, DARK.accent, StrokeKind::Outside)]);
    assert_eq!(
        ring(FocusStyle::Strong),
        [(gap, 3.0, DARK.accent, StrokeKind::Outside), (gap.expand(3.0), 1.0, DARK.bg, StrokeKind::Outside)]
    );
    let inset = |style| strokes(style, |p| tokens::focus_ring_inset(p, FOCUSED, 0));
    assert_eq!(inset(FocusStyle::Standard), [(FOCUSED, 1.5, DARK.accent, StrokeKind::Inside)]);
    assert_eq!(
        inset(FocusStyle::Strong),
        [(FOCUSED, 2.5, DARK.accent, StrokeKind::Inside), (FOCUSED.shrink(2.5), 1.0, DARK.bg, StrokeKind::Inside)]
    );
}

#[test]
fn a_strong_ring_on_a_primary_fill_is_edged_by_that_fill() {
    let fill = Color32::from_rgb(1, 2, 3);
    let ring = |style| strokes(style, |p| tokens::focus_ring_inset_in(p, FOCUSED, 0, DARK.on_primary, fill));
    assert_eq!(ring(FocusStyle::Standard), [(FOCUSED, 1.5, DARK.on_primary, StrokeKind::Inside)]);
    assert_eq!(
        ring(FocusStyle::Strong),
        [(FOCUSED, 1.0, fill, StrokeKind::Inside), (FOCUSED.shrink(1.0), 2.5, DARK.on_primary, StrokeKind::Inside)]
    );
}

#[test]
fn a_strong_text_field_takes_the_strong_ring_instead_of_its_halo() {
    let halo = |style| strokes(style, |p| tokens::focus_halo(p, FOCUSED, radius::MD));
    let soft = halo(FocusStyle::Standard);
    assert_eq!(soft.len(), 1);
    assert_eq!((soft[0].1, soft[0].2), (3.0, DARK.accent.gamma_multiply(0.22)), "the soft halo");
    assert!(halo(FocusStyle::Strong).is_empty(), "the field draws the Strong ring instead");
}

#[test]
fn the_radius_scale_nests() {
    const { assert!(radius::XS < radius::SM && radius::SM < radius::MD && radius::MD < radius::LG && radius::LG < radius::XL) };
    assert_eq!(radius::around(radius::MD, 4), 10);
    assert_eq!(radius::inside(10, 3), 7);
    assert_eq!(radius::inside(2, 3), 0, "never below 0");
    assert_eq!(radius::pill(20.0), 10);
    assert_eq!(radius::pill(1000.0), 255);
    assert_eq!(radius::top(5), CornerRadius { nw: 5, ne: 5, sw: 0, se: 0 });
    assert_eq!(radius::left(5), CornerRadius { nw: 5, sw: 5, ne: 0, se: 0 });
    assert_eq!(radius::right(5), CornerRadius { ne: 5, se: 5, nw: 0, sw: 0 });
}

#[test]
fn dialogs_float_higher_than_menus() {
    for p in [DARK, LIGHT] {
        let (f, d) = (shadow(Elevation::Float, &p), shadow(Elevation::Dialog, &p));
        assert!(d.blur > f.blur && d.offset[1] > f.offset[1]);
    }
    assert!(shadow(Elevation::Dialog, &DARK).color.a() > shadow(Elevation::Dialog, &LIGHT).color.a());
    for (p, alpha) in [(DARK, 64), (LIGHT, 28)] {
        let today = Shadow { offset: [0, 4], blur: 16, spread: 0, color: Color32::from_black_alpha(alpha) };
        assert_eq!(shadow(Elevation::Float, &p), today, "the popup shadow is unchanged");
    }
}

#[test]
fn a_lifted_row_casts_a_tight_shadow_darker_in_dark_themes() {
    for (p, alpha) in [(DARK, 89), (LIGHT, 46)] {
        let want = Shadow { offset: [0, 4], blur: 12, spread: 0, color: Color32::from_black_alpha(alpha) };
        assert_eq!(shadow(Elevation::Lift, &p), want, "y 4, blur 12, black at 35% dark and 18% light");
    }
    assert!(shadow(Elevation::Lift, &DARK).blur < shadow(Elevation::Float, &DARK).blur, "tighter than a menu's");
}

#[test]
fn the_easing_curve_settles_and_reduce_motion_stops_it() {
    assert_eq!(motion::ease(0.0), 0.0);
    assert!((motion::ease(1.0) - 1.0).abs() < 1e-4);
    let samples: Vec<f32> = (0..=100).map(|i| motion::ease(i as f32 / 100.0)).collect();
    assert!(samples.windows(2).all(|w| w[1] >= w[0]), "monotonic");
    assert!(motion::ease(0.5) > 0.5, "a quick start");
    let ctx = egui::Context::default();
    assert!(!motion::reduced(&ctx));
    assert_eq!(motion::duration(motion::STANDARD, &ctx), motion::STANDARD);
    ctx.all_styles_mut(|s| s.animation_time = 0.0);
    assert!(motion::reduced(&ctx));
    assert_eq!(motion::duration(motion::STANDARD, &ctx), 0.0);
    assert_eq!(motion::animation_time(true), 0.0);
    assert_eq!(motion::animation_time(false), motion::STANDARD);
}

#[test]
fn apply_style_installs_the_tokens() {
    for p in [DARK, LIGHT] {
        let ctx = egui::Context::default();
        apply_style(&ctx, &p);
        let s = ctx.style_of(egui_theme(&p));
        let v = &s.visuals;
        assert_eq!(v.window_fill, p.overlay());
        assert_eq!(v.window_stroke.color, p.overlay_edge());
        assert_eq!(v.menu_corner_radius, CornerRadius::same(radius::LG));
        assert_eq!(v.window_corner_radius, CornerRadius::same(radius::XL));
        assert_eq!(v.popup_shadow, shadow(Elevation::Float, &p));
        assert_eq!(v.widgets.inactive.corner_radius, CornerRadius::same(radius::MD));
        assert_eq!(v.widgets.hovered.weak_bg_fill, p.hover_on(p.overlay()));
        let frame = dialog_frame_for(&ctx, &p);
        assert_eq!((frame.fill, frame.corner_radius), (p.panel, CornerRadius::same(radius::XL)));
        assert_eq!((frame.shadow, frame.stroke.color), (shadow(Elevation::Dialog, &p), p.dialog_border));
        assert_eq!(modal_frame(&ctx, &p).fill, p.panel);
    }
}

#[test]
fn a_framed_tool_bar_edge_reads_on_the_backing() {
    for (name, p, _) in palettes() {
        if p.dark {
            assert_eq!(p.tool_edge(), p.border, "{name}: dark palettes keep the border");
        } else {
            let (edge, border) = (ratio(p.tool_edge(), p.tool_backing()), ratio(p.border, p.tool_backing()));
            assert!(edge > border, "{name}: the edge reads more than the border ({edge:.2} against {border:.2})");
        }
    }
    // Every built-in light palette: the picker edge reads at least 1.5:1 on its fill.
    for t in BuiltinTheme::ALL {
        let p = builtin(t);
        if !p.dark {
            let r = ratio(p.tool_edge(), p.tool_backing());
            assert!(r >= 1.5, "{t:?}: the picker edge on the tool backing {r:.2}");
        }
    }
    assert_eq!(GARDEN_LIGHT.tool_edge(), mix(GARDEN_LIGHT.border, GARDEN_LIGHT.text, 0.22));
}
