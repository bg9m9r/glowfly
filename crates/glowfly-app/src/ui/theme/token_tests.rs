//! The design tokens: the colour tokens worked out from every built-in palette and two custom
//! ones (their contrast floors and relationships), the corner scale, the elevations, the motion
//! curve and Reduce motion, and what `apply_style` installs.

use super::color::Palette;
use super::contrast::{lightness, mix, ratio};
use super::surfaces::Tone;
use super::tests::ocean;
use super::tokens::{Elevation, motion, radius, shadow};
use super::*;
use eframe::egui::{self, Color32, CornerRadius, Shadow};
use glowfly_core::settings::{BuiltinTheme, ColorValue};

/// (name, palette, high contrast) for every built-in and two custom palettes with odd colours.
fn palettes() -> Vec<(String, Palette, bool)> {
    let mut all: Vec<(String, Palette, bool)> = BuiltinTheme::ALL
        .iter()
        .map(|&t| {
            let hc = matches!(t, BuiltinTheme::HighContrastDark | BuiltinTheme::HighContrastLight);
            (format!("{t:?}"), *builtin(t), hc)
        })
        .collect();
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

/// Premultiplied `top` laid over `under`, byte by byte in gamma space as egui blends.
fn over(top: Color32, under: Color32) -> Color32 {
    let a = f32::from(top.a()) / 255.0;
    let ch = |t: u8, u: u8| (f32::from(t) + f32::from(u) * (1.0 - a)).round() as u8;
    Color32::from_rgb(ch(top.r(), under.r()), ch(top.g(), under.g()), ch(top.b(), under.b()))
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
    for t in BuiltinTheme::ALL {
        let p = builtin(t);
        for (s, what) in [(p.bg, "bg"), (p.panel, "panel"), (p.input, "input"), (p.raised, "raised"), (p.overlay(), "overlay")] {
            assert!(ratio(p.accent, s) >= 3.0, "{t:?}: accent on {what} {:.2}", ratio(p.accent, s));
        }
        assert!(ratio(p.accent, p.tool_backing()) >= 3.0, "{t:?}: accent on the tool backing");
    }
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
